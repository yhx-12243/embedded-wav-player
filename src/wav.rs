use core::{any::type_name, hint::unlikely};
use std::{
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    sync::mpsc::{Receiver, RecvError, Sender, TryRecvError},
};

use alsa::{
    Direction, PCM, ValueOr,
    pcm::{Access, Format, HwParams, IO, State},
};
use hound::WavReader;

use crate::{
    fmt_impl::{Fmt, S18_3, S20_3, S20_4, S24_3, S24_4},
    shift,
    util::{
        GUIEvent, Handle, MP3Event, PlayError, PlayerEvent, Progress, ProgressAccess,
        UnsupportedFormatError, buffer_resize, cvt_format, get_channel_handle,
    },
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
    progress: Progress,
}

impl<R> Player<R>
where
    R: io::Read,
{
    pub fn new(reader: WavReader<R>, multiplier: u8) -> Result<Self, PlayError> {
        let format = cvt_format(reader.spec())?;
        Ok(Self { reader, format, multiplier, progress: Progress::default() })
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

struct EndReporter {
    mtx: Sender<MP3Event>,
    gtx: Sender<GUIEvent>,
    handle: Handle,
}

impl Drop for EndReporter {
    fn drop(&mut self) {
        match self.mtx.send(MP3Event::PlayerEnd { player: self.handle }) {
            Ok(()) => tracing::info!("Player (with handle \x1b[33m{}\x1b[0m) ends.", self.handle),
            Err(e) => tracing::warn!("Failed to send end event: {e}"),
        }
        if let Err(e) = self.gtx.send(GUIEvent::ProgressAccess { access: None, handle: self.handle }) {
            tracing::warn!("Failed to clear progress access: {e}");
        }
    }
}

impl<R> Player<BufReader<R>>
where
    R: io::Read + io::Seek,
{
    pub fn play(&mut self, mtx: Sender<MP3Event>, gtx: Sender<GUIEvent>, rx: Receiver<PlayerEvent>) -> Result<(), PlayError> {
        let handle = get_channel_handle(&raw const rx);
        let _end_reporter = EndReporter { mtx, gtx: gtx.clone(), handle };

        let pcm = self.configure_pcm()?;

        match self.format {
            Format::S8 => self.play_inner::<i8>(pcm, gtx, rx),
            Format::S16LE => self.play_inner::<i16>(pcm, gtx, rx),
            Format::S183LE => self.play_inner::<S18_3>(pcm, gtx, rx),
            Format::S203LE => self.play_inner::<S20_3>(pcm, gtx, rx),
            Format::S243LE => self.play_inner::<S24_3>(pcm, gtx, rx),
            Format::S20LE => self.play_inner::<S20_4>(pcm, gtx, rx),
            Format::S24LE => self.play_inner::<S24_4>(pcm, gtx, rx),
            Format::S32LE => self.play_inner::<i32>(pcm, gtx, rx),
            Format::FloatLE => self.play_inner::<f32>(pcm, gtx, rx),
            Format::Float64LE => self.play_inner::<f64>(pcm, gtx, rx),
            _ => return Err(PlayError::Format(UnsupportedFormatError(self.reader.spec())))
        }
    }

    fn play_inner<S: Fmt>(&mut self, pcm: PCM, gtx: Sender<GUIEvent>, rx: Receiver<PlayerEvent>) -> Result<(), PlayError> {
        const SAMPLE_SIZE_TOO_LARGE: io::Error = io::const_error!(io::ErrorKind::InvalidInput, "sample size too large");
        const WRITE_ZERO: io::Error = io::const_error!(io::ErrorKind::WriteZero, "fail to write audio");
        const INVALID_RET: io::Error = io::const_error!(io::ErrorKind::InvalidInput, "invalid return values");
        const SIZE_MISMATCH: io::Error = io::const_error!(io::ErrorKind::InvalidInput, "sample size mismatch");

        let handle = get_channel_handle(&raw const rx);
        let spec = self.reader.spec();

        if usize::from(spec.bytes_per_sample) != size_of::<S>() || S::FORMAT != self.format {
            return Err(SIZE_MISMATCH.into());
        }

        let sample_size = usize::from(
            spec.bytes_per_sample.checked_mul(spec.channels)
                .map_or(const { Err(PlayError::Io(SAMPLE_SIZE_TOO_LARGE)) }, Ok)?
        );
        let num_samples = self.reader.len();
        let size_per_second = sample_size * spec.sample_rate as usize;

        let reader = unsafe { self.reader.as_mut_inner() };

        self.progress.begin = reader.seek(SeekFrom::Current(0))? as usize; // 重置 reader 指针并清空缓存
        self.progress.end = self.progress.begin + spec.bytes_per_sample as usize * num_samples as usize;
        self.progress.pos = self.progress.begin;
        self.progress.delay = 0;

        let buf_size_8 = 2 * shift::MAX_BUFFER_SIZE * sample_size;
        let buf_size = 2 * shift::MAX_BUFFER_SIZE * usize::from(spec.channels);
        buffer_resize(reader, buf_size_8);

        let io = IO::<S>::new(&pcm);
        let mut v = unsafe { Box::<[S]>::new_zeroed_slice(buf_size).assume_init() };
        let mut w = unsafe { Box::<[S]>::new_zeroed_slice(buf_size).assume_init() };
        let mut w_b;
        let mut w_e;

        let _ = gtx.send(GUIEvent::ProgressAccess {
            access: Some(ProgressAccess {
                multiplier: &raw const self.multiplier,
                progress: &raw const self.progress,
                duration: spec.bytes_per_sample as usize * num_samples as usize,
                size_per_second,
            }),
            handle,
        });

        loop {
            let e = rx.recv()?;
            tracing::info!("⟨\x1b[33m{handle}\x1b[0m, \x1b[35mStopping\x1b[0m at \x1b[36m{}/{}\x1b[0m⟩ Receive event \x1b[33m{e:?}\x1b[0m", self.progress.pos - self.progress.begin, self.progress.end - self.progress.begin);
            match e {
                PlayerEvent::Terminate => return Ok(()),
                PlayerEvent::Move { offset } => {
                    if self.progress.normalize(self.multiplier, offset * size_per_second.cast_signed()) {
                        reader.seek(SeekFrom::Start(self.progress.pos as u64))?;
                    }
                    continue;
                }
                PlayerEvent::SetMultiplier { multiplier } => {
                    self.multiplier = multiplier;
                    continue;
                }
                PlayerEvent::Pause => continue,
                PlayerEvent::Resume => {
                    w_b = 0;
                    w_e = 0;
                }
            }
            loop {
                if let Ok(delay) = pcm.delay() {
                    self.progress.delay = delay as isize * sample_size.cast_signed();
                }
                match rx.try_recv() {
                    Ok(e) => {
                        tracing::info!("⟨\x1b[33m{handle}\x1b[0m, \x1b[35mPlaying\x1b[0m at \x1b[36m{} ({:+})/{}\x1b[0m⟩ Receive event \x1b[33m{e:?}\x1b[0m", self.progress.pos - self.progress.begin, -self.progress.delay, self.progress.end - self.progress.begin);
                        match e {
                            PlayerEvent::Terminate => return Ok(()),
                            PlayerEvent::Move { offset } => {
                                if let Err(e) = pcm.drop() { tracing::warn!("drop: {e}"); }
                                if let Err(e) = pcm.prepare() { tracing::warn!("prepare: {e}"); }
                                if self.progress.normalize(self.multiplier, offset * size_per_second.cast_signed()) {
                                    reader.seek(SeekFrom::Start(self.progress.pos as u64))?;
                                }
                            }
                            PlayerEvent::SetMultiplier { multiplier } => {
                                if let Err(e) = pcm.drop() { tracing::warn!("drop: {e}"); }
                                if let Err(e) = pcm.prepare() { tracing::warn!("prepare: {e}"); }
                                if self.multiplier != multiplier {
                                    w_b = 0;
                                    w_e = 0;
                                    if self.progress.normalize(self.multiplier, 0) {
                                        reader.seek(SeekFrom::Start(self.progress.pos as u64))?;
                                    }
                                    self.multiplier = multiplier;
                                }
                            }
                            PlayerEvent::Pause => {
                                if let Err(e) = pcm.drop() { tracing::warn!("drop: {e}"); }
                                if let Err(e) = pcm.prepare() { tracing::warn!("prepare: {e}"); }
                                // w_b = 0;
                                // w_e = 0;
                                if self.progress.normalize(self.multiplier, 0) {
                                    reader.seek(SeekFrom::Start(self.progress.pos as u64))?;
                                }
                                break;
                            }
                            PlayerEvent::Resume => (),
                        }
                    }
                    Err(TryRecvError::Empty) => (),
                    Err(TryRecvError::Disconnected) => return Err(RecvError.into()),
                }

                // 还有没写完的，先写
                if w_b != w_e {
                    let expected = (w_e - w_b) / usize::from(spec.channels);
                    let real = match io.writei(&w[w_b..w_e]) {
                        Ok(s) => s,
                        Err(e) if io::Error::from_raw_os_error(e.errno()).kind() == io::ErrorKind::BrokenPipe => {
                            if let Err(e) = pcm.prepare() { tracing::warn!("play-prepare: {e}"); }
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    };
                    if real == 0 {
                        continue;
                    } else if real < expected { // print a warning
                        tracing::warn!("Not fully written. {real}/{expected} {}'s written.", type_name::<S>());
                    } else if real > expected {
                        return Err(PlayError::Io(INVALID_RET));
                    }

                    w_b += real * usize::from(spec.channels);
                    continue;
                }

                let d_size = shift::buffer_size(self.multiplier) * usize::from(spec.channels);
                let d_size_8 = shift::buffer_size(self.multiplier) * sample_size;
                let buf = reader.peek(buf_size_8)?;

                let l = buf.len();
                if l == 0 {
                    if unlikely(self.progress.pos != self.progress.end) {
                        return Err(INVALID_RET.into());
                    }

                    if pcm.state() == State::Running {
                        core::hint::spin_loop();
                        continue;
                    }

                    return Ok(());
                }

                let consume_in;
                if l < d_size_8 {
                    let v8 = unsafe { core::slice::from_raw_parts_mut(v.as_mut_ptr().cast(), d_size_8) };

                    if unlikely(self.progress.pos + l != self.progress.end) {
                        return Err(INVALID_RET.into());
                    }

                    v8[..l].copy_from_slice(buf);
                    v8[l..].fill(0);

                    (consume_in, w_e) = shift::process(&v[..d_size], usize::from(spec.channels), self.multiplier, &mut w);
                    let consume = l.min(consume_in * size_of::<S>());
                    reader.consume(consume);
                    self.progress.pos += consume;
                } else {
                    let reinterpret = unsafe { core::slice::from_raw_parts(buf.as_ptr().cast(), buf.len() / size_of::<S>()) };

                    (consume_in, w_e) = shift::process(reinterpret, usize::from(spec.channels), self.multiplier, &mut w);
                    reader.consume(consume_in * size_of::<S>());
                    self.progress.pos += consume_in * size_of::<S>();
                }

                w_b = 0;
                // 直接去下一个循环写
            }
        }
    }
}
