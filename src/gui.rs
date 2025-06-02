use core::{cell::SyncUnsafeCell, ffi::CStr, fmt, ptr::NonNull, time::Duration};
use std::{
    io::{self, SeekFrom},
    sync::mpsc::Sender,
    thread::sleep,
    time::Instant,
};

use lvgl::{
    CoreError, Display, Event, LvResult, Obj, Widget,
    timer::LvClock,
    widgets::{Btn, List},
};

use crate::util::{MP3Event, PlayerEvent};

struct Clock {
    start: Instant,
}

impl LvClock for Clock {
    fn since_init(&self) -> Duration {
        self.start.elapsed()
    }
}

pub fn cvt_lvgl_err<E: fmt::Debug>(err: E) -> io::Error {
    let wrapper = fmt::from_fn(|fmt| fmt::Debug::fmt(&err, fmt));
    std::io::Error::other(wrapper.to_string())
}

pub struct GUI {
    tx: Sender<MP3Event>,
    window: Display,
    screen: Obj,
}

unsafe impl Send for GUI {}

static TX_ONLY_USE_IT_FOR_CLOSE: SyncUnsafeCell<Option<Sender<MP3Event>>> = SyncUnsafeCell::new(None);

struct ConstDispatcher(Sender<MP3Event>, MP3Event);

impl FnOnce<(Btn, Event<()>)> for ConstDispatcher { type Output = (); extern "rust-call" fn call_once(self, args: (Btn, Event<()>)) { let _ = self.0.send(self.1); } }
impl FnMut<(Btn, Event<()>)> for ConstDispatcher { extern "rust-call" fn call_mut(&mut self, args: (Btn, Event<()>)) { let _ = self.0.send(self.1); } }
impl Fn<(Btn, Event<()>)> for ConstDispatcher { extern "rust-call" fn call(&self, args: (Btn, Event<()>)) { let _ = self.0.send(self.1); } }

impl GUI {
    extern "C" fn on_close(_: *mut lvgl_sys::lv_disp_t) -> bool {
        if let Some(tx) = unsafe { &*TX_ONLY_USE_IT_FOR_CLOSE.get() } {
            let _ = tx.send(MP3Event::Close);
        }
        true
    }

    pub fn new(tx: Sender<MP3Event>) -> Result<Self, CoreError> {
        const HORIZONTAL: i16 = 320;
        const VERTICAL: i16 = 240;
        const TITLE: &CStr = c"Music Player";

        unsafe { lvgl_sys::lv_wayland_init(); }

        unsafe { &mut *TX_ONLY_USE_IT_FOR_CLOSE.get() }.replace(tx.clone());
        let window = unsafe { lvgl_sys::lv_wayland_create_window(HORIZONTAL, VERTICAL, TITLE.as_ptr().cast_mut(), Some(Self::on_close)) };
        let screen = unsafe { lvgl_sys::lv_disp_get_scr_act(window) };

        let window = Display::from_raw(NonNull::new(window).ok_or(CoreError::ResourceNotAvailable)?, None);
        let screen = Obj::from_raw(NonNull::new(screen).ok_or(CoreError::ResourceNotAvailable)?);

        Ok(Self { tx, window, screen })
    }

    pub fn draw(&self) -> LvResult<()> {
        let list = List::new()?;

        let mut last_song = Btn::new()?;
        last_song.on_event(ConstDispatcher(self.tx.clone(), MP3Event::SwitchSong { seek: SeekFrom::Current(-1) }))?;

        let mut next_song = Btn::new()?;
        next_song.on_event(ConstDispatcher(self.tx.clone(), MP3Event::SwitchSong { seek: SeekFrom::Current(1) }))?;

        let mut fast_rewind = Btn::new()?;
        fast_rewind.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::Move { offset: -5 }.into()))?;

        let mut fast_forward = Btn::new()?;
        fast_rewind.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::Move { offset: 5 }.into()))?;

        let mut pause = Btn::new()?;
        pause.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::Pause.into()))?;

        let mut resume = Btn::new()?;
        resume.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::Resume.into()))?;

        // let mut speeds = Vec::new();
        for multiplier in 1..=4 {
            let mut speed = Btn::new()?;
            speed.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::SetMultiplier { multiplier }.into()))?;
            // speeds.push(speed);
        }

        // let mut vols = Vec::new();
        for volume in 0..=4 {
            let mut vol = Btn::new()?;
            vol.on_event(ConstDispatcher(self.tx.clone(), MP3Event::SetVolume { volume }))?;
            // vols.push(vol);
        }

        Ok(())
    }

    pub fn main_loop(self) {
        const TICK: Duration = Duration::from_millis(5);

        let clock = Clock { start: Instant::now() };
        while unsafe { lvgl_sys::lv_wayland_window_is_open(self.window.disp.as_ptr()) } {
            lvgl::task_handler();

            sleep(TICK);
            unsafe { lvgl::timer::update_clock(&clock).unwrap_unchecked(); }
        }

        let _ = self.tx.send(MP3Event::Close); // send multiple times does not matter
    }
}
