//! Layout tree.
//!
//! The idea is to take the DOM tree and produce a layout tree with SVG concepts.

use std::rc::Rc;

use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::dasharray::Dasharray;
use crate::document::AcquiredNodes;
use crate::element::Element;
use crate::length::*;
use crate::node::*;
use crate::paint_server::{PaintSource, UserSpacePaintSource};
use crate::path_builder::Path;
use crate::properties::{
    ClipRule, ComputedValues, Direction, FillRule, Filter, FontFamily, FontStretch, FontStyle,
    FontVariant, FontWeight, Isolation, MixBlendMode, Opacity, Overflow, PaintOrder,
    ShapeRendering, StrokeDasharray, StrokeLinecap, StrokeLinejoin, StrokeMiterlimit,
    TextDecoration, TextRendering, UnicodeBidi, XmlLang,
};
use crate::rect::Rect;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::transform::Transform;
use crate::unit_interval::UnitInterval;

/// SVG Stacking context, an inner node in the layout tree.
///
/// <https://www.w3.org/TR/SVG2/render.html#EstablishingStackingContex>
///
/// This is not strictly speaking an SVG2 stacking context, but a
/// looser version of it.  For example. the SVG spec mentions that a
/// an element should establish a stacking context if the `filter`
/// property applies to the element and is not `none`.  In that case,
/// the element is rendered as an "isolated group" -
/// <https://www.w3.org/TR/2015/CR-compositing-1-20150113/#csscompositingrules_SVG>
///
/// Here we store all the parameters that may lead to the decision to actually
/// render an element as an isolated group.
pub struct StackingContext {
    pub element_name: String,
    pub transform: Transform,
    pub opacity: Opacity,
    pub filter: Filter,
    pub clip_in_user_space: Option<Node>,
    pub clip_in_object_space: Option<Node>,
    pub mask: Option<Node>,
    pub mix_blend_mode: MixBlendMode,
    pub isolation: Isolation,

    /// Target from an <a> element
    pub link_target: Option<String>,
}

/// Stroke parameters in user-space coordinates.
pub struct Stroke {
    pub width: f64,
    pub miter_limit: StrokeMiterlimit,
    pub line_cap: StrokeLinecap,
    pub line_join: StrokeLinejoin,
    pub dash_offset: f64,
    pub dashes: Box<[f64]>,
}

/// Paths and basic shapes resolved to a path.
///
/// Note that `stroke_paint` and `fill_paint` are not in user-space coordinates;
/// they are just resolved to a `PaintSource`.  Turning them to a `UserSpacePaintSource`
/// involves knowing the bounding box of the path.
pub struct Shape {
    pub path: Rc<Path>,
    pub is_visible: bool,
    pub paint_order: PaintOrder,
    pub stroke: Stroke,
    pub stroke_paint: PaintSource,
    pub fill_paint: PaintSource,
    pub fill_rule: FillRule,
    pub clip_rule: ClipRule,
    pub shape_rendering: ShapeRendering,
    pub marker_start: Marker,
    pub marker_mid: Marker,
    pub marker_end: Marker,
}

pub struct Marker {
    pub node_ref: Option<Node>,
    pub context_stroke: PaintSource,
    pub context_fill: PaintSource,
}

/// Image in user-space coordinates.
pub struct Image {
    pub surface: SharedImageSurface,
    pub is_visible: bool,
    pub rect: Rect,
    pub aspect: AspectRatio,
    pub overflow: Overflow,
}

/// A single text span in user-space coordinates.
pub struct TextSpan {
    pub layout: pango::Layout,
    pub gravity: pango::Gravity,
    pub bbox: Option<BoundingBox>,
    pub is_visible: bool,
    pub x: f64,
    pub y: f64,
    pub paint_order: PaintOrder,
    pub stroke: Stroke,
    pub stroke_paint: UserSpacePaintSource,
    pub stroke_paint_source: PaintSource,
    pub fill_paint: UserSpacePaintSource,
    pub text_rendering: TextRendering,
    pub link_target: Option<String>,
}

/// Fully laid-out text in user-space coordinates.
pub struct Text {
    pub spans: Vec<TextSpan>,
}

/// Font-related properties extracted from `ComputedValues`.
pub struct FontProperties {
    pub xml_lang: XmlLang,
    pub unicode_bidi: UnicodeBidi,
    pub direction: Direction,
    pub font_family: FontFamily,
    pub font_style: FontStyle,
    pub font_variant: FontVariant,
    pub font_weight: FontWeight,
    pub font_stretch: FontStretch,
    pub font_size: f64,
    pub letter_spacing: f64,
    pub text_decoration: TextDecoration,
}

