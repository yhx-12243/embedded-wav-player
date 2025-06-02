use core::hint::unlikely;
use std::{
    io::{self, BufRead, BufReader, Seek, SeekFrom, Write},
    sync::mpsc::{Receiver, RecvError, Sender, TryRecvError},
};

use alsa::{
    Direction, PCM, ValueOr,
    pcm::{Access, Format, HwParams, IO},
};
use hound::WavReader;

use crate::util::{
    Handle, MP3Event, MP3EventPayload, PlayError, PlayerEvent, buffer_resize_if_need, cvt_format,
    get_channel_handle,
};

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
    multiplier: u8, // 倍速 * 0.5
}

impl<R> Player<R>
where
    R: io::Read,
{
    pub fn new(reader: WavReader<R>, multiplier: u8) -> Result<Self, PlayError> {
        let format = cvt_format(reader.spec())?;
        Ok(Self { reader, format, multiplier })
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

struct EndReporter(Sender<MP3Event>, Handle);

impl Drop for EndReporter {
    fn drop(&mut self) {
        match self.0.send(MP3Event { handle: self.1, payload: MP3EventPayload::PlayerEnd }) {
            Ok(()) => tracing::info!("Player (with handle \x1b[33m{}\x1b[0m) ends.", self.1),
            Err(e) => tracing::error!("Failed to send end event: {e}"),
        }
    }
}

impl<R> Player<BufReader<R>>
where
    R: io::Read + io::Seek,
{
    pub fn play(&mut self, tx: Sender<MP3Event>, rx: Receiver<PlayerEvent>) -> Result<(), PlayError> {
        const SAMPLE_SIZE_TOO_LARGE: io::Error = io::const_error!(io::ErrorKind::InvalidInput, "sample size too large");
        const WRITE_ZERO: io::Error = io::const_error!(io::ErrorKind::WriteZero, "fail to write audio");
        const INVALID_RET: io::Error = io::const_error!(io::ErrorKind::InvalidInput, "invalid return values");

        let handle = get_channel_handle(&raw const rx);
        let _end_reporter = EndReporter(tx, handle);

        let pcm = self.configure_pcm()?;

        let sample_size = self.reader.spec().bytes_per_sample
            .checked_mul(self.reader.spec().channels)
            .map_or(const { Err(PlayError::Io(SAMPLE_SIZE_TOO_LARGE)) }, Ok)?
            .into();
        let sample_rate = self.reader.spec().sample_rate;
        let num_samples = self.reader.len();
        let size_per_second = (sample_size as u64 * u64::from(sample_rate)).cast_signed();

        let reader = unsafe { self.reader.as_mut_inner() };
        #[allow(clippy::seek_from_current)]
        let begin = reader.seek(SeekFrom::Current(0))?; // 重置 reader 指针并清空缓存
        let end = begin + sample_size as u64 * u64::from(num_samples);
        let mut pos = begin;

        buffer_resize_if_need(sample_size, reader);

        let mut io = IO::<()>::new(&pcm);
        let mut v = vec![0; sample_size];

        loop {
            let e = rx.recv()?;
            tracing::info!("Receive event \x1b[33m{e:?}\x1b[0m [Stopping]");
            match e {
                PlayerEvent::Terminate => return Ok(()),
                PlayerEvent::Move { offset } => {
                    let new_pos = pos.saturating_add_signed(offset as i64 * size_per_second).clamp(begin, end);
                    if pos != new_pos {
                        pos = new_pos;
                        reader.seek(SeekFrom::Start(pos))?;
                    }
                    continue;
                }
                PlayerEvent::SetMultiplier { multiplier } => {
                    self.multiplier = multiplier;
                    continue;
                }
                PlayerEvent::Pause => continue,
                PlayerEvent::Resume => (),
            }
            loop {
                match rx.try_recv() {
                    Ok(e) => {
                        tracing::info!("Receive event \x1b[33m{e:?}\x1b[0m [Playing]");
                        match e {
                            PlayerEvent::Terminate => return Ok(()),
                            PlayerEvent::Move { offset } => {
                                let new_pos = pos.saturating_add_signed(offset as i64 * size_per_second).clamp(begin, end);
                                if pos != new_pos {
                                    pos = new_pos;
                                    reader.seek(SeekFrom::Start(pos))?;
                                }
                            }
                            PlayerEvent::SetMultiplier { multiplier } => self.multiplier = multiplier,
                            PlayerEvent::Pause => break,
                            PlayerEvent::Resume => (),
                        }
                    }
                    Err(TryRecvError::Empty) => (),
                    Err(TryRecvError::Disconnected) => return Err(RecvError.into()),
                }
                let buf = reader.fill_buf()?;
                if buf.is_empty() {
                    return pcm.drain().map_err(Into::into);
                }
                let l = buf.len();

                if unlikely(l < sample_size) {
                    v[..l].copy_from_slice(buf);
                    reader.consume(l);
                    pos += l as u64;
                    reader.get_mut().read_exact(&mut v[l..])?;
                    match io.write(&v)? {
                        0 => return Err(PlayError::Io(WRITE_ZERO)),
                        x if x == sample_size => continue,
                        _ => return Err(PlayError::Io(INVALID_RET)),
                    }
                }

                let expected = l;
                let real = io.write(buf)?;
                if real == 0 {
                    return Err(PlayError::Io(WRITE_ZERO));
                } else if real < expected { // print a warning
                    println!("Not fully written. {real}/{expected} bytes written.");
                } else if real > expected {
                    return Err(PlayError::Io(INVALID_RET));
                }

                reader.consume(real);
                pos += real as u64;
            }
        }
    }
}
