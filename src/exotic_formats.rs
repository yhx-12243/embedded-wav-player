use alsa::pcm::{Format, IoFormat};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct S24_3([u8; 3]);

impl IoFormat for S24_3 {
    const FORMAT: Format = Format::S243LE;
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct S24_4(u32);

impl IoFormat for S24_4 {
    const FORMAT: Format = Format::S24LE;
}
