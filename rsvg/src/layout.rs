//! Layout tree.
//!
//! The idea is to take the DOM tree and produce a layout tree with SVG concepts.

use std::rc::Rc;

use float_cmp::approx_eq;

use crate::aspect_ratio::AspectRatio;
use crate::cairo_path::CairoPath;
use crate::color::Color;
use crate::coord_units::CoordUnits;
use crate::dasharray::Dasharray;
use crate::document::{AcquiredNode, AcquiredNodes};
use crate::drawing_ctx::{DrawingCtx, FontOptions, Viewport, pango_layout_to_cairo_path};
use crate::element::{Element, ElementData};
use crate::error::{AcquireError, InternalRenderingError};
use crate::filter::FilterValueList;
use crate::length::*;
use crate::node::*;
use crate::paint_server::{PaintSource, UserSpacePaintSource};
use crate::path_builder::Path as SvgPath;
use crate::properties::{
    self, ClipRule, ComputedValues, Direction, FillRule, FontFamily, FontStretch, FontStyle,
    FontVariant, FontWeight, ImageRendering, Isolation, MixBlendMode, Opacity, Overflow,
    PaintOrder, ShapeRendering, StrokeDasharray, StrokeLinecap, StrokeLinejoin, StrokeMiterlimit,
    TextDecoration, TextRendering, UnicodeBidi, VectorEffect, XmlLang,
};
use crate::rect::Rect;
use crate::rsvg_log;
use crate::session::Session;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::transform::{Transform, ValidTransform};
use crate::unit_interval::UnitInterval;
use crate::viewbox::ViewBox;
use crate::{borrow_element_as, is_element_of_type};

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
    pub is_visible: bool,
    pub opacity: Opacity,
    pub filter: Option<Filter>,
    pub clip_rect: Option<Rect>,
    pub clip_in_object_space: Option<Node>,
    pub clip_path: Option<ClipPath>,
    pub mask: Option<Node>,
    pub mix_blend_mode: MixBlendMode,
    pub isolation: Isolation,

    /// Target from an `<a>` element
    pub link_target: Option<String>,
}

/// Recursive representation of the `clipPath` element, a union of clipping paths.
///
/// The `clipPath` element supports having a list of child elements that define paths; the
/// final clipping path is the union of them.  In turn, every path can have its own
/// clipPath.
pub struct ClipPath {
    pub clip_units: CoordUnits,
    pub transform: Transform,
    pub paths: Vec<ClipPathItem>,
    pub clip_path: Option<Box<ClipPath>>,
}

/// One item in a [`ClipPath`].
pub struct ClipPathItem {
    pub transform: Transform,
    pub path: CairoPath,
    pub clip_rule: ClipRule,
    pub clip_path: Option<Box<ClipPath>>,
}

/// The item being rendered inside a stacking context.
pub struct Layer {
    pub kind: LayerKind,
    pub stacking_ctx: StackingContext,
}

pub enum LayerKind {
    Shape(Box<Shape>),
    Text(Box<Text>),
    Image(Box<Image>),
    Group(Box<Group>),
}

pub struct Group {
    pub children: Vec<Layer>,
    pub establish_viewport: Option<LayoutViewport>,
    pub extents: Option<Rect>,
}

/// Used for elements that need to establish a new viewport, like `<svg>`.
pub struct LayoutViewport {
    // transform goes in the group's layer's StackingContext
    /// Position and size of the element, per its x/y/width/height properties.
    /// For markers, this is markerWidth/markerHeight.
    pub geometry: Rect,

    /// viewBox attribute
    pub vbox: Option<ViewBox>,

    /// preserveAspectRatio attribute
    pub preserve_aspect_ratio: AspectRatio,

    /// overflow property
    pub overflow: Overflow,
}

/// Stroke parameters in user-space coordinates.
pub struct Stroke {
    pub width: f64,
    pub miter_limit: StrokeMiterlimit,
    pub line_cap: StrokeLinecap,
    pub line_join: StrokeLinejoin,
    pub dash_offset: f64,
    pub dashes: Box<[f64]>,
    // https://svgwg.org/svg2-draft/painting.html#non-scaling-stroke
    pub non_scaling: bool,
}