impl StackingContext {
    pub fn new(
        acquired_nodes: &mut AcquiredNodes<'_>,
        element: &Element,
        transform: Transform,
        values: &ComputedValues,
    ) -> StackingContext {
        let element_name = format!("{}", element);

        let opacity;
        let filter;

        match *element {
            // "The opacity, filter and display properties do not apply to the mask element"
            // https://drafts.fxtf.org/css-masking-1/#MaskElement
            Element::Mask(_) => {
                opacity = Opacity(UnitInterval::clamp(1.0));
                filter = Filter::None;
            }

            _ => {
                opacity = values.opacity();
                filter = values.filter();
            }
        }

        let clip_path = values.clip_path();
        let clip_uri = clip_path.0.get();
        let (clip_in_user_space, clip_in_object_space) = clip_uri
            .and_then(|node_id| {
                acquired_nodes
                    .acquire(node_id)
                    .ok()
                    .filter(|a| is_element_of_type!(*a.get(), ClipPath))
            })
            .map(|acquired| {
                let clip_node = acquired.get().clone();

                let units = borrow_element_as!(clip_node, ClipPath).get_units();

                match units {
                    CoordUnits::UserSpaceOnUse => (Some(clip_node), None),
                    CoordUnits::ObjectBoundingBox => (None, Some(clip_node)),
                }
            })
            .unwrap_or((None, None));

        let mask = values.mask().0.get().and_then(|mask_id| {
            if let Ok(acquired) = acquired_nodes.acquire(mask_id) {
                let node = acquired.get();
                match *node.borrow_element() {
                    Element::Mask(_) => Some(node.clone()),

                    _ => {
                        rsvg_log!(
                            "element {} references \"{}\" which is not a mask",
                            element,
                            mask_id
                        );

                        None
                    }
                }
            } else {
                rsvg_log!(
                    "element {} references nonexistent mask \"{}\"",
                    element,
                    mask_id
                );

                None
            }
        });

        let mix_blend_mode = values.mix_blend_mode();
        let isolation = values.isolation();

        StackingContext {
            element_name,
            transform,
            opacity,
            filter,
            clip_in_user_space,
            clip_in_object_space,
            mask,
            mix_blend_mode,
            isolation,
            link_target: None,
        }
    }

    pub fn new_with_link(
        acquired_nodes: &mut AcquiredNodes<'_>,
        element: &Element,
        transform: Transform,
        values: &ComputedValues,
        link_target: Option<String>,
    ) -> StackingContext {
        let mut ctx = Self::new(acquired_nodes, element, transform, values);
        ctx.link_target = link_target;
        ctx
    }
}

impl Stroke {
    pub fn new(values: &ComputedValues, params: &NormalizeParams) -> Stroke {
        let width = values.stroke_width().0.to_user(params);
        let miter_limit = values.stroke_miterlimit();
        let line_cap = values.stroke_line_cap();
        let line_join = values.stroke_line_join();
        let dash_offset = values.stroke_dashoffset().0.to_user(params);

        let dashes = match values.stroke_dasharray() {
            StrokeDasharray(Dasharray::None) => Box::new([]),
            StrokeDasharray(Dasharray::Array(dashes)) => dashes
                .iter()
                .map(|l| l.to_user(params))
                .collect::<Box<[f64]>>(),
        };

        Stroke {
            width,
            miter_limit,
            line_cap,
            line_join,
            dash_offset,
            dashes,
        }
    }
}

impl FontProperties {
    /// Collects font properties from a `ComputedValues`.
    ///
    /// The `writing-mode` property is passed separately, as it must come from the `<text>` element,
    /// not the `<tspan>` whose computed values are being passed.
    pub fn new(values: &ComputedValues, params: &NormalizeParams) -> FontProperties {
        FontProperties {
            xml_lang: values.xml_lang(),
            unicode_bidi: values.unicode_bidi(),
            direction: values.direction(),
            font_family: values.font_family(),
            font_style: values.font_style(),
            font_variant: values.font_variant(),
            font_weight: values.font_weight(),
            font_stretch: values.font_stretch(),
            font_size: values.font_size().to_user(params),
            letter_spacing: values.letter_spacing().to_user(params),
            text_decoration: values.text_decoration(),
        }
    }
}
