#![feature(io_const_error_internals)]

mod util;
mod wav;

#[derive(clap::Parser)]
#[command(version)]
struct Args {
    file: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    use clap::Parser;
    use hound::WavReader;
    use wav::Player;

    let args = Args::parse();

    let reader = WavReader::open(args.file).map_err(util::cvt_err)?;
    let mut player = Player::new(reader).map_err(std::io::Error::other)?;
    player.play().map_err(std::io::Error::other)?;

    Ok(())
}
