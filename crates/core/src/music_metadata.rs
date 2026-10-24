//! Bounded header/container inspection; this never decodes or transcodes audio.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use symphonia::core::{
    formats::FormatOptions,
    io::{MediaSource, MediaSourceStream},
    meta::{Limit, MetadataOptions},
    probe::{Hint, ProbeResult},
    units::TimeBase,
};

struct ProbeFile {
    file: File,
    remaining: usize,
    seeks: usize,
}
impl Read for ProbeFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("Audio metadata read limit"));
        }
        let limit = buffer.len().min(self.remaining);
        let count = self.file.read(&mut buffer[..limit])?;
        self.remaining -= count;
        Ok(count)
    }
}
impl Seek for ProbeFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.seeks == 0 {
            return Err(io::Error::other("Audio metadata seek limit"));
        }
        self.seeks -= 1;
        self.file.seek(position)
    }
}
impl MediaSource for ProbeFile {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        self.file.metadata().ok().map(|m| m.len())
    }
}

pub(crate) fn duration(file: File, extension: &str) -> Option<f64> {
    let probe = probe(file, extension, 0)?;
    let parameters = &probe.format.default_track()?.codec_params;
    let frames = parameters.n_frames?;
    let base = parameters.time_base.or_else(|| {
        parameters
            .sample_rate
            .filter(|rate| *rate > 0)
            .map(|rate| TimeBase::new(1, rate))
    })?;
    let time = base.calc_time(frames);
    let seconds = time.seconds as f64 + time.frac;
    (seconds.is_finite() && seconds > 0.0 && seconds <= 7.0 * 24.0 * 3600.0).then_some(seconds)
}

pub(crate) fn artwork(file: File, extension: &str) -> Option<Vec<u8>> {
    let mut probe = probe(file, extension, 2 * 1024 * 1024)?;
    let choose = |revision: &symphonia::core::meta::MetadataRevision| {
        revision
            .visuals()
            .iter()
            .filter(|v| v.data.len() <= 2 * 1024 * 1024)
            .min_by_key(|v| v.usage != Some(symphonia::core::meta::StandardVisualKey::FrontCover))
            .map(|v| v.data.to_vec())
    };
    if let Some(metadata) = probe.metadata.get() {
        if let Some(bytes) = metadata.current().and_then(choose) {
            return Some(bytes);
        }
    }
    probe.format.metadata().current().and_then(choose)
}

fn probe(file: File, extension: &str, visual_bytes: usize) -> Option<ProbeResult> {
    let source = ProbeFile {
        file,
        remaining: 2 * 1024 * 1024,
        seeks: 256,
    };
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let metadata = MetadataOptions {
        limit_metadata_bytes: Limit::Maximum(64 * 1024),
        limit_visual_bytes: Limit::Maximum(visual_bytes),
    };
    let format = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    symphonia::default::get_probe()
        .format(&hint, stream, &format, &metadata)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn id3_front_cover_is_read_without_decoding_the_audio() {
        let mut jpeg = vec![];
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&image::RgbImage::new(8, 8))
            .unwrap();
        let mut payload = b"\0image/jpeg\0\x03\0".to_vec();
        payload.extend(&jpeg);
        let mut frame = b"APIC".to_vec();
        frame.extend((payload.len() as u32).to_be_bytes());
        frame.extend([0, 0]);
        frame.extend(payload);
        let size = frame.len() as u32;
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"ID3\x03\0\0").unwrap();
        file.write_all(&[
            ((size >> 21) & 127) as u8,
            ((size >> 14) & 127) as u8,
            ((size >> 7) & 127) as u8,
            (size & 127) as u8,
        ])
        .unwrap();
        file.write_all(&frame).unwrap();
        for _ in 0..8 {
            file.write_all(&[0xff, 0xfb, 0x90, 0]).unwrap();
            file.write_all(&[0; 413]).unwrap();
        }
        file.rewind().unwrap();
        assert_eq!(artwork(file, "mp3"), Some(jpeg));
    }
    #[test]
    fn valid_pcm_duration_and_malformed_input_are_distinct() {
        let mut file = tempfile::tempfile().unwrap();
        let samples = 16000u32;
        let mut header = vec![];
        header.extend(b"RIFF");
        header.extend((36 + samples).to_le_bytes());
        header.extend(b"WAVEfmt ");
        header.extend(16u32.to_le_bytes());
        header.extend(1u16.to_le_bytes());
        header.extend(1u16.to_le_bytes());
        header.extend(8000u32.to_le_bytes());
        header.extend(16000u32.to_le_bytes());
        header.extend(2u16.to_le_bytes());
        header.extend(16u16.to_le_bytes());
        header.extend(b"data");
        header.extend(samples.to_le_bytes());
        file.write_all(&header).unwrap();
        file.write_all(&vec![0; samples as usize]).unwrap();
        file.rewind().unwrap();
        assert!((duration(file, "wav").unwrap() - 1.0).abs() < 0.001);
        let mut malformed = tempfile::tempfile().unwrap();
        malformed.write_all(b"RIFF0000WAVEbad").unwrap();
        malformed.rewind().unwrap();
        assert!(duration(malformed, "wav").is_none());
    }
    #[test]
    fn reads_and_seeks_cannot_exceed_the_probe_budget() {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"12345678").unwrap();
        file.rewind().unwrap();
        let mut source = ProbeFile {
            file,
            remaining: 4,
            seeks: 1,
        };
        assert_eq!(source.read(&mut [0; 8]).unwrap(), 4);
        assert!(source.read(&mut [0; 1]).is_err());
        source.seek(SeekFrom::Start(0)).unwrap();
        assert!(source.seek(SeekFrom::Start(0)).is_err());
    }
}
