// input file handling for rsvg-convert
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Input {
    Stdin,
    Path(PathBuf),
}

impl std::fmt::Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Input::Stdin => "stdin".fmt(f),
            Input::Path(p) => p.display().fmt(f),
        }
    }
}
