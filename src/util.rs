use std::io;

pub fn cvt_err(err: hound::Error) -> io::Error {
	match err {
        hound::Error::IoError(io) => io,
		hound::Error::FormatError(m) => io::Error::from_static_message(m),
		e => io::Error::new(io::ErrorKind::InvalidData, e),
    }
}
