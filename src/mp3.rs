use core::hint::unlikely;
use std::{
    fs, io::{self, SeekFrom}, path::{Path, PathBuf}, sync::mpsc::{channel, Receiver, Sender}
};

use alsa::{Mixer, mixer::SelemId};
use hound::{WavReader, WavSpec};

use crate::{
    util::{GUIEvent, Handle, MP3Event, PlayerEvent, cvt_err, get_channel_handle},
    wav::Player,
};

#[derive(Clone)]
pub struct Song {
    path: PathBuf,
    spec: WavSpec,
    num_samples: u32,
}

impl Song {
    #[inline]
    pub fn get_path(&self) -> &Path {
        &self.path
    }

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
    mixer: Mixer,
    elem: *mut alsa_sys::snd_mixer_elem_t,
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
    #[inline]
    pub fn get_songs(&self) -> &[Song] {
        &self.songs
    }

    pub fn load(dir: PathBuf) -> io::Result<Self> {
        const NO_SONGS_FOUND: io::Error = io::const_error!(io::ErrorKind::NotFound, "No songs found in the specified directory");

        let mixer = Mixer::new("default", false).map_err(io::Error::other)?;

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
        tracing::info!("successfully load \x1b[36m{}\x1b[0m songs.", songs.len());
        songs.sort_unstable_by(|lhs, rhs| lhs.path.as_os_str().cmp(rhs.path.as_os_str()));

        let (mtx, mrx) = channel();
        Ok(Self {
            songs,
            current_idx: usize::MAX,
            multiplier: 2,
            mixer,
            elem: core::ptr::null_mut(),
            tx: None,
            mtx,
            mrx,
        })
    }

    pub fn set_volume(&mut self, volume: i32) -> alsa::Result<()> {
        const E: alsa::Error = alsa::Error::new("set_volume failed", -1);

        if unlikely(self.elem.is_null()) {
            let selem_id = SelemId::new("Master", 0);
            self.elem = self.mixer.find_selem(&selem_id).ok_or(E)?.handle;
        }

        let ret = unsafe { alsa_sys::snd_mixer_selem_set_playback_volume_all(self.elem, volume) };
        if ret < 0 {
            return Err(E);
        }

        tracing::info!("Set volume to {}%", volume as f64 * 0.1953125);
        Ok(())
    }

    fn switch_song(&mut self, idx: usize, gtx: Sender<GUIEvent>) -> io::Result<()> {
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
        let _ = gtx.send(GUIEvent::SwitchSong { index: idx, handle: get_channel_handle(&tx) });
        self.current_idx = idx;
        self.tx = Some(tx);

        tracing::info!("switch to song #{idx}: \x1b[36m{}\x1b[0m", song.path.file_name().unwrap_or(song.path.as_os_str()).display());
        {
            let mtx = self.mtx.clone();
            std::thread::spawn(move || player.play(mtx, gtx, rx).unwrap());
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

    pub fn main_loop(mut self, gtx: Sender<GUIEvent>) -> io::Result<()> {
        self.switch_song(0, gtx.clone())?;

        loop {
            match self.mrx.recv() {
                Ok(MP3Event::PlayerEnd { player }) => {
                    let cur_handle = self.get_current_handle();
                    if cur_handle == player {
                        tracing::info!("song #{} play finished, switch to next song.", self.current_idx);
                        self.switch_song((self.current_idx + 1) % self.songs.len(), gtx.clone())?;
                        if let Some(tx) = &self.tx {
                            let _ = tx.send(PlayerEvent::Resume);
                        }
                    } else {
                        tracing::info!("Stale end event: cur_handle = {cur_handle}, event_handle = {player}");
                    }
                }
                Ok(MP3Event::Close) => {
                    tracing::info!("Received close event, exiting main loop.");
                    return Ok(());
                }
                Ok(MP3Event::Dispatch { sub }) => {
                    tracing::info!("Received user event {sub:?}, dispatch to player.");
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(sub);
                    }
                }
                Ok(MP3Event::SwitchSong { seek }) => {
                    let idx = match seek {
                        SeekFrom::Start(idx) => (idx as usize) % self.songs.len(),
                        SeekFrom::Current(offset) => (self.current_idx.cast_signed() + offset as isize).rem_euclid(self.songs.len().cast_signed()).cast_unsigned(),
                        SeekFrom::End(offset) => (offset as isize).rem_euclid(self.songs.len().cast_signed()).cast_unsigned(),
                    };
                    self.switch_song(idx, gtx.clone())?;
                    if let Some(tx) = &self.tx {
                        let _ = tx.send(PlayerEvent::Resume);
                    }
                }
                Ok(MP3Event::SetVolume { volume }) => self.set_volume(volume).map_err(io::Error::other)?,
                Err(e) => return Err(io::Error::other(e)),
            }
        }
    }
}
