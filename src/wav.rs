use core::hint::unlikely;
use std::io::{self, BufRead, BufReader, Write};

use alsa::{
    Direction, PCM, ValueOr,
    pcm::{Access, Format, HwParams, IoFormat},
};
use hound::WavReader;

use crate::util::{UnsupportedFormatError, cvt_format, read_surplus};

pub fn dump_header<R>(reader: &WavReader<R>)
where
    R: io::Read,
{
    println!("RIFF 标志：RIFF");
    println!("文件大小：{}", reader.len() * u32::from(reader.spec().bytes_per_sample));
    println!("文件格式：WAVE");
    println!("格式块标识：fmt");
    println!("格式块长度：16");
    println!("编码格式：{:?}", reader.spec().sample_format);
    println!("声道数：{}", reader.spec().channels);
    println!("采样频率：{} Hz", reader.spec().sample_rate);
    let block_align = u32::from(reader.spec().bytes_per_sample) * u32::from(reader.spec().channels);
    println!("传输速率：{} B/s", reader.spec().sample_rate * block_align);
    println!("数据块对齐单位：{block_align} B/block");
    println!("采样位数：{} bit", reader.spec().bits_per_sample);
}

pub struct Player<R> {
    reader: WavReader<R>,
    format: Format,
}

impl<R> Player<R>
where
    R: io::Read,
{
    pub fn new(reader: WavReader<R>) -> Result<Self, UnsupportedFormatError> {
        let format = cvt_format(reader.spec())?;
        Ok(Self { reader, format })
    }

    fn configure_pcm(&self) -> Result<PCM, alsa::Error> {
        let header = self.reader.spec();

        // 打开 PCM 设备，分配 snd_pcm_hw_params_t 结构体，配置空间初始化
        let pcm = PCM::new("default", Direction::Playback, false)?;

        let params = HwParams::any(&pcm)?;

        // 设置交错模式 (访问模式)
        params.set_access(Access::RWInterleaved)?;

        // 设置样本长度 (位数)
        params.set_format(self.format)?;

        // 设置采样率
        params.set_rate_near(header.sample_rate, ValueOr::Nearest)?;

        // 设置通道数
        params.set_channels(header.channels.into())?;

        pcm.hw_params(&params)?;
        drop(params);

        // 设置硬件配置参数
        pcm.prepare()?;

        Ok(pcm)
    }
}

pub enum PlayError {
    Alsa(alsa::Error),
    Format(UnsupportedFormatError),
    Io(io::Error),
}

impl From<alsa::Error> for PlayError {
    fn from(err: alsa::Error) -> Self {
        Self::Alsa(err)
    }
}

impl From<UnsupportedFormatError> for PlayError {
    fn from(err: UnsupportedFormatError) -> Self {
        Self::Format(err)
    }
}

impl From<io::Error> for PlayError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<PlayError> for io::Error {
    fn from(err: PlayError) -> Self {
        match err {
            PlayError::Alsa(e) => Self::other(e),
            PlayError::Format(e) => Self::other(e),
            PlayError::Io(e) => e,
        }
    }
}

impl<R> Player<BufReader<R>>
where
    R: io::Read,
{
    pub fn play(&mut self) -> Result<(), PlayError> {
        let pcm = self.configure_pcm()?;

        match self.format {
            Format::S8 => self.play_inner::<i8>(pcm),
            Format::S16LE => self.play_inner::<i16>(pcm),
            Format::S32LE => self.play_inner::<i32>(pcm),
            Format::FloatLE => self.play_inner::<f32>(pcm),
            Format::Float64LE => self.play_inner::<f64>(pcm),
            _ => return Err(PlayError::Format(UnsupportedFormatError(self.reader.spec())))
        }
    }

    pub fn play_inner<S: IoFormat>(&mut self, pcm: PCM) -> Result<(), PlayError> {
        let sample_size = size_of::<S>() * usize::from(self.reader.spec().channels);

        let reader = unsafe { self.reader.as_mut_inner() };

        let mut io = pcm.io_checked::<S>()?;

        loop {
            let buf = reader.fill_buf()?;
            if buf.is_empty() {
                return Ok(());
            }
            let rem = sample_size - buf.len() % sample_size;

            if unlikely(rem != sample_size) {
                let v = read_surplus(buf, rem, sample_size, reader)?;
                let mut buf = &*v;
                while !buf.is_empty() {
                    let expected = v.len();
                    let real = io.write(&v)?;
                    if real == 0 {
                        return Err(PlayError::Io(io::const_error!(io::ErrorKind::WriteZero, "fail to write audio")));
                    } else if real < expected { // print a warning
                        println!("(Buffered) Not fully written. {real}/{expected} bytes written.");
                    } else if real > expected {
                        return Err(PlayError::Io(io::const_error!(io::ErrorKind::InvalidInput, "invalid return values")));
                    }
                    buf = unsafe { buf.get_unchecked(real..) };
                }
                continue;
            }

            // should be almost always zero since cap % sample_size == 0
            let expected = buf.len();
            let real = io.write(buf)?;
            if real == 0 {
                return Err(PlayError::Io(io::const_error!(io::ErrorKind::WriteZero, "fail to write audio")));
            } else if real < expected { // print a warning
                println!("Not fully written. {real}/{expected} bytes written.");
            } else if real > expected {
                return Err(PlayError::Io(io::const_error!(io::ErrorKind::InvalidInput, "invalid return values")));
            }

            reader.consume(real);
        }
    }
}
