//! UnRAR adapter: inspect raw redirection metadata and stream bytes, never ask
//! UnRAR to create filesystem entries. The high-level wrapper omits redir_type.
use crate::error::{AppError, Result};
use std::ffi::CString;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use unrar_sys as ffi;

// dll.hpp uses #pragma pack(push, 1). unrar_sys 0.5.8 uses repr(C),
// which pads fields differently on 64-bit systems. Keep these wire structs
// packed and zero their reserved/output-pointer fields before calling C.
#[repr(C, packed)]
struct OpenData {
    archive_name: *const libc::c_char,
    archive_name_w: *const libc::wchar_t,
    open_mode: u32,
    open_result: u32,
    comment_buffer: *mut libc::c_char,
    comment_buffer_size: u32,
    comment_size: u32,
    comment_state: u32,
    flags: u32,
    callback: Option<ffi::Callback>,
    user_data: ffi::LPARAM,
    op_flags: u32,
    comment_buffer_w: *mut libc::wchar_t,
    reserved: [u32; 25],
}

#[repr(C, packed)]
struct HeaderData {
    archive_name: [libc::c_char; 1024],
    archive_name_w: [libc::wchar_t; 1024],
    filename: [libc::c_char; 1024],
    filename_w: [libc::wchar_t; 1024],
    flags: u32,
    pack_size: u32,
    pack_size_high: u32,
    unp_size: u32,
    unp_size_high: u32,
    host_os: u32,
    file_crc: u32,
    file_time: u32,
    unp_ver: u32,
    method: u32,
    file_attr: u32,
    comment_buffer: *mut libc::c_char,
    comment_buffer_size: u32,
    comment_size: u32,
    comment_state: u32,
    dict_size: u32,
    hash_type: u32,
    hash: [libc::c_char; 32],
    redir_type: u32,
    redir_name: *mut libc::wchar_t,
    redir_name_size: u32,
    dir_target: u32,
    mtime_low: u32,
    mtime_high: u32,
    ctime_low: u32,
    ctime_high: u32,
    atime_low: u32,
    atime_high: u32,
    archive_name_ex: *mut libc::wchar_t,
    archive_name_ex_size: u32,
    filename_ex: *mut libc::wchar_t,
    filename_ex_size: u32,
    reserved: [u32; 982],
}

pub(crate) struct Archive(*const ffi::Handle);

pub(crate) struct Header {
    pub name: String,
    pub size: u64,
    pub directory: bool,
}

fn error(code: i32) -> AppError {
    AppError::new(
        "INTAKE_FORMAT",
        format!("RAR read failed (UnRAR code {code})"),
    )
}

// Refuse password and volume requests, including while opening an archive.
extern "C" fn refuse_requests(_: ffi::UINT, _: ffi::LPARAM, _: ffi::LPARAM, _: ffi::LPARAM) -> i32 {
    -1
}

