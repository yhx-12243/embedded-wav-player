use core::{cell::SyncUnsafeCell, ffi::CStr, fmt, ptr::NonNull, time::Duration};
use std::{
    borrow::Cow,
    ffi::CString,
    io::{self, SeekFrom},
    sync::mpsc::{Receiver, Sender},
    thread::sleep,
    time::Instant,
};

use lvgl::{
    Align, CoreError, Display, Event, LvError, LvResult, NativeObject, Obj, Widget,
    timer::LvClock,
    widgets::{Bar, Btn, Label, List, Slider},
};

use crate::{
    mp3::Song,
    util::{GUIEvent, Handle, MP3Event, PlayerEvent},
};

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
    song_labels: Vec<Label>,
    progress: Option<Bar>,
    pl: Option<Label>,
    pn: Option<Label>,
}

unsafe impl Send for GUI {}

static TX_ONLY_USE_IT_FOR_CLOSE: SyncUnsafeCell<Option<Sender<MP3Event>>> = SyncUnsafeCell::new(None);

struct ConstDispatcher(Sender<MP3Event>, MP3Event);

impl ConstDispatcher {
    fn dispatch(&self, event: Event<()>) {
        if event == Event::Clicked {
            let _ = self.0.send(self.1);
        }
    }
}

impl<W> FnOnce<(W, Event<()>)> for ConstDispatcher { type Output = (); #[inline] extern "rust-call" fn call_once(self, args: (W, Event<()>)) { self.dispatch(args.1); } }
impl<W> FnMut<(W, Event<()>)> for ConstDispatcher { #[inline] extern "rust-call" fn call_mut(&mut self, args: (W, Event<()>)) { self.dispatch(args.1); } }
impl<W> Fn<(W, Event<()>)> for ConstDispatcher { #[inline] extern "rust-call" fn call(&self, args: (W, Event<()>)) { self.dispatch(args.1); } }

extern "C" fn set_volume(event: *mut lvgl_sys::lv_event_t) {
    unsafe {
        let vol = (*event).target;
        let volume = lvgl_sys::lv_bar_get_value(vol);
        let tx = (*event).user_data as *const Sender<MP3Event>;
        let _ = (*tx).send(MP3Event::SetVolume { volume });
    }
}

impl GUI {
    extern "C" fn on_close(_: *mut lvgl_sys::lv_disp_t) -> bool {
        if let Some(tx) = unsafe { &*TX_ONLY_USE_IT_FOR_CLOSE.get() } {
            let _ = tx.send(MP3Event::Close);
        }
        true
    }

    pub fn new(tx: Sender<MP3Event>) -> Result<Self, CoreError> {
        const HORIZONTAL: i16 = 560;
        const VERTICAL: i16 = 320;
        const TITLE: &CStr = c"Music Player";

        unsafe { lvgl_sys::lv_wayland_init(); }

        unsafe { &mut *TX_ONLY_USE_IT_FOR_CLOSE.get() }.replace(tx.clone());
        let window_r = unsafe { lvgl_sys::lv_wayland_create_window(HORIZONTAL, VERTICAL, TITLE.as_ptr().cast_mut(), Some(Self::on_close)) };
        let screen_r = unsafe { lvgl_sys::lv_disp_get_scr_act(window_r) };

        let window = Display::from_raw(NonNull::new(window_r).ok_or(CoreError::ResourceNotAvailable)?, None);
        let screen = Obj::from_raw(NonNull::new(screen_r).ok_or(CoreError::ResourceNotAvailable)?);

        let main = unsafe { lvgl_sys::lv_win_create(screen_r, 20) };
        unsafe { lvgl_sys::lv_win_add_title(main, TITLE.as_ptr()) };

        Ok(Self {
            tx,
            window,
            screen,
            song_labels: Vec::new(),
            progress: None,
            pl: None,
            pn: None,
        })
    }

