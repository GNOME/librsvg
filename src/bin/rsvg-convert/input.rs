// input file handling for rsvg-convert

use core::ops::Deref;
use gio::FileExt;
use std::os::unix::io::RawFd;
use std::path::PathBuf;

struct Stdin;

impl Stdin {
    pub fn stream() -> gio::UnixInputStream {
        unsafe { gio::UnixInputStream::new(Self {}) }
    }
}

impl std::os::unix::io::IntoRawFd for Stdin {
    fn into_raw_fd(self) -> RawFd {
        0 as RawFd
    }
}

pub enum Input<'a> {
    Paths(std::slice::Iter<'a, PathBuf>),
    Stdin(std::iter::Once<gio::UnixInputStream>),
}

impl<'a> Input<'a> {
    pub fn new(paths: &'a [PathBuf]) -> Self {
        match paths.len() {
            0 => Input::Stdin(std::iter::once(Stdin::stream())),
            _ => Input::Paths(paths.iter()),
        }
    }
}

enum Stream {
    File(gio::FileInputStream),
    Unix(gio::UnixInputStream),
}

impl Deref for Stream {
    type Target = gio::InputStream;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::File(stream) => stream.as_ref(),
            Self::Unix(stream) => stream.as_ref(),
        }
    }
}

pub struct Item {
    stream: Stream,
    file: Option<gio::File>,
}

impl Item {
    fn from_file(file: gio::File) -> Self {
        Self {
            // TODO: unwrap
            stream: Stream::File(file.read(None::<&gio::Cancellable>).unwrap()),
            file: Some(file),
        }
    }
    fn from_path(path: &PathBuf) -> Self {
        Self::from_file(gio::File::new_for_path(path))
    }

    fn from_unix_stream(stream: gio::UnixInputStream) -> Self {
        Self {
            stream: Stream::Unix(stream),
            file: None,
        }
    }

    pub fn stream(&self) -> &gio::InputStream {
        self.stream.deref()
    }

    pub fn file(&self) -> Option<&gio::File> {
        self.file.as_ref()
    }
}

impl Iterator for Input<'_> {
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Input::Paths(paths) => paths.next().map(Item::from_path),
            Input::Stdin(iter) => iter.next().map(Item::from_unix_stream),
        }
    }
}
