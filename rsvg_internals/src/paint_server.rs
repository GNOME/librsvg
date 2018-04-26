use cairo;
use cssparser;
use glib::translate::*;
use glib_sys;
use libc;

use std::ptr;
use std::rc::Rc;

use bbox::RsvgBbox;
use drawing_ctx;
use error::*;
use gradient;
use node::NodeType;
use parsers::{Parse, ParseError};
use pattern;
use util::utf8_cstr;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PaintServerSpread(pub cairo::enums::Extend);

impl Parse for PaintServerSpread {
    type Data = ();
    type Err = AttributeError;

    fn parse(s: &str, _: ()) -> Result<PaintServerSpread, AttributeError> {
        match s {
            "pad" => Ok(PaintServerSpread(cairo::enums::Extend::Pad)),
            "reflect" => Ok(PaintServerSpread(cairo::enums::Extend::Reflect)),
            "repeat" => Ok(PaintServerSpread(cairo::enums::Extend::Repeat)),
            _ => Err(AttributeError::Parse(ParseError::new(
                "expected 'pad' | 'reflect' | 'repeat'",
            ))),
        }
    }
}

impl Default for PaintServerSpread {
    fn default() -> PaintServerSpread {
        PaintServerSpread(cairo::enums::Extend::Pad)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaintServer {
    Inherit,
    Iri {
        iri: String,
        alternate: Option<cssparser::Color>,
    },
    SolidColor(cssparser::Color),
}

impl Parse for PaintServer {
    type Data = ();
    type Err = AttributeError;

    fn parse(s: &str, _: ()) -> Result<PaintServer, AttributeError> {
        let mut input = cssparser::ParserInput::new(s);
        let mut parser = cssparser::Parser::new(&mut input);

        if parser.try(|i| i.expect_ident_matching("inherit")).is_ok() {
            Ok(PaintServer::Inherit)
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
            cssparser::Color::parse(&mut parser)
                .map(PaintServer::SolidColor)
                .map_err(AttributeError::from)
        }
    }
}

fn _set_source_rsvg_solid_color(
    ctx: *mut drawing_ctx::RsvgDrawingCtx,
    color: &cssparser::Color,
    opacity: u8,
    current_color: &cssparser::RGBA,
) {
    let rgba = match *color {
        cssparser::Color::RGBA(ref rgba) => rgba,
        cssparser::Color::CurrentColor => current_color,
    };

    drawing_ctx::get_cairo_context(ctx).set_source_rgba(
        f64::from(rgba.red_f32()),
        f64::from(rgba.green_f32()),
        f64::from(rgba.blue_f32()),
        f64::from(rgba.alpha_f32()) * (f64::from(opacity) / 255.0),
    );
}

/// Parses the paint specification, creating a new paint server object.
/// Return value: (nullable): The newly created paint server, or NULL on error.
///
/// # Arguments
///
/// * `str` - The SVG paint specification string to parse.
#[no_mangle]
pub extern "C" fn rsvg_paint_server_parse(
    inherit: *mut glib_sys::gboolean,
    str: *const libc::c_char,
) -> *const PaintServer {
    if !inherit.is_null() {
        unsafe {
            *inherit = true.to_glib();
        }
    }

    let paint_server = PaintServer::parse(unsafe { utf8_cstr(str) }, ());

    if let Ok(PaintServer::Inherit) = paint_server {
        if !inherit.is_null() {
            unsafe {
                *inherit = false.to_glib();
            }
        }
    }

    match paint_server {
        Ok(m) => Rc::into_raw(Rc::new(m)),
        Err(_) => ptr::null_mut(),
    }
}

/// Increase references counter of `PaintServer`.
///
/// # Arguments
///
/// * `paint_server` - must be constructed with `rsvg_paint_server_parse`.
#[no_mangle]
pub extern "C" fn rsvg_paint_server_ref(paint_server: *const PaintServer) {
    if paint_server.is_null() {
        return;
    }

    let server: Rc<PaintServer> = unsafe { Rc::from_raw(paint_server) };

    // forget about references
    Rc::into_raw(server.clone());
    Rc::into_raw(server);
}

/// Decrease references counter of `PaintServer`.
///
/// # Arguments
///
/// * `paint_server` - must be constructed with `rsvg_paint_server_parse`.
#[no_mangle]
pub extern "C" fn rsvg_paint_server_unref(paint_server: *const PaintServer) {
    if paint_server.is_null() {
        return;
    }

    // drop reference
    unsafe { Rc::from_raw(paint_server) };
}

pub fn _set_source_rsvg_paint_server(
    c_ctx: *mut drawing_ctx::RsvgDrawingCtx,
    ps: &PaintServer,
    opacity: u8,
    bbox: &RsvgBbox,
    current_color: &cssparser::RGBA,
) -> bool {
    let mut had_paint_server = false;

    match *ps {
        PaintServer::Iri {
            ref iri,
            ref alternate,
        } => {
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
                _set_source_rsvg_solid_color(
                    c_ctx,
                    alternate.as_ref().unwrap(),
                    opacity,
                    current_color,
                );
                had_paint_server = true;
            }
        }

        PaintServer::SolidColor(color) => {
            _set_source_rsvg_solid_color(c_ctx, &color, opacity, current_color);
            had_paint_server = true;
        }

        _ => {}
    };

    had_paint_server
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spread_method() {
        assert_eq!(
            PaintServerSpread::parse("pad", ()),
            Ok(PaintServerSpread(cairo::enums::Extend::Pad))
        );

        assert_eq!(
            PaintServerSpread::parse("reflect", ()),
            Ok(PaintServerSpread(cairo::enums::Extend::Reflect))
        );

        assert_eq!(
            PaintServerSpread::parse("repeat", ()),
            Ok(PaintServerSpread(cairo::enums::Extend::Repeat))
        );

        assert!(PaintServerSpread::parse("foobar", ()).is_err());
    }

    #[test]
    fn catches_invalid_syntax() {
        assert!(PaintServer::parse("", ()).is_err());
        assert!(PaintServer::parse("42", ()).is_err());
        assert!(PaintServer::parse("invalid", ()).is_err());
    }

    #[test]
    fn parses_inherit() {
        assert_eq!(PaintServer::parse("inherit", ()), Ok(PaintServer::Inherit));
    }

    #[test]
    fn parses_solid_color() {
        assert_eq!(
            PaintServer::parse("rgb(255, 128, 64, 0.5)", ()),
            Ok(PaintServer::SolidColor(cssparser::Color::RGBA(
                cssparser::RGBA::new(255, 128, 64, 128)
            )))
        );

        assert_eq!(
            PaintServer::parse("currentColor", ()),
            Ok(PaintServer::SolidColor(cssparser::Color::CurrentColor))
        );
    }

    #[test]
    fn parses_iri() {
        assert_eq!(
            PaintServer::parse("url(#link)", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: None,
            },)
        );

        assert_eq!(
            PaintServer::parse("url(#link) none", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: None,
            },)
        );

