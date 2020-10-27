// output stream for rsvg-convert

use std::fs;
use std::io;

pub enum Stream {
    File(fs::File),
    Stdout(io::Stdout),
}

impl Stream {
    pub fn new(path: Option<&std::path::Path>) -> io::Result<Self> {
        match path {
            Some(path) => {
                let file = fs::File::create(path)?;
                Ok(Self::File(file))
            }
            None => Ok(Self::Stdout(io::stdout())),
        }
    }
}

impl io::Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::File(file) => file.write(buf),
            Self::Stdout(stream) => stream.write(buf),
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            Self::File(file) => file.write_all(buf),
            Self::Stdout(stream) => stream.write_all(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::File(file) => file.flush(),
            Self::Stdout(stream) => stream.flush(),
        }
    }
}