impl Archive {
    pub fn open(path: &Path) -> Result<Self> {
        let name = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| AppError::new("INTAKE_FORMAT", "Invalid RAR path"))?;
        #[cfg(windows)]
        let wide: Vec<u16> = {
            use std::os::windows::ffi::OsStrExt;
            path.as_os_str().encode_wide().chain(Some(0)).collect()
        };
        let mut data = OpenData {
            archive_name: name.as_ptr(),
            #[cfg(windows)]
            archive_name_w: wide.as_ptr(),
            open_mode: ffi::RAR_OM_EXTRACT,
            callback: Some(refuse_requests),
            // SAFETY: all remaining fields are integers, nullable pointers,
            // arrays, or an Option function pointer; zero is valid for each.
            ..unsafe { std::mem::zeroed() }
        };
        // SAFETY: name and data remain alive throughout this synchronous call.
        let handle = unsafe { ffi::RAROpenArchiveEx((&mut data as *mut OpenData).cast()) };
        if handle.is_null() {
            return Err(error(data.open_result as i32));
        }
        let archive = Self(handle);
        if data.flags & (ffi::ROADF_ENCHEADERS | ffi::ROADF_VOLUME) != 0 {
            return Err(AppError::new(
                "INTAKE_FORMAT",
                "Encrypted or multi-volume RAR archives are not supported",
            ));
        }
        Ok(archive)
    }

    pub fn next(&mut self) -> Result<Option<Header>> {
        // SAFETY: this C struct contains only scalar fields and nullable pointers.
        let mut h: HeaderData = unsafe { std::mem::zeroed() };
        // SAFETY: self owns a live handle; UnRAR fills the writable header.
        let code = unsafe { ffi::RARReadHeaderEx(self.0, (&mut h as *mut HeaderData).cast()) };
        if code == ffi::ERAR_END_ARCHIVE {
            return Ok(None);
        }
        if code != ffi::ERAR_SUCCESS {
            return Err(error(code));
        }
        validate_header(&h)?;
        let filename_w = h.filename_w;
        let units: Vec<_> = filename_w.iter().copied().take_while(|c| *c != 0).collect();
        #[cfg(windows)]
        let name = String::from_utf16(&units).ok();
        #[cfg(not(windows))]
        let name: Option<String> = units
            .into_iter()
            .map(|c| char::from_u32(c as u32))
            .collect();
        let name = name
            .ok_or_else(|| AppError::new("INTAKE_UNSAFE", "RAR filename is not valid Unicode"))?;
        Ok(Some(Header {
            name,
            size: (u64::from(h.unp_size_high) << 32) | u64::from(h.unp_size),
            directory: h.flags & ffi::RHDF_DIRECTORY != 0,
        }))
    }

    pub fn skip(&mut self) -> Result<()> {
        self.process(ffi::RAR_SKIP)
    }

    fn process(&mut self, operation: i32) -> Result<()> {
        // SAFETY: live handle; null destinations since we never use RAR_EXTRACT.
        let code =
            unsafe { ffi::RARProcessFile(self.0, operation, std::ptr::null(), std::ptr::null()) };
        if code == ffi::ERAR_SUCCESS {
            Ok(())
        } else {
            Err(error(code))
        }
    }

    pub fn write(&mut self, file: &mut File, size: u64) -> Result<()> {
        let mut output = Output {
            file,
            remaining: size,
            failure: None,
        };
        // SAFETY: callback runs synchronously in process(); output lives until
        // after callback removal. It is never accessed concurrently by Rust.
        unsafe {
            ffi::RARSetCallback(
                self.0,
                Some(write_data),
                &mut output as *mut Output<'_> as ffi::LPARAM,
            )
        };
        let result = self.process(ffi::RAR_TEST);
        unsafe { ffi::RARSetCallback(self.0, Some(refuse_requests), 0) };
        if let Some(failure) = output.failure {
            return Err(failure);
        }
        result?;
        if output.remaining != 0 {
            return Err(AppError::new(
                "INTAKE_FORMAT",
                "RAR entry ended before its declared size",
            ));
        }
        file.flush()?;
        Ok(())
    }
}

impl Drop for Archive {
    fn drop(&mut self) {
        // SAFETY: this handle is owned and closed exactly once.
        unsafe { ffi::RARCloseArchive(self.0) };
    }
}

fn validate_header(h: &HeaderData) -> Result<()> {
    if h.flags & (ffi::RHDF_ENCRYPTED | ffi::RHDF_SPLITBEFORE | ffi::RHDF_SPLITAFTER) != 0 {
        return Err(AppError::new(
            "INTAKE_FORMAT",
            "Encrypted or multi-volume RAR archives are not supported",
        ));
    }
    // RAR4 Unix modes use the low bits. UnRAR reports both RAR4 symlinks
    // and RAR5 symlinks, hardlinks, junctions and file copies as redirections.
    if h.redir_type != 0 || h.file_attr & 0o170000 == 0o120000 {
        return Err(AppError::new(
            "INTAKE_UNSAFE",
            "RAR links and redirection entries are not supported",
        ));
    }
    Ok(())
}

struct Output<'a> {
    file: &'a mut File,
    remaining: u64,
    failure: Option<AppError>,
}

extern "C" fn write_data(
    message: ffi::UINT,
    user: ffi::LPARAM,
    data: ffi::LPARAM,
    len: ffi::LPARAM,
) -> i32 {
    if message != ffi::UCM_PROCESSDATA || len < 0 || (len > 0 && data == 0) {
        return -1;
    }
    // SAFETY: UnRAR returns the state pointer registered by write(), with a
    // readable decompressed buffer valid only for this callback invocation.
    let out = unsafe { &mut *(user as *mut Output<'_>) };
    if out.failure.is_some() {
        return -1;
    }
    if len as u64 > out.remaining {
        out.failure = Some(AppError::new(
            "INTAKE_LIMIT",
            "An entry unpacked to more than its declared size",
        ));
        return -1;
    }
    if len == 0 {
        return 1;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, len as usize) };
    if let Err(e) = out.file.write_all(bytes) {
        out.failure = Some(e.into());
        return -1;
    }
    out.remaining -= len as u64;
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_refuses_oversized_data_before_writing() {
        let mut file = tempfile::tempfile().unwrap();
        let mut output = Output {
            file: &mut file,
            remaining: 2,
            failure: None,
        };
        let bytes = b"abc";
        let result = write_data(
            ffi::UCM_PROCESSDATA,
            &mut output as *mut Output<'_> as ffi::LPARAM,
            bytes.as_ptr() as ffi::LPARAM,
            3,
        );
        assert_eq!(result, -1);
        assert_eq!(output.failure.unwrap().code, "INTAKE_LIMIT");
        assert_eq!(file.metadata().unwrap().len(), 0);
    }
}
