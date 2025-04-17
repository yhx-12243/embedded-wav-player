use core::{error::Error, fmt, hint::likely, mem, ptr, slice};
use std::io::{self, BufRead, BufReader};

use alsa::pcm::Format;
use hound::{SampleFormat, WavSpec};

pub fn cvt_err(err: hound::Error) -> io::Error {
    match err {
        hound::Error::IoError(io) => io,
        hound::Error::FormatError(m) => io::Error::from_static_message(m),
        e => io::Error::new(io::ErrorKind::InvalidData, e),
    }
}

#[cold]
pub fn read_surplus<R>(
    buf: *const [u8],
    rem: usize,
    sample_size: usize,
    reader: &mut BufReader<R>,
) -> io::Result<Vec<u8>>
where
    R: io::Read,
{
    let m = buf.len();
    let n = m + rem;
    let mut v = Vec::with_capacity(n);
    unsafe {
        ptr::copy_nonoverlapping(buf.as_ptr(), v.as_mut_ptr(), m);
        reader.consume(m);
        reader.get_mut().read_exact(slice::from_raw_parts_mut(v.as_mut_ptr().add(m), rem))?;
        v.set_len(n);
    }

    let cap = reader.capacity();
    let crem = cap % sample_size;
    if likely(crem == 0) {
        return Ok(v);
    }

    let new_cap = cap + (sample_size - crem);
    println!("buffer capacity ({cap}) is not multiple of sample size ({sample_size}), re-buffering with size {new_cap}.");
    let inner = unsafe { ptr::read(reader) };
    let new = BufReader::with_capacity(new_cap, inner.into_inner());
    unsafe { ptr::write(reader, new) };

    Ok(v)
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
            (20, 4) => Ok(unsafe {
                mem::transmute::<u8, alsa::pcm::Format>(alsa_sys::SND_PCM_FORMAT_S20_LE as u8)
            }),
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
