use std::io;

pub fn cvt_err(err: hound::Error) -> io::Error {
	match err {
        hound::Error::IoError(io) => io,
		e => io::Error::new(io::ErrorKind::InvalidData, e),
    }
}
