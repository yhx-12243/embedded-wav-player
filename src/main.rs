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

    let args = Args::parse();

    let reader = WavReader::open(args.file).map_err(util::cvt_err)?;

    // TODO: used to output in project part 1, delete it after this
    wav::dump_header(&reader);

    Ok(())
}
