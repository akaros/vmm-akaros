use std::io;

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    Mach(u32, &'static str),
    Unhandled(u64, &'static str),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<(u32, &'static str)> for Error {
    fn from(e: (u32, &'static str)) -> Error {
        Error::Mach(e.0, e.1)
    }
}
