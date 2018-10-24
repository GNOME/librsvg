use cairo;
use cairo::MatrixTrait;
use cairo::Pattern as CairoPattern;
use std::cell::RefCell;
use std::f64;
use std::rc::*;

use aspect_ratio::*;
use attributes::Attribute;
use bbox::*;
use coord_units::CoordUnits;
use drawing_ctx::{DrawingCtx, NodeStack};
use error::RenderingError;
use float_eq_cairo::ApproxEqCairo;
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::{parse, parse_and_validate};
use property_bag::PropertyBag;
use state::ComputedValues;
use viewbox::*;

coord_units!(PatternUnits, CoordUnits::ObjectBoundingBox);
coord_units!(PatternContentUnits, CoordUnits::UserSpaceOnUse);

#[derive(Clone)]
struct Pattern {
    pub units: Option<PatternUnits>,
    pub content_units: Option<PatternContentUnits>,
    // This Option<Option<ViewBox>> is a bit strange.  We want a field
    // with value None to mean, "this field isn't resolved yet".  However,
    // the vbox can very well be *not* specified in the SVG file.
    // In that case, the fully resolved pattern will have a .vbox=Some(None) value.
    pub vbox: Option<Option<ViewBox>>,
    pub preserve_aspect_ratio: Option<AspectRatio>,
    pub affine: Option<cairo::Matrix>,
    pub fallback: Option<String>,
    pub x: Option<Length>,
    pub y: Option<Length>,
    pub width: Option<Length>,
    pub height: Option<Length>,

    // Point back to our corresponding node, or to the fallback node which has children.
    // If the value is None, it means we are fully resolved and didn't find any children
    // among the fallbacks.
    pub node: Option<Weak<Node>>,
}

impl Default for Pattern {
    fn default() -> Pattern {
        // These are per the spec

        Pattern {
            units: Some(PatternUnits::default()),
            content_units: Some(PatternContentUnits::default()),
            vbox: Some(None),
            preserve_aspect_ratio: Some(AspectRatio::default()),
            affine: Some(cairo::Matrix::identity()),
            fallback: None,
            x: Some(Length::default()),
            y: Some(Length::default()),
            width: Some(Length::default()),
            height: Some(Length::default()),
            node: None,
        }
    }
}

// All of the Pattern's fields are Option<foo> values, because
// those fields can be omitted in the SVG file.  We need to resolve
// them to default values, or to fallback values that come from
// another Pattern.
//
// For the fallback case, this would need something like
//
//    if self.foo.is_none () { self.foo = fallback.foo; }
//
// And for the default case, it would be like
//    if self.foo.is_none () { self.foo = Some (default_value); }
//
// Both can be replaced by
//
//    self.foo = self.foo.take ().or (bar);
//
// So we define a macro for that.
macro_rules! fallback_to (
    ($dest:expr, $default:expr) => (
        $dest = $dest.take ().or ($default)
    );
);

impl Pattern {
    fn unresolved() -> Pattern {
        Pattern {
            units: None,
            content_units: None,
            vbox: None,
            preserve_aspect_ratio: None,
            affine: None,
            fallback: None,
            x: None,
            y: None,
            width: None,
            height: None,
            node: None,
        }
    }

    fn is_resolved(&self) -> bool {
        self.units.is_some()
            && self.content_units.is_some()
            && self.vbox.is_some()
            && self.preserve_aspect_ratio.is_some()
            && self.affine.is_some()
            && self.x.is_some()
            && self.y.is_some()
            && self.width.is_some()
            && self.height.is_some()
            && self.children_are_resolved()
    }

    fn children_are_resolved(&self) -> bool {
        if let Some(ref weak) = self.node {
            let strong_node = &weak.clone().upgrade().unwrap();
            strong_node.has_children()
        } else {
            // We are an empty pattern; there is nothing further that
            // can be resolved for children.
            true
        }
    }

    fn resolve_from_defaults(&mut self) {
        self.resolve_from_fallback(&Pattern::default());
    }

    fn resolve_from_fallback(&mut self, fallback: &Pattern) {
        fallback_to!(self.units, fallback.units);
        fallback_to!(self.content_units, fallback.content_units);
        fallback_to!(self.vbox, fallback.vbox);
        fallback_to!(self.preserve_aspect_ratio, fallback.preserve_aspect_ratio);
        fallback_to!(self.affine, fallback.affine);
        fallback_to!(self.x, fallback.x);
        fallback_to!(self.y, fallback.y);
        fallback_to!(self.width, fallback.width);
        fallback_to!(self.height, fallback.height);

        self.fallback = fallback.fallback.clone();

        if !self.children_are_resolved() {
            if fallback.node.is_some() {
                self.node = fallback.node.clone();
            } else {
                self.node = None;
            }
        }
    }
}

pub struct NodePattern {
    pattern: RefCell<Pattern>,
}

impl NodePattern {
    pub fn new() -> NodePattern {
        NodePattern {
            pattern: RefCell::new(Pattern::unresolved()),
        }
    }
}

