use std::io;

#[derive(Debug)]
pub enum Error {
    IO(io::Error),                                   // IO errors
    Mach(u32, &'static str), // Error code returned by Mach Kernel, (error, func())
    Unhandled(u64, &'static str), // (vm exit reason, description of error)
    Thread(Box<dyn std::any::Any + Send + 'static>), // Wrap the Error of std::thread::Thread
    Program(&'static str),   // any other error message
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

impl From<&'static str> for Error {
    fn from(msg: &'static str) -> Error {
        Error::Program(msg)
    }
}
