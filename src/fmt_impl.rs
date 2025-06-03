use alsa::pcm::{Format, IoFormat};

#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct S18_3([u8; 3]);

impl IoFormat for S18_3 {
    const FORMAT: Format = Format::S183LE;
}

#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct S20_3([u8; 3]);

impl IoFormat for S20_3 {
    const FORMAT: Format = Format::S203LE;
}

#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct S24_3([u8; 3]);

impl IoFormat for S24_3 {
    const FORMAT: Format = Format::S243LE;
}

#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct S20_4(i32);

impl IoFormat for S20_4 {
    const FORMAT: Format = Format::S20LE;
}

#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct S24_4(i32);

impl IoFormat for S24_4 {
    const FORMAT: Format = Format::S24LE;
}

pub trait Fmt: IoFormat + Default {
    fn to_f64(self) -> f64;
    fn from_f64(f: f64) -> Self;
}

macro_rules! impl_simple {
    ($($t:ty),+) => {
        $(
            impl Fmt for $t {
                #[inline(always)]
                fn to_f64(self) -> f64 {
                    self.into()
                }

                #[inline(always)]
                fn from_f64(f: f64) -> Self {
                    f as Self
                }
            }
        )+
    };
}

macro_rules! impl_3_bytes {
    ($t:ty, $shift_amt:literal) => {
        impl Fmt for $t {
            #[inline]
            fn to_f64(self) -> f64 {
                let i32 = i32::from_le_bytes([self.0[0], self.0[1], self.0[2], 0]);
                f64::from(i32 << const { 32 - $shift_amt } >> const { 32 - $shift_amt })
            }

            #[inline]
            fn from_f64(f: f64) -> Self {
                let i32_bytes = i32::to_le_bytes((f as i32) << const { 32 - $shift_amt } >> const { 32 - $shift_amt });
                Self([i32_bytes[0], i32_bytes[1], i32_bytes[2]])
            }
        }
    };
}

macro_rules! impl_4_bytes {
    ($t:ty, $shift_amt:literal) => {
        impl Fmt for $t {
            #[inline]
            fn to_f64(self) -> f64 {
                f64::from(self.0 << const { 32 - $shift_amt } >> const { 32 - $shift_amt })
            }

            #[inline]
            fn from_f64(f: f64) -> Self {
                Self((f as i32) << const { 32 - $shift_amt } >> const { 32 - $shift_amt })
            }
        }
    };
}

impl_simple!(i8, i16, i32, f32, f64);
impl_3_bytes!(S18_3, 18);
impl_3_bytes!(S20_3, 20);
impl_3_bytes!(S24_3, 24);
impl_4_bytes!(S20_4, 20);
impl_4_bytes!(S24_4, 24);
