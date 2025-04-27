#![feature(
    io_const_error,
    io_const_error_internals,
    likely_unlikely,
)]

mod util;
mod wav;

#[derive(clap::Parser)]
#[command(version)]
struct Args {
    #[arg(
        long, 
        short, 
        help = "File name"
    )]
    file: std::path::PathBuf,
    #[arg(
        long, 
        short, 
        default_value_t = 2, 
        value_parser = clap::value_parser!(u8).range(0..=4),
        help = "Volume level (0-4)"
    )]
    volume: u8,
}

fn main() -> std::io::Result<()> {
    use clap::Parser;
    use hound::WavReader;
    use wav::Player;

    let args = Args::parse();

    let reader = WavReader::open(args.file).map_err(util::cvt_err)?;
    let mut player = Player::new(reader, args.volume)?;
    player.play()?;

    Ok(())
}