    fn set_label(btn: &mut Btn, content: Cow<str>) -> LvResult<()> {
        let mut lbl = Label::create(btn)?;
        match content {
            Cow::Borrowed(s) => lbl.set_text_static(CStr::from_bytes_with_nul(s.as_bytes()).map_err(|_| LvError::InvalidReference)?)?,
            Cow::Owned(s) => lbl.set_text(&CString::new(s).map_err(|_| LvError::InvalidReference)?)?,
        }
        lbl.set_align(Align::Center, 0, 0)
    }

    fn add_entry(list: &mut List, text: &[u8]) -> LvResult<Label> {
        let text = CString::new(text).map_err(|_| LvError::InvalidReference)?;
        let lbl = unsafe { lvgl_sys::lv_list_add_text(list.raw()?.as_ptr(), text.as_ptr()) };
        unsafe { lvgl_sys::lv_obj_add_flag(lbl, lvgl_sys::LV_OBJ_FLAG_CLICKABLE) };
        match NonNull::new(lbl) {
            Some(p) => Ok(Label::from_raw(p)),
            None => Err(LvError::InvalidReference),
        }
    }

    pub fn draw(&mut self, songs: &[Song], initial_volume: i32) -> LvResult<()> {
        let mut list = List::new()?;
        list.set_pos(340, 25)?;
        list.set_size(200, 270)?;
        for (i, song) in songs.iter().enumerate() {
            let p = song.get_path();
            let mut lbl = Self::add_entry(&mut list, p.file_name().unwrap_or(p.as_os_str()).as_encoded_bytes())?;
            lbl.on_event(ConstDispatcher(self.tx.clone(), MP3Event::SwitchSong { seek: SeekFrom::Start(i as u64) }))?;
            self.song_labels.push(lbl);
        }

        let mut last_song = Btn::new()?;
        last_song.set_pos(25, 245)?;
        last_song.set_size(40, 20)?;
        Self::set_label(&mut last_song, "\u{f048}\0" /* "⏮\0" */.into())?;
        last_song.on_event(ConstDispatcher(self.tx.clone(), MP3Event::SwitchSong { seek: SeekFrom::Current(-1) }))?;

        let mut next_song = Btn::new()?;
        next_song.set_pos(275, 245)?;
        next_song.set_size(40, 20)?;
        Self::set_label(&mut next_song, "\u{f051}\0" /* "⏭\0" */.into())?;
        next_song.on_event(ConstDispatcher(self.tx.clone(), MP3Event::SwitchSong { seek: SeekFrom::Current(1) }))?;

        let mut fast_rewind = Btn::new()?;
        fast_rewind.set_pos(75, 245)?;
        fast_rewind.set_size(40, 20)?;
        Self::set_label(&mut fast_rewind, "\u{f053}\0" /* "⏪\0" */.into())?;
        fast_rewind.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::Move { offset: -5 }.into()))?;

        let mut fast_forward = Btn::new()?;
        fast_forward.set_pos(225, 245)?;
        fast_forward.set_size(40, 20)?;
        Self::set_label(&mut fast_forward, "\u{f054}\0" /* "⏩\0" */.into())?;
        fast_forward.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::Move { offset: 5 }.into()))?;

