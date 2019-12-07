use markup5ever::QualName;

use crate::error::NodeError;

/// An enumeration of possible inputs for a filter primitive.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Input {
    SourceGraphic,
    SourceAlpha,
    BackgroundImage,
    BackgroundAlpha,
    FillPaint,
    StrokePaint,
    FilterOutput(String),
}

impl Input {
    pub fn parse(attr: QualName, s: &str) -> Result<Input, NodeError> {
        match s {
            "SourceGraphic" => Ok(Input::SourceGraphic),
            "SourceAlpha" => Ok(Input::SourceAlpha),
            "BackgroundImage" => Ok(Input::BackgroundImage),
            "BackgroundAlpha" => Ok(Input::BackgroundAlpha),
            "FillPaint" => Ok(Input::FillPaint),
            "StrokePaint" => Ok(Input::StrokePaint),
            s if !s.is_empty() => Ok(Input::FilterOutput(s.to_string())),
            _ => Err(NodeError::parse_error(attr, "invalid value")),
        }
    }
}
