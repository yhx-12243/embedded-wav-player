use core::{ffi::CStr, fmt, ptr::NonNull, time::Duration};
use std::{io, sync::mpsc::Sender, thread::sleep};

use lvgl::{
    input_device::{pointer::Pointer, InputDriver}, widgets::List, CoreError, Display, DisplayError, DrawBuffer, LvResult, Obj, Widget,
};

use crate::util::MP3Event;

pub fn cvt_lvgl_err<E: fmt::Debug>(err: E) -> io::Error {
    let wrapper = fmt::from_fn(|fmt| fmt::Debug::fmt(&err, fmt));
    std::io::Error::other(wrapper.to_string())
}

pub struct GUI {
    tx: Sender<MP3Event>,
    window: *mut lvgl_sys::lv_disp_t,
    screen: Obj,
}

unsafe impl Send for GUI {}

impl GUI {
    extern "C" fn on_close(_: *mut lvgl_sys::lv_disp_t) -> bool {
        true
    }

    pub fn new(tx: Sender<MP3Event>) -> Result<Self, CoreError> {
        unsafe { lvgl_sys::lv_wayland_init(); }

        const HORIZONTAL: i16 = 320;
        const VERTICAL: i16 = 240;
        const TITLE: &CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"Music Player\0") };

        let window = unsafe {
            lvgl_sys::lv_wayland_create_window(HORIZONTAL, VERTICAL, TITLE.as_ptr().cast_mut(), Some(GUI::on_close))
        };

        let screen = unsafe {
            lvgl_sys::lv_disp_get_scr_act(window)
        };

        let screen = Obj::from_raw(NonNull::new(screen).ok_or(CoreError::ResourceNotAvailable)?);

        Ok(Self { tx, window, screen })
    }

    pub fn draw(&mut self) -> LvResult<()> {
        let list = List::new()?;

        Ok(())
    }

    pub fn main_loop(mut self) -> LvResult<()> {
        const TICK: Duration = Duration::from_millis(5);
        loop {
            lvgl::task_handler();

            sleep(TICK);
            // lgvl::timer::update_clock();
        }

        Ok(())
    }
}