/// A path known to be representable by Cairo.
pub struct Path {
    pub cairo_path: CairoPath,
    pub path: Rc<SvgPath>,
    pub extents: Option<Rect>,
}

/// Paths and basic shapes resolved to a path.
pub struct Shape {
    pub path: Path,
    pub paint_order: PaintOrder,
    pub stroke_paint: UserSpacePaintSource,
    pub fill_paint: UserSpacePaintSource,
    pub stroke: Stroke,
    pub fill_rule: FillRule,
    pub clip_rule: ClipRule,
    pub shape_rendering: ShapeRendering,
    pub marker_start: Marker,
    pub marker_mid: Marker,
    pub marker_end: Marker,
}

pub struct Marker {
    pub node_ref: Option<Node>,
    pub context_stroke: Rc<PaintSource>,
    pub context_fill: Rc<PaintSource>,
}

/// Image in user-space coordinates.
pub struct Image {
    pub surface: SharedImageSurface,
    pub rect: Rect,
    pub aspect: AspectRatio,
    pub overflow: Overflow,
    pub image_rendering: ImageRendering,
}

/// A single text span in user-space coordinates.
pub struct TextSpan {
    pub layout: pango::Layout,
    pub gravity: pango::Gravity,
    pub extents: Option<Rect>,
    pub is_visible: bool,
    pub x: f64,
    pub y: f64,
    pub paint_order: PaintOrder,
    pub stroke: Stroke,
    pub stroke_paint: UserSpacePaintSource,
    pub fill_paint: UserSpacePaintSource,
    pub text_rendering: TextRendering,
    pub link_target: Option<String>,
}

