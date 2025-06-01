#![feature(io_const_error, io_const_error_internals, likely_unlikely)]

use crate::mp3::MP3;

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

fn main() -> std::io::Result<!> {
    use clap::Parser;
    use hound::WavReader;
    use wav::Player;

    let args = Args::parse();

    MP3::set_volume(args.volume)?;
    let mp3 = MP3::load(args.file)?;

    mp3.start_loop()
}
