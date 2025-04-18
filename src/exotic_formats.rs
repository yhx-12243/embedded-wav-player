use alsa::pcm::{Format, IoFormat};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct S18_3([u8; 3]);

impl IoFormat for S18_3 {
    const FORMAT: Format = Format::S183LE;
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct S20_3([u8; 3]);

impl IoFormat for S20_3 {
    const FORMAT: Format = Format::S203LE;
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct S24_3([u8; 3]);

impl IoFormat for S24_3 {
    const FORMAT: Format = Format::S243LE;
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct S20_4(u32);

impl IoFormat for S20_4 {
    const FORMAT: Format = panic!("waiting for https://github.com/diwic/alsa-rs/pull/133"); // Format::S20LE;
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct S24_4(u32);

impl IoFormat for S24_4 {
    const FORMAT: Format = Format::S24LE;
}