/// Fully laid-out text in user-space coordinates.
pub struct Text {
    pub spans: Vec<TextSpan>,
    pub extents: Option<Rect>,
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

pub struct Filter {
    pub filter_list: FilterValueList,
    pub current_color: Color,
    pub stroke_paint_source: Rc<PaintSource>,
    pub fill_paint_source: Rc<PaintSource>,
    pub normalize_values: NormalizeValues,
}

fn get_filter(
    values: &ComputedValues,
    acquired_nodes: &mut AcquiredNodes<'_>,
    referencing_element_name: &str,
    session: &Session,
) -> Option<Filter> {
    match values.filter() {
        properties::Filter::None => None,

        properties::Filter::List(filter_list) => Some(get_filter_from_filter_list(
            filter_list,
            acquired_nodes,
            referencing_element_name,
            values,
            session,
        )),
    }
}

fn get_filter_from_filter_list(
    filter_list: FilterValueList,
    acquired_nodes: &mut AcquiredNodes<'_>,
    referencing_element_name: &str,
    values: &ComputedValues,
    session: &Session,
) -> Filter {
    let current_color = values.color().0;

    let stroke_paint_source = values.stroke().0.resolve(
        acquired_nodes,
        referencing_element_name,
        values.stroke_opacity().0,
        current_color,
        None,
        None,
        session,
    );

    let fill_paint_source = values.fill().0.resolve(
        acquired_nodes,
        referencing_element_name,
        values.fill_opacity().0,
        current_color,
        None,
        None,
        session,
    );

    let normalize_values = NormalizeValues::new(values);

    Filter {
        filter_list,
        current_color,
        stroke_paint_source,
        fill_paint_source,
        normalize_values,
    }
}

fn acquire_clip_path(
    source_element: &Element,
    acquired_nodes: &mut AcquiredNodes<'_>,
) -> Result<Option<AcquiredNode>, AcquireError> {
    let values = source_element.get_computed_values();
    let clip_path_prop = values.clip_path();

    if let Some(node_id) = clip_path_prop.0.get() {
        let source_element_name = format!("{source_element}");
        let acquired = acquired_nodes.acquire(&source_element_name, node_id)?;

        let candidate_clip_path_node = acquired.get().clone();

        if is_element_of_type!(candidate_clip_path_node, ClipPath) {
            Ok(Some(acquired))
        } else {
            Err(AcquireError::InvalidLinkType(node_id.clone()))
        }
    } else {
        Ok(None)
    }
}

fn acquire_clip_path_and_log_error(
    session: &Session,
    source_element: &Element,
    acquired_nodes: &mut AcquiredNodes<'_>,
) -> Option<AcquiredNode> {
    match acquire_clip_path(source_element, acquired_nodes) {
        Ok(node) => node,
        Err(e) => {
            rsvg_log!(session, "ignoring clip-path for {source_element}: {e}");
            None
        }
    }
}

fn layout_clip_path(
    session: &Session,
    source_element: &Element,
    font_options: &FontOptions,
    acquired_nodes: &mut AcquiredNodes<'_>,
    params: &NormalizeParams,
    viewport: &Viewport,
) -> Option<ClipPath> {
    if let Some(acquired) = acquire_clip_path_and_log_error(session, source_element, acquired_nodes)
    {
        let clip_path_node = acquired.get();
        let clip_path_elt = clip_path_node.borrow_element();
        let clip_path_data = borrow_element_as!(clip_path_node, ClipPath);

        let values = clip_path_elt.get_computed_values();

        let clip_units = clip_path_data.get_units();
        let transform = values.transform();

        let paths = layout_paths_for_clip_path(
            session,
            clip_path_node,
            font_options,
            acquired_nodes,
            params,
            viewport,
        );
        let recursive_clip_path = layout_clip_path(
            session,
            &clip_path_elt,
            font_options,
            acquired_nodes,
            params,
            viewport,
        )
        .map(Box::new);

        Some(ClipPath {
            clip_units,
            transform,
            paths,
            clip_path: recursive_clip_path,
        })
    } else {
        None
    }
}

// FIXME: this will need to use Text2 when we implement it
fn path_from_text(
    text: &crate::text::Text,
    node: &Node,
    cascaded: &CascadedValues<'_>,
    session: &Session,
    font_options: &FontOptions,
    acquired_nodes: &mut AcquiredNodes<'_>,
    viewport: &Viewport,
) -> Result<CairoPath, Box<InternalRenderingError>> {
    let text_layout = text.layout_text_spans(
        node,
        acquired_nodes,
        cascaded,
        viewport,
        font_options.clone(),
        session,
    );

    let mut result = CairoPath::empty();

    for span in &text_layout.spans {
        let path = pango_layout_to_cairo_path(span.x, span.y, &span.layout, span.gravity)?;

        // FIXME: does the text-rendering property (for text antialiasing) apply to clipping paths?

        result.append(path);
    }

    Ok(result)
}

fn path_from_use_referenced_from_clip_path(
    use_node: &Node,
    session: &Session,
    font_options: &FontOptions,
    acquired_nodes: &mut AcquiredNodes<'_>,
    viewport: &Viewport,
) -> Option<ClipPathItem> {
    // FIXME: the following is copied from DrawingCtx::draw_from_use_node().  We need to
    // refactor this; maybe by making AcquiredNode carry the acquire_ref()'ed <use> node as well.

    let _use_acquired = match acquired_nodes.acquire_ref(use_node) {
        Ok(n) => n,

        _ => return None,
    };

    let use_element = use_node.borrow_element();
    let use_element_name = format!("{use_element}");
    let use_element_data = borrow_element_as!(use_node, Use);

    let acquired = if let Some(link) = use_element_data.get_link() {
        match acquired_nodes.acquire(&use_element_name, &link) {
            Ok(acquired) => acquired,

            _ => return None,
        }
    } else {
        return None;
    };

    let use_values = use_element.get_computed_values();
    let use_params = NormalizeParams::new(use_values, viewport);

    // width or height set to 0 disables rendering of the element
    // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute

    let use_rect = use_element_data.get_rect(&use_params);
    if use_rect.is_empty() {
        return None;
    }

    let child = acquired.get();
    if !element_can_be_used_inside_use_inside_clip_path(&child.borrow_element()) {
        return None;
    }

    let child_cascaded = CascadedValues::new_from_values(
        child, use_values, None, // fill_paint, not used in clipping
        None, // stroke_paint, not used in clipping
    );
    let child_values = child_cascaded.get();

    if !child_values.is_displayed() || !child_values.is_visible() {
        // https://www.w3.org/TR/css-masking-1/#ClipPathElement
        //
        // "If a child element [of the clipPath element] is made invisible by display or
        // visibility it does not contribute to the clipping path."

        return None;
    }

    let use_transform = ValidTransform::try_from(
        use_values
            .transform()
            .pre_translate(use_rect.x0, use_rect.y0)
            .pre_transform(&child_values.transform()),
    )
    .ok()?;

    match viewport.with_composed_transform(use_transform) {
        Ok(use_viewport) => {
            let params = NormalizeParams::new(child_values, &use_viewport);
            let child_data = child.borrow_element_data();

            let path = match *child_data {
                ElementData::Path(ref e) => e.make_path(&params, child_values).to_cairo_path(false),
                ElementData::Polygon(ref e) => {
                    e.make_path(&params, child_values).to_cairo_path(false)
                }
                ElementData::Polyline(ref e) => {
                    e.make_path(&params, child_values).to_cairo_path(false)
                }
                ElementData::Line(ref e) => e.make_path(&params, child_values).to_cairo_path(false),
                ElementData::Rect(ref e) => e.make_path(&params, child_values).to_cairo_path(false),
                ElementData::Circle(ref e) => {
                    e.make_path(&params, child_values).to_cairo_path(false)
                }
                ElementData::Ellipse(ref e) => {
                    e.make_path(&params, child_values).to_cairo_path(false)
                }
                ElementData::Text(ref e) => path_from_text(
                    e,
                    child,
                    &child_cascaded,
                    session,
                    font_options,
                    acquired_nodes,
                    viewport,
                )
                .ok()?,
                _ => unreachable!(
                    "we already checked the allowed types of elements in <use> in <clipPath>"
                ),
            };

            Some(ClipPathItem {
                transform: *use_transform,
                path,
                clip_rule: child_values.clip_rule(),
                clip_path: layout_clip_path(
                    session,
                    &child.borrow_element(),
                    font_options,
                    acquired_nodes,
                    &params,
                    &use_viewport,
                )
                .map(Box::new),
            })
        }

        Err(e) => {
            rsvg_log!(session, "ignoring {use_node} inside clipPath: {e}");
            None
        }
    }
}

// https://www.w3.org/TR/css-masking-1/#ClipPathElement
pub fn element_can_be_used_inside_use_inside_clip_path(element: &Element) -> bool {
    use ElementData::*;

    matches!(
        element.element_data,
        Circle(_) | Ellipse(_) | Line(_) | Path(_) | Polygon(_) | Polyline(_) | Rect(_) | Text(_)
    )
}

fn clip_path_item_from_node(
    session: &Session,
    node: &Node,
    font_options: &FontOptions,
    acquired_nodes: &mut AcquiredNodes<'_>,
    params: &NormalizeParams,
    viewport: &Viewport,
) -> Option<ClipPathItem> {
    let elt = node.borrow_element();
    let data = node.borrow_element_data();
    let values = elt.get_computed_values();

    if !values.is_displayed() || !values.is_visible() {
        // https://www.w3.org/TR/css-masking-1/#ClipPathElement
        //
        // "If a child element [of the clipPath element] is made invisible by display or
        // visibility it does not contribute to the clipping path."

        return None;
    }

    if let ElementData::Use(_) = *data {
        // FIXME: the following implies that we need to return a Result, likely everywhere,
        // to deal with MaxReferencesExceeded and such.
        path_from_use_referenced_from_clip_path(
            node,
            session,
            font_options,
            acquired_nodes,
            viewport,
        )
    } else {
        let path = match *data {
            ElementData::Path(ref e) => e.make_path(params, values).to_cairo_path(false),
            ElementData::Polygon(ref e) => e.make_path(params, values).to_cairo_path(false),
            ElementData::Polyline(ref e) => e.make_path(params, values).to_cairo_path(false),
            ElementData::Line(ref e) => e.make_path(params, values).to_cairo_path(false),
            ElementData::Rect(ref e) => e.make_path(params, values).to_cairo_path(false),
            ElementData::Circle(ref e) => e.make_path(params, values).to_cairo_path(false),
            ElementData::Ellipse(ref e) => e.make_path(params, values).to_cairo_path(false),
            ElementData::Text(ref e) => {
                let cascaded = CascadedValues::new_from_node(node);
                path_from_text(
                    e,
                    node,
                    &cascaded,
                    session,
                    font_options,
                    acquired_nodes,
                    viewport,
                )
                .ok()?
            }

            _ => return None,
        };

        Some(ClipPathItem {
            transform: values.transform(),
            path,
            clip_rule: values.clip_rule(),
            clip_path: layout_clip_path(
                session,
                &elt,
                font_options,
                acquired_nodes,
                params,
                viewport,
            )
            .map(Box::new),
        })
    }
}

fn layout_paths_for_clip_path(
    session: &Session,
    clip_path_node: &Node,
    font_options: &FontOptions,
    acquired_nodes: &mut AcquiredNodes<'_>,
    params: &NormalizeParams,
    viewport: &Viewport,
) -> Vec<ClipPathItem> {
    clip_path_node
        .children()
        .filter(|c| c.is_element())
        .filter_map(|child| {
            clip_path_item_from_node(
                session,
                &child,
                font_options,
                acquired_nodes,
                params,
                viewport,
            )
        })
        .collect()
}

fn resolve_object_space_clip_path(
    values: &ComputedValues,
    acquired_nodes: &mut AcquiredNodes<'_>,
    referencing_element_name: &str,
) -> Option<Node> {
    let clip_path = values.clip_path();
    let clip_uri = clip_path.0.get();
    clip_uri
        .and_then(|node_id| {
            acquired_nodes
                .acquire(referencing_element_name, node_id)
                .ok()
                .filter(|a| is_element_of_type!(*a.get(), ClipPath))
        })
        .map(|acquired| {
            let clip_node = acquired.get().clone();

            let units = borrow_element_as!(clip_node, ClipPath).get_units();

            match units {
                CoordUnits::UserSpaceOnUse => None,
                CoordUnits::ObjectBoundingBox => Some(clip_node),
            }
        })
        .unwrap_or(None)
}

impl StackingContext {
    pub fn new(
        draw_ctx: &DrawingCtx,
        acquired_nodes: &mut AcquiredNodes<'_>,
        element: &Element,
        transform: Transform,
        clip_rect: Option<Rect>,
        values: &ComputedValues,
        viewport: &Viewport,
    ) -> StackingContext {
        // FIXME: practically the only reason we need the DrawingCtx as an argument is so that
        // the call to layout_clip_path() below can extract the FontOptions from the DrawingCtx, and that
        // is only so that if a referenced clipPath element has a <text> child, then we'll be able
        // to layout the text element to use as a clipping path.  Could we carry the FontOptions
        // somewhere else...?

        let session = draw_ctx.session().clone();
        let font_options = draw_ctx.get_font_options();

        let element_name = format!("{element}");

        let is_visible = values.is_visible();

        let opacity;
        let filter;

        match element.element_data {
            // "The opacity, filter and display properties do not apply to the mask element"
            // https://drafts.fxtf.org/css-masking-1/#MaskElement
            ElementData::Mask(_) => {
                opacity = Opacity(UnitInterval::clamp(1.0));
                filter = None;
            }

            _ => {
                opacity = values.opacity();
                filter = get_filter(values, acquired_nodes, &element_name, &session);
            }
        }

        // These are the params "outside" the stacking context, and they are used to normalize
        // lengths inside a clipPath's children (e.g. <clipPath> <rect x="10%"/> </clipPath>).
        let params = NormalizeParams::new(values, viewport);
        let clip_path = layout_clip_path(
            &session,
            element,
            &font_options,
            acquired_nodes,
            &params,
            viewport,
        );

        let element_name = format!("{element}");

        let clip_in_object_space =
            resolve_object_space_clip_path(values, acquired_nodes, &element_name);

        let mask = values.mask().0.get().and_then(|mask_id| {
            if let Ok(acquired) = acquired_nodes.acquire(&element_name, mask_id) {
                let node = acquired.get();
                match *node.borrow_element_data() {
                    ElementData::Mask(_) => Some(node.clone()),

                    _ => {
                        rsvg_log!(
                            session,
                            "element {} references \"{}\" which is not a mask",
                            element,
                            mask_id
                        );

                        None
                    }
                }
            } else {
                rsvg_log!(
                    session,
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
            is_visible,
            opacity,
            filter,
            clip_rect,
            clip_in_object_space,
            clip_path,
            mask,
            mix_blend_mode,
            isolation,
            link_target: None,
        }
    }

    pub fn new_with_link(
        draw_ctx: &DrawingCtx,
        acquired_nodes: &mut AcquiredNodes<'_>,
        element: &Element,
        transform: Transform,
        values: &ComputedValues,
        viewport: &Viewport,
        link_target: Option<String>,
    ) -> StackingContext {
        // Note that the clip_rect=Some(...) argument is only used by the markers code,
        // hence it is None here.  Something to refactor later.
        let mut ctx = Self::new(
            draw_ctx,
            acquired_nodes,
            element,
            transform,
            None,
            values,
            viewport,
        );
        ctx.link_target = link_target;
        ctx
    }

    pub fn should_isolate(&self) -> bool {
        let Opacity(UnitInterval(opacity)) = self.opacity;
        match self.isolation {
            Isolation::Auto => {
                let is_opaque = approx_eq!(f64, opacity, 1.0);
                !(is_opaque
                    && self.filter.is_none()
                    && self.mask.is_none()
                    && self.mix_blend_mode == MixBlendMode::Normal
                    && self.clip_in_object_space.is_none())
            }
            Isolation::Isolate => true,
        }
    }
}

impl LayerKind {
    /// Gets the extents of a layer in its local coordinate system.
    ///
    /// Each object or layer is able to compute its own extents, in its local coordinate
    /// system.  When the parent group layer wants to take the union of the extents of its
    /// children, that parent group will need to convert the children's extents using each
    /// child layer's transform.
    pub fn local_extents(&self) -> Option<Rect> {
        match *self {
            LayerKind::Shape(ref shape) => shape.path.extents,
            LayerKind::Text(ref text) => text.extents,
            LayerKind::Image(ref image) => Some(image.rect),
            LayerKind::Group(ref group) => group.extents,
        }
    }
}

impl Stroke {
    pub fn new(values: &ComputedValues, params: &NormalizeParams) -> Stroke {
        let width = values.stroke_width().0.to_user(params);
        let miter_limit = values.stroke_miterlimit();
        let line_cap = values.stroke_line_cap();
        let line_join = values.stroke_line_join();
        let dash_offset = values.stroke_dashoffset().0.to_user(params);
        let non_scaling = values.vector_effect() == VectorEffect::NonScalingStroke;

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
            non_scaling,
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::document::Document;
    use crate::dpi::Dpi;

    fn assert_no_clip_path(svg: &'static [u8], element_with_clip_path: &str) {
        let document = Document::load_from_bytes(svg);

        let node = document
            .lookup_internal_node(element_with_clip_path)
            .unwrap();
        let elt = node.borrow_element();

        let mut acquired = AcquiredNodes::new(&document, None::<gio::Cancellable>);
        let session = Session::default();
        let font_options = FontOptions::new(true);
        let params = NormalizeParams::from_dpi(Dpi::new(96.0, 96.0));
        let viewport = Viewport::new(Dpi::new(96.0, 96.0), 1.0, 1.0);
        let clip_path = layout_clip_path(
            &session,
            &elt,
            &font_options,
            &mut acquired,
            &params,
            &viewport,
        );

        assert!(clip_path.is_none());
    }

    #[test]
    fn detects_no_clip_path() {
        assert_no_clip_path(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <rect id="foo"/>
</svg>
"#,
            "foo",
        );
    }

    #[test]
    fn detects_invalid_reference_to_clip_path() {
        assert_no_clip_path(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <rect id="foo" clip-path="url(#bar)"/>
</svg>
"#,
            "foo",
        );
    }

    #[test]
    fn detects_reference_that_is_not_a_clip_path() {
        assert_no_clip_path(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <defs>
    <rect id="bar"/> <!-- not a clipPath -->
  </defs>
  <rect id="foo" clip-path="url(#bar)"/>
</svg>
"#,
            "foo",
        );
    }

    #[test]
    fn decodes_basic_clip_path() {
        let document = Document::load_from_bytes(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <defs>
    <clipPath id="clip1" clipPathUnits="objectBoundingBox" transform="matrix(1.0 2.0 3.0 4.0 5.0 6.0)">
      <rect x="5" y="5" width="10" height="20" transform="matrix(2.0 3.0 4.0 5.0 6.0 7.0)" clip-rule="evenodd"/>
      <rect x="1" y="2" width="30" height="40" clip-path="url(#clip2)"/>
    </clipPath>
    <clipPath id="clip2">
      <rect x="6" y="6" width="5" height="6"/>
    </clipPath>
  </defs>
  <rect id="foo" clip-path="url(#clip1)"/>
</svg>
"#,
        );

        let node = document.lookup_internal_node("foo").unwrap();
        let elt = node.borrow_element();

        let mut acquired = AcquiredNodes::new(&document, None::<gio::Cancellable>);
        let session = Session::default();
        let font_options = FontOptions::new(true);
        let params = NormalizeParams::from_dpi(Dpi::new(96.0, 96.0));
        let viewport = Viewport::new(Dpi::new(96.0, 96.0), 1.0, 1.0);
        let clip_path = layout_clip_path(
            &session,
            &elt,
            &font_options,
            &mut acquired,
            &params,
            &viewport,
        )
        .expect("find a clipPath node");

        assert_eq!(clip_path.clip_units, CoordUnits::ObjectBoundingBox);
        assert_eq!(
            clip_path.transform,
            Transform::new_unchecked(1.0, 2.0, 3.0, 4.0, 5.0, 6.0)
        );
        assert_eq!(clip_path.paths.len(), 2);
        assert_eq!(
            clip_path.paths[0].transform,
            Transform::new_unchecked(2.0, 3.0, 4.0, 5.0, 6.0, 7.0)
        );
        assert_eq!(clip_path.paths[0].clip_rule, ClipRule::EvenOdd);
        assert!(clip_path.paths[0].clip_path.is_none());

        assert_eq!(clip_path.paths[1].clip_rule, ClipRule::NonZero);

        if let Some(ref sub_clip_path) = clip_path.paths[1].clip_path {
            assert_eq!(sub_clip_path.paths.len(), 1);
        } else {
            panic!("clip2 not found");
        }
    }

    #[test]
    fn decodes_nested_clip_path() {
        let document = Document::load_from_bytes(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <defs>
    <clipPath id="clip1" clip-path="url(#clip2)">
      <rect x="1" y="2" width="30" height="40"/>
    </clipPath>
    <clipPath id="clip2">
      <rect x="6" y="6" width="5" height="6"/>
    </clipPath>
  </defs>
  <rect id="foo" clip-path="url(#clip1)"/>
</svg>
"#,
        );

        let node = document.lookup_internal_node("foo").unwrap();
        let elt = node.borrow_element();

        let mut acquired = AcquiredNodes::new(&document, None::<gio::Cancellable>);
        let session = Session::default();
        let font_options = FontOptions::new(true);
        let params = NormalizeParams::from_dpi(Dpi::new(96.0, 96.0));
        let viewport = Viewport::new(Dpi::new(96.0, 96.0), 1.0, 1.0);
        let clip_path = layout_clip_path(
            &session,
            &elt,
            &font_options,
            &mut acquired,
            &params,
            &viewport,
        )
        .expect("find a clipPath node");

        assert_eq!(clip_path.paths.len(), 1);

        if let Some(ref nested_clip_path) = clip_path.clip_path {
            assert_eq!(nested_clip_path.paths.len(), 1);
        } else {
            panic!("clip2 not found");
        }
    }

    #[test]
    fn clip_path_does_not_include_invisible_children() {
        let document = Document::load_from_bytes(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <defs>
    <clipPath id="clip1">
      <rect x="1" y="2" width="30" height="40" visibility="hidden"/>
      <rect x="2" y="3" width="40" height="50" display="none"/>
    </clipPath>
  </defs>
  <rect id="foo" clip-path="url(#clip1)"/>
</svg>
"#,
        );

        let node = document.lookup_internal_node("foo").unwrap();
        let elt = node.borrow_element();

        let mut acquired = AcquiredNodes::new(&document, None::<gio::Cancellable>);
        let session = Session::default();
        let font_options = FontOptions::new(true);
        let params = NormalizeParams::from_dpi(Dpi::new(96.0, 96.0));
        let viewport = Viewport::new(Dpi::new(96.0, 96.0), 1.0, 1.0);
        let clip_path = layout_clip_path(
            &session,
            &elt,
            &font_options,
            &mut acquired,
            &params,
            &viewport,
        )
        .expect("find a clipPath node");

        assert_eq!(clip_path.paths.len(), 0);
    }

    #[test]
    fn clip_path_can_have_use_children() {
        let document = Document::load_from_bytes(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <rect id="rect" x="20" y="20" width="60" height="60"/>
    <clipPath id="clip1">
      <use href="#rect" x="20" y="20"/>/>
    </clipPath>
  </defs>

  <rect width="100%" height="100%" fill="white"/>

  <rect id="foo" x="20" y="20" width="60" height="60" fill="#00ff00" clip-path="url(#clip1)"/>
</svg>
"##,
        );

        let node = document.lookup_internal_node("foo").unwrap();
        let elt = node.borrow_element();

        let mut acquired = AcquiredNodes::new(&document, None::<gio::Cancellable>);
        let session = Session::default();
        let font_options = FontOptions::new(true);
        let params = NormalizeParams::from_dpi(Dpi::new(96.0, 96.0));
        let viewport = Viewport::new(Dpi::new(96.0, 96.0), 1.0, 1.0);
        let clip_path = layout_clip_path(
            &session,
            &elt,
            &font_options,
            &mut acquired,
            &params,
            &viewport,
        )
        .expect("find a clipPath node");

        assert_eq!(clip_path.paths.len(), 1);

        let item = &clip_path.paths[0];

        assert_eq!(item.transform, Transform::new_translate(20.0, 20.0));
    }

    #[test]
    fn clip_path_handles_use_with_nonexistent_href() {
        let document = Document::load_from_bytes(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <clipPath id="clip1">
      <use href="#nonexistent"/>
    </clipPath>
  </defs>

  <rect width="100%" height="100%" fill="white"/>

  <rect id="foo" x="20" y="20" width="60" height="60" fill="#00ff00" clip-path="url(#clip1)"/>
</svg>
"##,
        );

        let node = document.lookup_internal_node("foo").unwrap();
        let elt = node.borrow_element();

        let mut acquired = AcquiredNodes::new(&document, None::<gio::Cancellable>);
        let session = Session::default();
        let font_options = FontOptions::new(true);
        let params = NormalizeParams::from_dpi(Dpi::new(96.0, 96.0));
        let viewport = Viewport::new(Dpi::new(96.0, 96.0), 1.0, 1.0);
        let clip_path = layout_clip_path(
            &session,
            &elt,
            &font_options,
            &mut acquired,
            &params,
            &viewport,
        )
        .expect("find a clipPath node");

        assert!(clip_path.paths.is_empty());
    }

    #[test]
    fn clip_path_handles_use_with_no_href() {
        let document = Document::load_from_bytes(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
  <defs>
    <clipPath id="clip1">
      <use/>
    </clipPath>
  </defs>

  <rect width="100%" height="100%" fill="white"/>

  <rect id="foo" x="20" y="20" width="60" height="60" fill="#00ff00" clip-path="url(#clip1)"/>
</svg>
"##,
        );

        let node = document.lookup_internal_node("foo").unwrap();
        let elt = node.borrow_element();

        let mut acquired = AcquiredNodes::new(&document, None::<gio::Cancellable>);
        let session = Session::default();
        let font_options = FontOptions::new(true);
        let params = NormalizeParams::from_dpi(Dpi::new(96.0, 96.0));
        let viewport = Viewport::new(Dpi::new(96.0, 96.0), 1.0, 1.0);
        let clip_path = layout_clip_path(
            &session,
            &elt,
            &font_options,
            &mut acquired,
            &params,
            &viewport,
        )
        .expect("find a clipPath node");

        assert!(clip_path.paths.is_empty());
    }
}