impl NodeTrait for NodePattern {
    fn set_atts(
        &self,
        node: &RsvgNode,
        _: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        // pattern element has overflow:hidden
        // https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
        node.set_overflow_hidden();

        let mut p = self.pattern.borrow_mut();

        p.node = Some(Rc::downgrade(node));

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::PatternUnits => p.units = Some(parse("patternUnits", value, ())?),

                Attribute::PatternContentUnits => {
                    p.content_units = Some(parse("patternContentUnits", value, ())?)
                }

                Attribute::ViewBox => p.vbox = Some(Some(parse("viewBox", value, ())?)),

                Attribute::PreserveAspectRatio => {
                    p.preserve_aspect_ratio = Some(parse("preserveAspectRatio", value, ())?)
                }

                Attribute::PatternTransform => {
                    p.affine = Some(parse("patternTransform", value, ())?)
                }

                Attribute::XlinkHref => p.fallback = Some(value.to_owned()),

                Attribute::X => p.x = Some(parse("x", value, LengthDir::Horizontal)?),

                Attribute::Y => p.y = Some(parse("y", value, LengthDir::Vertical)?),

                Attribute::Width => {
                    p.width = Some(parse_and_validate(
                        "width",
                        value,
                        LengthDir::Horizontal,
                        Length::check_nonnegative,
                    )?)
                }

                Attribute::Height => {
                    p.height = Some(parse_and_validate(
                        "height",
                        value,
                        LengthDir::Vertical,
                        Length::check_nonnegative,
                    )?)
                }

                _ => (),
            }
        }

        Ok(())
    }
}

fn resolve_pattern(pattern: &Pattern, draw_ctx: &mut DrawingCtx<'_>) -> Pattern {
    let mut result = pattern.clone();

    let mut stack = NodeStack::new();

    while !result.is_resolved() {
        if let Some(acquired) = draw_ctx.get_acquired_node_of_type(
            result.fallback.as_ref().map(String::as_ref),
            NodeType::Pattern,
        ) {
            let node = acquired.get();

            if stack.contains(node) {
                // FIXME: return a Result here with RenderingError::CircularReference
                // FIXME: print the pattern's name
                rsvg_log!("circular reference in pattern");
                result.resolve_from_defaults();
                break;
            }

            node.with_impl(|i: &NodePattern| result.resolve_from_fallback(&*i.pattern.borrow()));

            stack.push(node);
        } else {
            result.resolve_from_defaults();
        }
    }

    result
}

