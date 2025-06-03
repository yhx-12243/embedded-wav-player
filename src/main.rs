#![feature(
    debug_closure_helpers,
    fn_traits,
    io_const_error,
    io_const_error_internals,
    likely_unlikely,
    mixed_integer_ops_unsigned_sub,
    never_type,
    new_zeroed_alloc,
    stmt_expr_attributes,
    sync_unsafe_cell,
    unboxed_closures,
)]

mod fmt_impl;
mod gui;
mod log;
mod mp3;
mod shift;
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

    let mut mp3 = MP3::load(args.dir)?;
    mp3.set_volume(i32::from(args.volume) * 128).map_err(std::io::Error::other)?;
    let mtx = mp3.mtx.clone();

    lvgl::init();

    let mut gui = GUI::new(mtx).map_err(gui::cvt_lvgl_err)?;
    tracing::info!("GUI initialized.");
    gui.draw(mp3.get_songs(), i32::from(args.volume) * 128).map_err(gui::cvt_lvgl_err)?;
    tracing::info!("GUI drawing finished.");

    let (gtx, grx) = std::sync::mpsc::channel();
    std::thread::spawn(move || gui.main_loop(grx));
    mp3.main_loop(gtx)
}
