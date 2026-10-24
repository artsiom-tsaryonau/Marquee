//! Bounded header/container inspection; this never decodes or transcodes audio.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use symphonia::core::{
    formats::FormatOptions,
    io::{MediaSource, MediaSourceStream},
    meta::{Limit, MetadataOptions},
    probe::Hint,
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
        limit_visual_bytes: Limit::Maximum(0),
    };
    let format = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let probe = symphonia::default::get_probe()
        .format(&hint, stream, &format, &metadata)
        .ok()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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
