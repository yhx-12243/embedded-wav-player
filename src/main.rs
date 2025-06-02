#![feature(
    debug_closure_helpers,
    fn_traits,
    io_const_error,
    io_const_error_internals,
    likely_unlikely,
    never_type,
    sync_unsafe_cell,
    unboxed_closures,
)]

mod gui;
mod log;
mod mp3;
mod util;
mod wav;

#[derive(clap::Parser)]
#[command(version)]
struct Args {
    #[arg(help = "Music list directory name")]
    dir: std::path::PathBuf,
    #[arg(
        long,
        short,
        default_value_t = 2,
        value_parser = clap::value_parser!(u8).range(0..=4),
        help = "Volume level (0-4)",
    )]
    volume: u8,
}

fn main() -> std::io::Result<()> {
    use clap::Parser;
    use gui::GUI;
    use mp3::MP3;

    env_logger::builder().format(log::format).init();
    let args = Args::parse();

    MP3::set_volume(args.volume).map_err(std::io::Error::other)?;
    let mp3 = MP3::load(args.dir)?;
    let mtx = mp3.mtx.clone();

    let mut gui = GUI::new(mtx).map_err(gui::cvt_lvgl_err)?;
    gui.draw().map_err(gui::cvt_lvgl_err)?;

    std::thread::spawn(move || gui.main_loop());
    mp3.main_loop()
}
