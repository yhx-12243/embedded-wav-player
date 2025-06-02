use std::{
    fs, io,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, channel},
};

use alsa::{Mixer, mixer::SelemId};
use hound::{WavReader, WavSpec};

use crate::{
    util::{Handle, MP3Event, MP3EventPayload, PlayerEvent, cvt_err, get_channel_handle},
    wav::Player,
};

#[derive(Clone)]
pub struct Song {
    path: PathBuf,
    spec: WavSpec,
    num_samples: u32,
}

impl Song {
    pub fn load(path: PathBuf) -> Result<Self, PathBuf> {
        let Ok(tmp_reader) = WavReader::open(&path) else { return Err(path) };
        let spec = tmp_reader.spec();
        let num_samples = tmp_reader.len();
        Ok(Self {
            path,
            spec,
            num_samples,
        })
    }
}

pub struct MP3 {
    songs: Vec<Song>,
    current_idx: usize,
    multiplier: u8, // 倍速 * 0.5
    tx: Option<Sender<PlayerEvent>>,
    pub mtx: Sender<MP3Event>,
    mrx: Receiver<MP3Event>,
}

impl Drop for MP3 {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(PlayerEvent::Terminate);
        }
    }
}

impl MP3 {
    pub fn load(dir: PathBuf) -> io::Result<Self> {
        const NO_SONGS_FOUND: io::Error = io::const_error!(
            io::ErrorKind::NotFound,
            "No songs found in the specified directory"
        );

        let mut songs = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            match Song::load(path) {
                Ok(song) => {
                    tracing::info!("\x1b[32m{}\x1b[0m WAV sanity check passed (spec={:?}, num_samples={}).", song.path.display(), song.spec, song.num_samples);
                    songs.push(song);
                }
                Err(path) => tracing::info!("\x1b[33m{}\x1b[0m is not a WAV file, skipped.", path.display()),
            }
        }
        if songs.is_empty() { return Err(NO_SONGS_FOUND); }
        tracing::info!("successfully load {} songs.", songs.len());

        let (mtx, mrx) = channel();
        Ok(Self {
            songs,
            current_idx: usize::MAX,
            multiplier: 2,
            tx: None,
            mtx,
            mrx,
        })
    }

    pub fn set_volume(volume: u8) -> alsa::Result<()> {
        // 打开混音器
        let mixer = Mixer::new("default", false)?;

        // 获取第一个混音器元素
        let selem_id = SelemId::new("Master", 0);
        let elem = mixer.find_selem(&selem_id).ok_or_else(|| alsa::Error::new(
            "set_volume: Mixer element not found",
            -1, // 使用 alsa::Error::UNKNOWN 或其他错误码
        ))?;

        // 获取音量范围
        // let (min, max) = elem.get_playback_volume_range();

        // 计算实际音量值 (0-4 映射到 0-512)
        let volume_value = i64::from(volume) * 128;

        // 设置所有通道的音量
        elem.set_playback_volume_all(volume_value)?;

        Ok(())
    }

    pub fn switch_song(&mut self, idx: usize) -> io::Result<()> {
        const OUT_OF_BOUNDS: io::Error = io::const_error!(io::ErrorKind::NotFound, "Song index out of bounds");

        if idx == self.current_idx {
            return Ok(());
        }

        let song = self.songs.get(idx).ok_or(OUT_OF_BOUNDS)?;
        let mut player = Player::new(
            WavReader::open(&song.path).map_err(cvt_err)?,
            self.multiplier,
        )?;

        if let Some(tx) = self.tx.take() {
            let _ = tx.send(PlayerEvent::Terminate);
        }

        let (tx, rx) = channel();
        self.current_idx = idx;
        self.tx = Some(tx);

        tracing::info!("switch to song #{idx}: \x1b[36m{}\x1b[0m", song.path.file_name().unwrap_or(song.path.as_os_str()).display());
        {
            let tx = self.mtx.clone();
            std::thread::spawn(move || player.play(tx, rx).unwrap());
        }

        Ok(())
    }

    const fn get_current_handle(&self) -> Handle {
        if let Some(tx) = &self.tx {
            get_channel_handle(core::ptr::from_ref(tx))
        } else {
            Handle::NONE
        }
    }

    pub fn main_loop(mut self) -> io::Result<()> {
        self.switch_song(0)?;

        loop {
            match self.mrx.recv() {
                Ok(MP3Event { handle, payload: MP3EventPayload::PlayerEnd }) => {
                    let cur_handle = self.get_current_handle();
                    if cur_handle == handle {
                        tracing::info!("song #{} play finished, switch to next song.", self.current_idx);
                        self.switch_song((self.current_idx + 1) % self.songs.len())?;
                        if let Some(tx) = &self.tx {
                            let _ = tx.send(PlayerEvent::Resume);
                        }
                    } else {
                        tracing::info!("Stale end event: cur_handle = {cur_handle}, event_handle = {handle}");
                    }
                }
                Ok(MP3Event { payload: MP3EventPayload::Close, .. }) => {
                    tracing::info!("Received close event, exiting main loop.");
                    return Ok(());
                }
                Err(e) => return Err(io::Error::other(e)),
            }
        }
    }
}
