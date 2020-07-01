/* SPDX-License-Identifier: GPL-2.0-only */

use std::io;

#[derive(Debug)]
pub enum Error {
    /// IO errors
    IO(io::Error),
    /// Error code returned by foreign functions: (error code, function name)
    FFI(u32, &'static str),
    /// Unhandled VMExit: (reason code, explanation)
    Unhandled(u64, String),
    /// Error type associated with std::thread::JoinHandle::join
    Thread(Box<dyn std::any::Any + Send + 'static>),
    /// Other error message
    Program(String),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IO(e)
    }
}

impl From<(u32, &'static str)> for Error {
    fn from(e: (u32, &'static str)) -> Error {
        Error::FFI(e.0, e.1)
    }
}

impl From<(u64, String)> for Error {
    fn from(e: (u64, String)) -> Error {
        Error::Unhandled(e.0, e.1)
    }
}

impl From<Box<dyn std::any::Any + Send + 'static>> for Error {
    fn from(e: Box<dyn std::any::Any + Send + 'static>) -> Error {
        Error::Thread(e)
    }
}

impl From<String> for Error {
    fn from(msg: String) -> Error {
        Error::Program(msg)
    }
}