fn set_pattern_on_draw_context(
    pattern: &Pattern,
    values: &ComputedValues,
    draw_ctx: &mut DrawingCtx<'_>,
    bbox: &BoundingBox,
) -> Result<bool, RenderingError> {
    assert!(pattern.is_resolved());

    if pattern.node.is_none() {
        // This means we didn't find any children among the fallbacks,
        // so there is nothing to render.
        return Ok(false);
    }

    let units = pattern.units.unwrap();
    let content_units = pattern.content_units.unwrap();
    let pattern_affine = pattern.affine.unwrap();
    let vbox = pattern.vbox.unwrap();
    let preserve_aspect_ratio = pattern.preserve_aspect_ratio.unwrap();

    let (pattern_x, pattern_y, pattern_width, pattern_height) = {
        let params = if units == PatternUnits(CoordUnits::ObjectBoundingBox) {
            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        };

        let pattern_x = pattern.x.unwrap().normalize(values, &params);
        let pattern_y = pattern.y.unwrap().normalize(values, &params);
        let pattern_width = pattern.width.unwrap().normalize(values, &params);
        let pattern_height = pattern.height.unwrap().normalize(values, &params);

        (pattern_x, pattern_y, pattern_width, pattern_height)
    };

    // Work out the size of the rectangle so it takes into account the object bounding box

    let bbwscale: f64;
    let bbhscale: f64;

    match units {
        PatternUnits(CoordUnits::ObjectBoundingBox) => {
            let bbrect = bbox.rect.unwrap();
            bbwscale = bbrect.width;
            bbhscale = bbrect.height;
        }

        PatternUnits(CoordUnits::UserSpaceOnUse) => {
            bbwscale = 1.0;
            bbhscale = 1.0;
        }
    }

    let cr = draw_ctx.get_cairo_context();
    let affine = cr.get_matrix();
    let taffine = cairo::Matrix::multiply(&pattern_affine, &affine);

    let mut scwscale = (taffine.xx * taffine.xx + taffine.xy * taffine.xy).sqrt();
    let mut schscale = (taffine.yx * taffine.yx + taffine.yy * taffine.yy).sqrt();

    let pw: i32 = (pattern_width * bbwscale * scwscale) as i32;
    let ph: i32 = (pattern_height * bbhscale * schscale) as i32;

    let scaled_width = pattern_width * bbwscale;
    let scaled_height = pattern_height * bbhscale;

    if scaled_width.abs() < f64::EPSILON || scaled_height.abs() < f64::EPSILON || pw < 1 || ph < 1 {
        return Ok(false);
    }

    scwscale = f64::from(pw) / scaled_width;
    schscale = f64::from(ph) / scaled_height;

    let mut affine: cairo::Matrix = cairo::Matrix::identity();

    // Create the pattern coordinate system
    match units {
        PatternUnits(CoordUnits::ObjectBoundingBox) => {
            let bbrect = bbox.rect.unwrap();
            affine.translate(
                bbrect.x + pattern_x * bbrect.width,
                bbrect.y + pattern_y * bbrect.height,
            );
        }

        PatternUnits(CoordUnits::UserSpaceOnUse) => {
            affine.translate(pattern_x, pattern_y);
        }
    }

    // Apply the pattern transform
    affine = cairo::Matrix::multiply(&affine, &pattern_affine);

    let mut caffine: cairo::Matrix;

    // Create the pattern contents coordinate system
    let _params = if let Some(vbox) = vbox {
        // If there is a vbox, use that
        let (mut x, mut y, w, h) = preserve_aspect_ratio.compute(
            vbox.0.width,
            vbox.0.height,
            0.0,
            0.0,
            pattern_width * bbwscale,
            pattern_height * bbhscale,
        );

        x -= vbox.0.x * w / vbox.0.width;
        y -= vbox.0.y * h / vbox.0.height;

        caffine = cairo::Matrix::new(w / vbox.0.width, 0.0, 0.0, h / vbox.0.height, x, y);

        draw_ctx.push_view_box(vbox.0.width, vbox.0.height)
    } else if content_units == PatternContentUnits(CoordUnits::ObjectBoundingBox) {
        // If coords are in terms of the bounding box, use them
        let bbrect = bbox.rect.unwrap();

        caffine = cairo::Matrix::identity();
        caffine.scale(bbrect.width, bbrect.height);

        draw_ctx.push_view_box(1.0, 1.0)
    } else {
        caffine = cairo::Matrix::identity();
        draw_ctx.get_view_params()
    };

    if !scwscale.approx_eq_cairo(&1.0) || !schscale.approx_eq_cairo(&1.0) {
        let mut scalematrix = cairo::Matrix::identity();
        scalematrix.scale(scwscale, schscale);
        caffine = cairo::Matrix::multiply(&caffine, &scalematrix);

        scalematrix = cairo::Matrix::identity();
        scalematrix.scale(1.0 / scwscale, 1.0 / schscale);

        affine = cairo::Matrix::multiply(&scalematrix, &affine);
    }

    // Draw to another surface

    let cr_save = draw_ctx.get_cairo_context();

    let surface = cr_save
        .get_target()
        .create_similar(cairo::Content::ColorAlpha, pw, ph);

    let cr_pattern = cairo::Context::new(&surface);

    draw_ctx.set_cairo_context(&cr_pattern);

    // Set up transformations to be determined by the contents units

    // Draw everything
    let pattern_node = pattern.node.clone().unwrap().upgrade().unwrap();
    let pattern_cascaded = pattern_node.get_cascaded_values();
    let pattern_values = pattern_cascaded.get();

    cr_pattern.set_matrix(caffine);

    let res = draw_ctx.with_discrete_layer(&pattern_node, pattern_values, false, &mut |dc| {
        pattern_node.draw_children(&pattern_cascaded, dc, false)
    });

    // Return to the original coordinate system and rendering context

    draw_ctx.set_cairo_context(&cr_save);

    // Set the final surface as a Cairo pattern into the Cairo context

    let surface_pattern = cairo::SurfacePattern::create(&surface);
    surface_pattern.set_extend(cairo::Extend::Repeat);

    let mut matrix = affine;
    matrix.invert();

    surface_pattern.set_matrix(matrix);
    surface_pattern.set_filter(cairo::Filter::Best);

    cr_save.set_source(&surface_pattern);

    res.and_then(|_| Ok(true))
}

fn resolve_fallbacks_and_set_pattern(
    pattern: &Pattern,
    values: &ComputedValues,
    draw_ctx: &mut DrawingCtx<'_>,
    bbox: &BoundingBox,
) -> Result<bool, RenderingError> {
    let resolved = resolve_pattern(pattern, draw_ctx);

    set_pattern_on_draw_context(&resolved, values, draw_ctx, bbox)
}

pub fn pattern_resolve_fallbacks_and_set_pattern(
    node: &RsvgNode,
    draw_ctx: &mut DrawingCtx<'_>,
    bbox: &BoundingBox,
) -> Result<bool, RenderingError> {
    assert!(node.get_type() == NodeType::Pattern);

    node.with_impl(|node_pattern: &NodePattern| {
        let pattern = &*node_pattern.pattern.borrow();
        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        resolve_fallbacks_and_set_pattern(pattern, values, draw_ctx, bbox)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_resolved_from_defaults_is_really_resolved() {
        let mut pat = Pattern::unresolved();

        pat.resolve_from_defaults();
        assert!(pat.is_resolved());
    }
}
