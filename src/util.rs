use core::{error::Error, fmt, hint::unlikely};
use std::{
    io::{self, BufReader},
    sync::mpsc::RecvError,
};

use alsa::pcm::Format;
use hound::{SampleFormat, WavSpec};
use replace_with::replace_with_or_abort;

#[inline]
pub fn cvt_err(err: hound::Error) -> io::Error {
    match err {
        hound::Error::IoError(io) => io,
        hound::Error::FormatError(m) => io::Error::from_static_message(m),
        e => io::Error::new(io::ErrorKind::InvalidData, e),
    }
}

#[cold]
pub fn buffer_resize_if_need<R>(sample_size: usize, reader: &mut BufReader<R>)
where
    R: io::Read,
{
    let cap = reader.capacity();
    let crem = cap % sample_size;

    if unlikely(crem != 0) {
        let new_cap = cap + (sample_size - crem);
        println!("buffer capacity ({cap}) is not multiple of sample size ({sample_size}), re-buffering with size {new_cap}.");
        replace_with_or_abort(reader, |owned| BufReader::with_capacity(new_cap, owned.into_inner()));
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

pub fn cvt_format(spec: WavSpec) -> Result<Format, PlayError> {
    let format = match spec.sample_format {
        SampleFormat::Int => match (spec.bits_per_sample, spec.bytes_per_sample) {
            (8, 1) => Format::S8,
            (16, 2) => Format::S16LE,
            (18, 3) => Format::S183LE,
            (20, 3) => Format::S203LE,
            (24, 3) => Format::S243LE,
            (20, 4) => Format::S20LE,
            (24, 4) => Format::S24LE,
            (32, 4) => Format::S32LE,
            _ => return Err(PlayError::Format(UnsupportedFormatError(spec))),
        },
        SampleFormat::Float => match (spec.bits_per_sample, spec.bytes_per_sample) {
            (32, 4) => Format::FloatLE,
            (64, 8) => Format::Float64LE,
            _ => return Err(PlayError::Format(UnsupportedFormatError(spec))),
        },
    };
    if format.physical_width()? == i32::from(spec.bytes_per_sample) * 8 && format.width()? == spec.bits_per_sample.into() {
        Ok(format)
    } else {
        Err(PlayError::Format(UnsupportedFormatError(spec)))
    }
}

pub enum PlayError {
    Alsa(alsa::Error),
    Format(UnsupportedFormatError),
    Io(io::Error),
}

impl From<alsa::Error> for PlayError {
    #[inline]
    fn from(err: alsa::Error) -> Self {
        Self::Alsa(err)
    }
}

impl From<UnsupportedFormatError> for PlayError {
    #[inline]
    fn from(err: UnsupportedFormatError) -> Self {
        Self::Format(err)
    }
}

impl From<io::Error> for PlayError {
    #[inline]
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<RecvError> for PlayError {
    #[inline]
    fn from(err: RecvError) -> Self {
        Self::Io(io::Error::other(err))
    }
}

impl From<PlayError> for io::Error {
    #[inline]
    fn from(err: PlayError) -> Self {
        match err {
            PlayError::Alsa(e) => Self::other(e),
            PlayError::Format(e) => Self::other(e),
            PlayError::Io(e) => e,
        }
    }
}

pub enum PlayerEvent {
    Terminate,
    Move { offset: isize },
    SetMultipler { multiplier: u8 },
    Pause,
    Resume,
}

pub enum MP3Event {
    PlayerEnd,
}
