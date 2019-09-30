use cssparser::{self, Parser};

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
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<PaintServer, ValueErrorKind> {
        if parser
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            Ok(PaintServer::None)
        } else if let Ok(url) = parser.try_parse(|i| i.expect_url()) {
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
                iri: Fragment::parse(&url)?,
                alternate,
            })
        } else {
            cssparser::Color::parse(parser)
                .map(PaintServer::SolidColor)
                .map_err(ValueErrorKind::from)
        }
    }
}

pub trait PaintSource {
    type Resolved: ResolvedPaintSource;

    fn resolve(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<Option<Self::Resolved>, RenderingError>;

    fn resolve_fallbacks_and_set_pattern(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx,
        opacity: &UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        if let Some(resolved) = self.resolve(&node, draw_ctx)? {
            let cascaded = CascadedValues::new_from_node(node);
            let values = cascaded.get();
            resolved.set_pattern_on_draw_context(values, draw_ctx, opacity, bbox)
        } else {
            Ok(false)
        }
    }
}

pub trait ResolvedPaintSource {
    fn set_pattern_on_draw_context(
        self,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: &UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError>;
}

// Any of the attributes in gradient and pattern elements may be omitted.
// The missing ones are resolved from the "fallback" IRI. If still missing,
// they are resolved to the default value
pub trait Resolve {
    fn is_resolved(&self) -> bool;

    fn resolve_from_fallback(&mut self, fallback: &Self);

    fn resolve_from_defaults(&mut self);
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