        let mut pause = Btn::new()?;
        pause.set_pos(175, 245)?;
        pause.set_size(40, 20)?;
        Self::set_label(&mut pause, "\u{f04c}\0" /* "⏸\0" */.into())?;
        pause.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::Pause.into()))?;

        let mut resume = Btn::new()?;
        resume.set_pos(125, 245)?;
        resume.set_size(40, 20)?;
        Self::set_label(&mut resume, "\u{f04b}\0" /* "▶\0" */.into())?;
        resume.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::Resume.into()))?;

        // let mut speeds = Vec::new();
        for multiplier in 1..=4 {
            let mut speed = Btn::new()?;
            speed.set_pos(i16::from(multiplier) * 75 - 50, 275)?;
            speed.set_size(65, 20)?;
            Self::set_label(&mut speed, format!("{}x", f32::from(multiplier) * 0.5).into())?;
            speed.on_event(ConstDispatcher(self.tx.clone(), PlayerEvent::SetMultiplier { multiplier }.into()))?;
            // speeds.push(speed);
        }

        let mut vol = Slider::new()?;
        vol.set_pos(290, 25)?;
        vol.set_size(15, 150)?;
        let leaked_tx = Box::into_raw(Box::new(self.tx.clone()));
        unsafe {
            lvgl_sys::lv_bar_set_range(vol.raw()?.as_ptr(), 0, 512);
            lvgl_sys::lv_bar_set_value(vol.raw()?.as_ptr(), initial_volume, 0);
            // lvgl_sys::lv_obj_add_flag(vol.raw()?.as_ptr(), lvgl_sys::LV_OBJ_FLAG_CLICKABLE);
            lvgl_sys::lv_obj_add_event_cb(vol.raw()?.as_ptr(), Some(set_volume), lvgl_sys::lv_event_code_t_LV_EVENT_VALUE_CHANGED, leaked_tx.cast());
        }

        let mut progress = Bar::new()?;
        progress.set_pos(25, 195)?;
        progress.set_size(290, 15)?;
        unsafe { lvgl_sys::lv_bar_set_range(progress.raw()?.as_ptr(), 0, 0x10_0000); }
        self.progress = Some(progress);

        let mut pl = Label::new()?;
        pl.set_pos(25, 220)?;
        self.pl = Some(pl);

        let mut pn = Label::new()?;
        pn.set_pos(315, 220)?;
        self.pn = Some(pn);

        Ok(())
    }

    fn highlight(label: &Label) -> LvResult<()> {
        unsafe {
            lvgl_sys::lv_obj_set_style_bg_color(
                label.raw()?.as_ptr(),
                lvgl_sys::lv_palette_main(6),
                0,
            );
        }
        Ok(())
    }

    fn de_highlight(label: &Label) -> LvResult<()> {
        unsafe {
            lvgl_sys::lv_obj_remove_local_style_prop(
                label.raw()?.as_ptr(),
                lvgl_sys::lv_style_prop_t_LV_STYLE_BG_COLOR,
                0,
            );
        }
        Ok(())
    }

    pub fn main_loop(mut self, grx: Receiver<GUIEvent>) {
        const TICK: Duration = Duration::from_millis(5);

        let clock = Clock { start: Instant::now() };
        let mut pa = None;
        let mut last_index = usize::MAX;
        let mut cur_handle = Handle::NONE;
        while unsafe { lvgl_sys::lv_wayland_window_is_open(self.window.disp.as_ptr()) } {
            unsafe { lvgl_sys::lv_wayland_timer_handler(); }

            while let Ok(event) = grx.try_recv() {
                match event {
                    GUIEvent::SwitchSong { index, handle } => {
                        if let Some(l) = self.song_labels.get(last_index) {
                            let _ = Self::de_highlight(l);
                        }
                        if let Some(l) = self.song_labels.get(index) {
                            let _ = Self::highlight(l);
                        }
                        last_index = index;
                        cur_handle = handle;
                        pa = None;
                    }
                    GUIEvent::ProgressAccess { access, handle } => {
                        if cur_handle == handle {
                            pa = access;
                        }
                    }
                }
            }

            if let Some(access) = pa {
                if let Some(progress) = &mut self.progress && let Ok(progress) = progress.raw() {
                    unsafe { lvgl_sys::lv_bar_set_value(progress.as_ptr(), access.p() as i32, 0); }
                }
                if let Some(pl) = &mut self.pl {
                    let _ = pl.set_text(&unsafe { CString::from_vec_unchecked(access.l().into()) });
                }
                if let Some(pn) = &mut self.pn {
                    let _ = pn.set_text(&unsafe { CString::from_vec_unchecked(access.n().into()) });
                    if let Ok(pn) = pn.raw() {
                        unsafe { lvgl_sys::lv_obj_set_x(pn.as_ptr(), 315 - lvgl_sys::lv_obj_get_width(pn.as_ptr())) }
                    }
                }
            }

            sleep(TICK);
            unsafe { lvgl::timer::update_clock(&clock).unwrap_unchecked(); }
        }

        let _ = self.tx.send(MP3Event::Close); // send multiple times does not matter
    }
}
