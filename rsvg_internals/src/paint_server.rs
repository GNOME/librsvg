use cssparser::{self, Parser};

use bbox::BoundingBox;
use drawing_ctx;
use error::*;
use gradient;
use node::NodeType;
use parsers::Parse;
use pattern;
use unitinterval::UnitInterval;

#[derive(Debug, Clone, PartialEq)]
pub enum PaintServer {
    None,
    Iri {
        iri: String,
        alternate: Option<cssparser::Color>,
    },
    SolidColor(cssparser::Color),
}

impl Parse for PaintServer {
    type Data = ();
    type Err = AttributeError;

    fn parse(parser: &mut Parser, _: ()) -> Result<PaintServer, AttributeError> {
        if parser.try(|i| i.expect_ident_matching("none")).is_ok() {
            Ok(PaintServer::None)
        } else if let Ok(url) = parser.try(|i| i.expect_url()) {
            let alternate = if !parser.is_exhausted() {
                if parser.try(|i| i.expect_ident_matching("none")).is_ok() {
                    None
                } else {
                    Some(parser.try(|i| cssparser::Color::parse(i))?)
                }
            } else {
                None
            };

            Ok(PaintServer::Iri {
                iri: String::from(url.as_ref()),
                alternate,
            })
        } else {
            cssparser::Color::parse(parser)
                .map(PaintServer::SolidColor)
                .map_err(AttributeError::from)
        }
    }
}

fn set_color(
    ctx: *mut drawing_ctx::RsvgDrawingCtx,
    color: &cssparser::Color,
    opacity: &UnitInterval,
    current_color: &cssparser::RGBA,
) {
    let rgba = match *color {
        cssparser::Color::RGBA(ref rgba) => rgba,
        cssparser::Color::CurrentColor => current_color,
    };

    let &UnitInterval(o) = opacity;
    drawing_ctx::get_cairo_context(ctx).set_source_rgba(
        f64::from(rgba.red_f32()),
        f64::from(rgba.green_f32()),
        f64::from(rgba.blue_f32()),
        f64::from(rgba.alpha_f32()) * o,
    );
}

pub fn set_source_paint_server(
    c_ctx: *mut drawing_ctx::RsvgDrawingCtx,
    ps: &PaintServer,
    opacity: &UnitInterval,
    bbox: &BoundingBox,
    current_color: &cssparser::RGBA,
) -> bool {
    let mut had_paint_server;

    match *ps {
        PaintServer::Iri {
            ref iri,
            ref alternate,
        } => {
            had_paint_server = false;

            if let Some(acquired) = drawing_ctx::get_acquired_node(c_ctx, iri.as_str()) {
                let node = acquired.get();

                if node.get_type() == NodeType::LinearGradient
                    || node.get_type() == NodeType::RadialGradient
                {
                    had_paint_server = gradient::gradient_resolve_fallbacks_and_set_pattern(
                        &node, c_ctx, opacity, bbox,
                    );
                } else if node.get_type() == NodeType::Pattern {
                    had_paint_server =
                        pattern::pattern_resolve_fallbacks_and_set_pattern(&node, c_ctx, bbox);
                }
            }

            if !had_paint_server && alternate.is_some() {
                set_color(c_ctx, alternate.as_ref().unwrap(), opacity, current_color);
                had_paint_server = true;
            }
        }

        PaintServer::SolidColor(color) => {
            set_color(c_ctx, &color, opacity, current_color);
            had_paint_server = true;
        }

        PaintServer::None => {
            had_paint_server = false;
        }
    };

    had_paint_server
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_invalid_syntax() {
        assert!(PaintServer::parse_str("", ()).is_err());
        assert!(PaintServer::parse_str("42", ()).is_err());
        assert!(PaintServer::parse_str("invalid", ()).is_err());
    }

    #[test]
    fn parses_none() {
        assert_eq!(PaintServer::parse_str("none", ()), Ok(PaintServer::None));
    }

    #[test]
    fn parses_solid_color() {
        assert_eq!(
            PaintServer::parse_str("rgb(255, 128, 64, 0.5)", ()),
            Ok(PaintServer::SolidColor(cssparser::Color::RGBA(
                cssparser::RGBA::new(255, 128, 64, 128)
            )))
        );

        assert_eq!(
            PaintServer::parse_str("currentColor", ()),
            Ok(PaintServer::SolidColor(cssparser::Color::CurrentColor))
        );
    }

    #[test]
    fn parses_iri() {
        assert_eq!(
            PaintServer::parse_str("url(#link)", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: None,
            },)
        );

        assert_eq!(
            PaintServer::parse_str("url(#link) none", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: None,
            },)
        );

        assert_eq!(
            PaintServer::parse_str("url(#link) #ff8040", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: Some(cssparser::Color::RGBA(cssparser::RGBA::new(
                    255, 128, 64, 255
                ))),
            },)
        );

        assert_eq!(
            PaintServer::parse_str("url(#link) rgb(255, 128, 64, 0.5)", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: Some(cssparser::Color::RGBA(cssparser::RGBA::new(
                    255, 128, 64, 128
                ))),
            },)
        );

        assert_eq!(
            PaintServer::parse_str("url(#link) currentColor", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: Some(cssparser::Color::CurrentColor),
            },)
        );

        assert!(PaintServer::parse_str("url(#link) invalid", ()).is_err());
    }
}
