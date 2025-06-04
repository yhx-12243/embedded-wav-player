use core::{error::Error, fmt};
use std::{io, sync::mpsc::RecvError};

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

pub fn buffer_resize<R>(reader: &mut io::BufReader<R>, new_size: usize)
where
    R: io::Read,
{
    if reader.capacity() != new_size {
        tracing::info!("Resizing buffer from {} to {}", reader.capacity(), new_size);
        replace_with_or_abort(reader, |owned| io::BufReader::with_capacity(new_size, owned.into_inner()));
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

#[derive(Debug)]
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

#[derive(Clone, Copy, Debug)]
pub enum PlayerEvent {
    Terminate,
    Move { offset: isize },
    SetMultiplier { multiplier: u8 },
    Pause,
    Resume,
}

pub use hack::Handle;

#[derive(Clone, Copy, Debug)]
pub enum MP3Event {
    PlayerEnd { player: Handle },
    Close,
    Dispatch { sub: PlayerEvent },
    SwitchSong { seek: io::SeekFrom },
    SetVolume { volume: i32 },
}

impl From<PlayerEvent> for MP3Event {
    #[inline]
    fn from(event: PlayerEvent) -> Self {
        Self::Dispatch { sub: event }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum GUIEvent {
    SwitchSong { index: usize, handle: Handle },
    ProgressAccess { access: Option<ProgressAccess>, handle: Handle },
    SetMultiplier { multiplier: u8 },
}

#[derive(Clone, Copy, Default)]
pub struct Progress {
    pub begin: usize,
    pub pos: usize,
    pub end: usize,
    pub delay: isize,
}

impl Progress {
    #[inline(always)]
    pub fn c(self, multiplier: u8, extra: isize) -> usize {
        self.pos
            .saturating_add_signed(extra)
            .saturating_sub_signed(self.delay * isize::from(multiplier) / 2)
            .clamp(self.begin, self.end)
    }

    #[inline(always)]
    pub fn normalize(&mut self, multiplier: u8, extra: isize) -> bool {
        let new_pos = self.c(multiplier, extra);
        let eq = self.pos == new_pos;
        self.pos = new_pos;
        self.delay = 0;
        !eq
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProgressAccess {
    pub multiplier: *const u8,
    pub progress: *const Progress,
    pub duration: usize,
    pub size_per_second: usize,
}

unsafe impl Send for ProgressAccess {}

impl ProgressAccess {
    #[inline]
    fn i(mut num: usize, den: usize) -> String {
        use fmt::Write;

        let mut ret = String::with_capacity(10); // 12:34.567\0
        let min = num / (den * 60);
        let _ = write!(&mut ret, "{min}:");
        num %= den * 60;
        let sec = num / den;
        let _ = write!(&mut ret, "{sec:02}.");
        num %= den;
        let ms = num * 1000 / den;
        let _ = write!(&mut ret, "{ms:03}");
        ret
    }

    #[inline(always)]
    pub fn c(self) -> usize {
        unsafe { (*self.progress).c(*self.multiplier, 0) }
    }

    #[inline(always)]
    pub fn p(self) -> usize {
        (((self.c() as u64) << 20) / self.duration as u64) as usize
    }

    #[inline(always)]
    pub fn l(self) -> String {
        Self::i(self.c(), self.size_per_second)
    }

    #[inline(always)]
    pub fn n(self) -> String {
        Self::i(self.duration, self.size_per_second)
    }
}

#[inline(always)]
/// It returns the same value for a pair of (tx, rx), returns different value for different pairs.
pub const fn get_channel_handle<T>(chan: *const T) -> Handle {
    unsafe { hack::get_channel_handle_inner(chan.cast()) }
}

mod hack {
    use core::fmt;

    /// corresponding [`std::sync::mpmc::counter::Counter<Channel<T>>`]
    #[repr(transparent)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Handle(*const ());

    impl fmt::Debug for Handle {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Pointer::fmt(&self.0, f)
        }
    }

    impl fmt::Display for Handle {
        #[inline]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Pointer::fmt(&self.0, f)
        }
    }

    unsafe impl Send for Handle {}

    impl Handle {
        pub const NONE: Self = Self(core::ptr::null());
    }

    pub const unsafe fn get_channel_handle_inner(thing: *const Handle) -> Handle {
        unsafe { *thing.add(1) }
    }
}
