//! SVG paint servers.

use cssparser::Parser;

use crate::bbox::BoundingBox;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::Element;
use crate::error::*;
use crate::gradient::{ResolvedGradient, UserSpaceGradient};
use crate::node::NodeBorrow;
use crate::parsers::Parse;
use crate::pattern::{ResolvedPattern, UserSpacePattern};
use crate::properties::ComputedValues;
use crate::url_resolver::Fragment;

#[derive(Debug, Clone, PartialEq)]
pub enum PaintServer {
    None,
    Iri {
        iri: Fragment,
        alternate: Option<cssparser::Color>,
    },
    SolidColor(cssparser::Color),
}

pub enum PaintSource {
    None,
    Gradient(ResolvedGradient, Option<cssparser::Color>),
    Pattern(ResolvedPattern, Option<cssparser::Color>),
    SolidColor(cssparser::Color),
}

pub enum UserSpacePaintSource {
    None,
    Gradient(UserSpaceGradient, Option<cssparser::Color>),
    Pattern(UserSpacePattern, Option<cssparser::Color>),
    SolidColor(cssparser::Color),
}

impl Parse for PaintServer {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<PaintServer, ParseError<'i>> {
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

impl PaintServer {
    pub fn resolve(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
    ) -> Result<PaintSource, RenderingError> {
        match self {
            PaintServer::Iri {
                ref iri,
                ref alternate,
            } => acquired_nodes
                .acquire(iri)
                .and_then(|acquired| {
                    let node = acquired.get();
                    assert!(node.is_element());

                    match *node.borrow_element() {
                        Element::LinearGradient(ref g) => g
                            .resolve(&node, acquired_nodes)
                            .map(|g| PaintSource::Gradient(g, *alternate)),
                        Element::Pattern(ref p) => p
                            .resolve(&node, acquired_nodes)
                            .map(|p| PaintSource::Pattern(p, *alternate)),
                        Element::RadialGradient(ref g) => g
                            .resolve(&node, acquired_nodes)
                            .map(|g| PaintSource::Gradient(g, *alternate)),
                        _ => Err(AcquireError::InvalidLinkType(iri.clone())),
                    }
                })
                .or_else(|err| match (err, alternate) {
                    (AcquireError::MaxReferencesExceeded, _) => {
                        rsvg_log!("maximum number of references exceeded");
                        Err(RenderingError::InstancingLimit)
                    }

                    // The following two cases catch AcquireError::CircularReference, which for
                    // paint servers may mean that there is a pattern or gradient with a reference
                    // cycle in its "href" attribute.  This is an invalid paint server, and per
                    // https://www.w3.org/TR/SVG2/painting.html#SpecifyingPaint we should try to
                    // fall back to the alternate color.
                    (_, Some(color)) => {
                        rsvg_log!(
                            "could not resolve paint server \"{}\", using alternate color",
                            iri
                        );

                        Ok(PaintSource::SolidColor(*color))
                    }

                    (_, _) => {
                        rsvg_log!(
                            "could not resolve paint server \"{}\", no alternate color specified",
                            iri
                        );

                        Ok(PaintSource::None)
                    }
                }),

            PaintServer::SolidColor(color) => Ok(PaintSource::SolidColor(*color)),

            PaintServer::None => Ok(PaintSource::None),
        }
    }
}

impl PaintSource {
    pub fn to_user_space(
        &self,
        bbox: &BoundingBox,
        draw_ctx: &DrawingCtx,
        values: &ComputedValues,
    ) -> UserSpacePaintSource {
        match *self {
            PaintSource::None => UserSpacePaintSource::None,
            PaintSource::SolidColor(c) => UserSpacePaintSource::SolidColor(c),

            PaintSource::Gradient(ref g, c) => match (g.to_user_space(bbox, draw_ctx, values), c) {
                (Some(gradient), c) => UserSpacePaintSource::Gradient(gradient, c),
                (None, Some(c)) => UserSpacePaintSource::SolidColor(c),
                (None, None) => UserSpacePaintSource::None,
            },

            PaintSource::Pattern(ref p, c) => match (p.to_user_space(bbox, draw_ctx, values), c) {
                (Some(pattern), c) => UserSpacePaintSource::Pattern(pattern, c),
                (None, Some(c)) => UserSpacePaintSource::SolidColor(c),
                (None, None) => UserSpacePaintSource::None,
            },
        }
    }
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
