// input file handling for rsvg-convert

use core::ops::Deref;
use gio::FileExt;
use std::fmt;
use std::path::PathBuf;

struct Stdin;

impl Stdin {
    pub fn stream() -> gio::UnixInputStream {
        unsafe { gio::UnixInputStream::new(0) }
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
    pub fn stream(&self) -> &gio::InputStream {
        self.stream.deref()
    }

    pub fn file(&self) -> Option<&gio::File> {
        self.file.as_ref()
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.file {
            Some(file) => file.get_path().unwrap().display().fmt(f),
            None => "stdin".fmt(f),
        }
    }
}

impl Iterator for Input<'_> {
    type Item = Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Input::Paths(paths) => paths.next().and_then(|p| {
                let file = gio::File::new_for_path(p);
                let stream = file.read(None::<&gio::Cancellable>).ok()?;
                Some(Item {
                    stream: Stream::File(stream),
                    file: Some(file),
                })
            }),

            Input::Stdin(iter) => iter.next().map(|s| Item {
                stream: Stream::Unix(s),
                file: None,
            }),
        }
    }
}
