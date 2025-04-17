use core::{error::Error, fmt};
use std::io;

use alsa::pcm::Format;
use hound::{SampleFormat, WavSpec};

pub fn cvt_err(err: hound::Error) -> io::Error {
    match err {
        hound::Error::IoError(io) => io,
        hound::Error::FormatError(m) => io::Error::from_static_message(m),
        e => io::Error::new(io::ErrorKind::InvalidData, e),
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct UnsupportedFormatError(pub WavSpec);

impl fmt::Display for UnsupportedFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "Unsupported format: {:?}, ({} bits, {} bytes) per sample",
            self.0.sample_format, self.0.bits_per_sample, self.0.bytes_per_sample
        )
    }
}

impl Error for UnsupportedFormatError {}

pub fn cvt_format(spec: WavSpec) -> Result<Format, UnsupportedFormatError> {
    match spec.sample_format {
        SampleFormat::Int => match (spec.bits_per_sample, spec.bytes_per_sample) {
            (8, 1) => Ok(Format::S8),
            (16, 2) => Ok(Format::S16LE),
            (18, 3) => Ok(Format::S183LE),
            (20, 3) => Ok(Format::S203LE),
            (24, 3) => Ok(Format::S243LE),
            (24, 4) => Ok(Format::S24LE),
            (32, 4) => Ok(Format::S32LE),
            _ => Err(UnsupportedFormatError(spec)),
        },
        SampleFormat::Float => match (spec.bits_per_sample, spec.bytes_per_sample) {
            (32, 4) => Ok(Format::FloatLE),
            (64, 8) => Ok(Format::Float64LE),
            _ => Err(UnsupportedFormatError(spec)),
        },
    }
}
