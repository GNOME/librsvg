// output stream for rsvg-convert
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Output {
    Stdout,
    Path(PathBuf),
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Output::Stdout => "stdout".fmt(f),
            Output::Path(p) => p.display().fmt(f),
        }
    }
}
