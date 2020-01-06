//! SVG paint servers.

use cssparser::Parser;

use crate::allowed_url::Fragment;
use crate::bbox::BoundingBox;
use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::node::{CascadedValues, RsvgNode};
use crate::parsers::Parse;
use crate::properties::ComputedValues;
use crate::unit_interval::UnitInterval;

#[derive(Debug, Clone, PartialEq)]
pub enum PaintServer {
    None,
    Iri {
        iri: Fragment,
        alternate: Option<cssparser::Color>,
    },
    SolidColor(cssparser::Color),
}

impl Parse for PaintServer {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<PaintServer, CssParseError<'i>> {
        if parser
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            Ok(PaintServer::None)
        } else if let Ok(url) = parser.try_parse(|i| i.expect_url()) {
            let loc = parser.current_source_location();

            let alternate = if !parser.is_exhausted() {
                if parser
                    .try_parse(|i| i.expect_ident_matching("none"))
                    .is_ok()
                {
                    None
                } else {
                    Some(parser.try_parse(|i| cssparser::Color::parse(i))?)
                }
            } else {
                None
            };

            Ok(PaintServer::Iri {
                iri: Fragment::parse(&url)
                    .map_err(|e: HrefError| -> ValueErrorKind { e.into() })
                    .map_err(|e| loc.new_custom_error(e))?,
                alternate,
            })
        } else {
            Ok(cssparser::Color::parse(parser).map(PaintServer::SolidColor)?)
        }
    }
}

pub trait PaintSource {
    type Resolved: AsPaintSource;

    fn resolve(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<Self::Resolved, AcquireError>;

    fn resolve_fallbacks_and_set_pattern(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx,
        opacity: UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        match self.resolve(&node, draw_ctx) {
            Ok(resolved) => {
                let cascaded = CascadedValues::new_from_node(node);
                let values = cascaded.get();
                resolved.set_as_paint_source(values, draw_ctx, opacity, bbox)
            }

            Err(AcquireError::CircularReference(node)) => {
                rsvg_log!("circular reference in paint server {}", node);
                Err(RenderingError::CircularReference)
            }

            Err(AcquireError::MaxReferencesExceeded) => {
                rsvg_log!("maximum number of references exceeded");
                Err(RenderingError::InstancingLimit)
            }

            Err(e) => {
                rsvg_log!("not using paint server {}: {}", node, e);

                // "could not resolve" means caller needs to fall back to color
                Ok(false)
            }
        }
    }
}

pub trait AsPaintSource {
    fn set_as_paint_source(
        self,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError>;
}

// Any of the attributes in gradient and pattern elements may be omitted.
// The missing ones are resolved from the "fallback" IRI. If still missing,
// they are resolved to the default value
pub trait Resolve {
    fn is_resolved(&self) -> bool;

    fn resolve_from_fallback(&self, fallback: &Self) -> Self;

    fn resolve_from_defaults(&self) -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_invalid_syntax() {
        assert!(PaintServer::parse_str("").is_err());
        assert!(PaintServer::parse_str("42").is_err());
        assert!(PaintServer::parse_str("invalid").is_err());
    }

    #[test]
    fn parses_none() {
        assert_eq!(PaintServer::parse_str("none"), Ok(PaintServer::None));
    }

    #[test]
    fn parses_solid_color() {
        assert_eq!(
            PaintServer::parse_str("rgb(255, 128, 64, 0.5)"),
            Ok(PaintServer::SolidColor(cssparser::Color::RGBA(
                cssparser::RGBA::new(255, 128, 64, 128)
            )))
        );

        assert_eq!(
            PaintServer::parse_str("currentColor"),
            Ok(PaintServer::SolidColor(cssparser::Color::CurrentColor))
        );
    }

    #[test]
    fn parses_iri() {
        assert_eq!(
            PaintServer::parse_str("url(#link)"),
            Ok(PaintServer::Iri {
                iri: Fragment::new(None, "link".to_string()),
                alternate: None,
            },)
        );

        assert_eq!(
            PaintServer::parse_str("url(foo#link) none"),
            Ok(PaintServer::Iri {
                iri: Fragment::new(Some("foo".to_string()), "link".to_string()),
                alternate: None,
            },)
        );

        assert_eq!(
            PaintServer::parse_str("url(#link) #ff8040"),
            Ok(PaintServer::Iri {
                iri: Fragment::new(None, "link".to_string()),
                alternate: Some(cssparser::Color::RGBA(cssparser::RGBA::new(
                    255, 128, 64, 255
                ))),
            },)
        );

        assert_eq!(
            PaintServer::parse_str("url(#link) rgb(255, 128, 64, 0.5)"),
            Ok(PaintServer::Iri {
                iri: Fragment::new(None, "link".to_string()),
                alternate: Some(cssparser::Color::RGBA(cssparser::RGBA::new(
                    255, 128, 64, 128
                ))),
            },)
        );

        assert_eq!(
            PaintServer::parse_str("url(#link) currentColor"),
            Ok(PaintServer::Iri {
                iri: Fragment::new(None, "link".to_string()),
                alternate: Some(cssparser::Color::CurrentColor),
            },)
        );

        assert!(PaintServer::parse_str("url(#link) invalid").is_err());
    }
}