        assert_eq!(
            PaintServer::parse("url(#link) #ff8040", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: Some(cssparser::Color::RGBA(cssparser::RGBA::new(
                    255, 128, 64, 255
                ))),
            },)
        );

        assert_eq!(
            PaintServer::parse("url(#link) rgb(255, 128, 64, 0.5)", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: Some(cssparser::Color::RGBA(cssparser::RGBA::new(
                    255, 128, 64, 128
                ))),
            },)
        );

        assert_eq!(
            PaintServer::parse("url(#link) currentColor", ()),
            Ok(PaintServer::Iri {
                iri: "#link".to_string(),
                alternate: Some(cssparser::Color::CurrentColor),
            },)
        );

        assert!(PaintServer::parse("url(#link) invalid", ()).is_err());
    }

    #[test]
    fn paint_server_refs_and_unrefs() {
        let rc = Rc::new(PaintServer::parse("#ffffff", ()).unwrap());
        let weak = Rc::downgrade(&rc);
        let ps = Rc::into_raw(rc);

        rsvg_paint_server_ref(ps);
        assert!(weak.upgrade().is_some());

        rsvg_paint_server_unref(ps);
        assert!(weak.upgrade().is_some());

        rsvg_paint_server_unref(ps);
        assert!(weak.upgrade().is_none());
    }
}
