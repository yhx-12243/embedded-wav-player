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

pub enum PlayError {
    Alsa(alsa::Error),
    Format(WavSpec),
}

impl From<alsa::Error> for PlayError {
    fn from(err: alsa::Error) -> Self {
        Self::Alsa(err)
    }
}

impl WavSpec for PlayError {
    fn from_spec(spec: WavSpec) -> Self {
        Self::Format(spec)
    }
}

pub fn cvt_format(spec: WavSpec) -> Result<Format, WavSpec> {
    match spec.sample_format {
        SampleFormat::Int => match (spec.bits_per_sample, spec.bytes_per_sample) {
            (8, 1) => Ok(Format::S8),
            (16, 2) => Ok(Format::S16LE),
            (18, 3) => Ok(Format::S183LE),
            (20, 3) => Ok(Format::S203LE),
            (24, 3) => Ok(Format::S243LE),
            (24, 4) => Ok(Format::S24LE),
            (32, 4) => Ok(Format::S32LE),
            _ => Err(spec),
        },
        SampleFormat::Float => match (spec.bits_per_sample, spec.bytes_per_sample) {
            (32, 4) => Ok(Format::FloatLE),
            (64, 8) => Ok(Format::Float64LE),
            _ => Err(spec),
        },
    }
}
