use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread::JoinHandle,
};

use hound::WavReader;

use crate::{
    util::{MP3Event, PlayerEvent, PlayerFeedback, cvt_err},
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
        let tmp_reader = WavReader::open(&path).map_err(cvt_err)?;
        let spec = tmp_reader.spec();
        let num_samples = tmp_reader.len();
        Ok(Song {
            path,
            spec,
            num_samples,
        })
    }
}

pub struct MP3 {
    songs: Vec<Song>,
    current_idx: usize,
    multipler: u8, // 倍速 * 0.5
    tx: Option<Sender<PlayerEvent>>,
    mtx: Sender<MP3Event>,
    mrx: Receiver<MP3Event>,
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
                    tracing::info!("\x1b[32m{}\x1b[0m WAV sanity check passed.", song.path.display());
                    songs.push(song);
                }
                Err(path) => tracing::info!("\x1b[33m{}\x1b[0m is not a WAV file, skipped.", path.display()),
            }
        }
        if songs.is_empty() {
            tracing::warn!("No songs founds at {dir}");
            return Err(NO_SONGS_FOUND);
        }
        tracing::info!("successfully load {} songs.", songs.len());

        let (mtx, mrx) = mpsc::channel();
        Ok(Self {
            songs,
            current_idx: usize::MAX,
            multipler: 2,
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
        const OUT_OF_BOUNDS: io::Error =
            io::const_error!(io::ErrorKind::NotFound, "Song index out of bounds");

        if idx == current_idx {
            return Ok(());
        }

        let song = self.songs.get(idx).ok_or(OUT_OF_BOUNDS)?;
        let player = Player::new(WavReader::open(song.path)?, self.multiplier).map_err(cvt_err)?;

        if let Some(tx) = self.tx.take() {
            let _ = tx.send(PlayerEvent::Terminiate);
        }

        let (tx, rx) = mpsc::channel();
        self.current_idx = idx;
        self.tx = Some(tx);

        std::thread::spawn(move || player.play(self.mtx.clone(), rx));

        Ok(())
    }

    pub fn start_loop(&mut self) -> io::Result<!> {
        self.switch_song(0)?;

        loop {
            match self.mrx.recv() {
                Ok(MP3Event::PlayerEnd) => {
                    self.switch_song((self.current_idx + 1) % self.songs.len())
                }
                Err(e) => return Err(io::Error::other(e)),
            }
        }
    }
}
