use libc;
use std::collections::HashMap;

use attributes::Attribute;
use clip_path::NodeClipPath;
use defs::Defs;
use filters::{
    blend::Blend,
    color_matrix::ColorMatrix,
    component_transfer::{ComponentTransfer, FuncX},
    composite::Composite,
    convolve_matrix::ConvolveMatrix,
    displacement_map::DisplacementMap,
    flood::Flood,
    gaussian_blur::GaussianBlur,
    image::Image,
    light::{light_source::LightSource, lighting::Lighting},
    merge::{Merge, MergeNode},
    morphology::Morphology,
    node::NodeFilter,
    offset::Offset,
    tile::Tile,
    turbulence::Turbulence,
};
use gradient::NodeGradient;
use handle::RsvgHandle;
use image::NodeImage;
use link::NodeLink;
use marker::NodeMarker;
use mask::NodeMask;
use node::*;
use pattern::NodePattern;
use property_bag::PropertyBag;
use shapes::{NodeCircle, NodeEllipse, NodeLine, NodePath, NodePoly, NodeRect};
use stop::NodeStop;
use structure::{NodeDefs, NodeGroup, NodeSvg, NodeSwitch, NodeSymbol, NodeUse};
use text::{NodeTRef, NodeTSpan, NodeText};
use util::utf8_cstr;

macro_rules! node_create_fn {
    ($name:ident, $node_type:ident, $new_fn:expr) => {
        fn $name(
            element_name: &str,
            id: Option<&str>,
            class: Option<&str>,
            parent: *const RsvgNode,
        ) -> *const RsvgNode {
            boxed_node_new(
                NodeType::$node_type,
                parent,
                element_name,
                id,
                class,
                Box::new($new_fn()),
            )
        }
    };
}

node_create_fn!(create_circle, Circle, NodeCircle::new);
node_create_fn!(create_clip_path, ClipPath, NodeClipPath::new);
node_create_fn!(create_blend, FilterPrimitiveBlend, Blend::new);
node_create_fn!(
    create_color_matrix,
    FilterPrimitiveColorMatrix,
    ColorMatrix::new
);
node_create_fn!(
    create_component_transfer,
    FilterPrimitiveComponentTransfer,
    ComponentTransfer::new
);
node_create_fn!(
    create_component_transfer_func_r,
    ComponentTransferFunction,
    FuncX::new_r
);
node_create_fn!(
    create_component_transfer_func_g,
    ComponentTransferFunction,
    FuncX::new_g
);
node_create_fn!(
    create_component_transfer_func_b,
    ComponentTransferFunction,
    FuncX::new_b
);
node_create_fn!(
    create_component_transfer_func_a,
    ComponentTransferFunction,
    FuncX::new_a
);
node_create_fn!(create_composite, FilterPrimitiveComposite, Composite::new);
node_create_fn!(
    create_convolve_matrix,
    FilterPrimitiveConvolveMatrix,
    ConvolveMatrix::new
);
node_create_fn!(create_defs, Defs, NodeDefs::new);
node_create_fn!(
    create_diffuse_lighting,
    FilterPrimitiveDiffuseLighting,
    Lighting::new_diffuse
);
node_create_fn!(
    create_distant_light,
    LightSource,
    LightSource::new_distant_light
);
node_create_fn!(
    create_displacement_map,
    FilterPrimitiveDisplacementMap,
    DisplacementMap::new
);
node_create_fn!(create_ellipse, Ellipse, NodeEllipse::new);
node_create_fn!(create_filter, Filter, NodeFilter::new);
node_create_fn!(create_flood, FilterPrimitiveFlood, Flood::new);
node_create_fn!(
    create_gaussian_blur,
    FilterPrimitiveGaussianBlur,
    GaussianBlur::new
);
node_create_fn!(create_group, Group, NodeGroup::new);
node_create_fn!(create_image, Image, NodeImage::new);
node_create_fn!(create_fe_image, FilterPrimitiveImage, Image::new);
node_create_fn!(
    create_linear_gradient,
    LinearGradient,
    NodeGradient::new_linear
);
node_create_fn!(create_line, Line, NodeLine::new);
node_create_fn!(create_link, Link, NodeLink::new);
node_create_fn!(create_marker, Marker, NodeMarker::new);
node_create_fn!(create_mask, Mask, NodeMask::new);
node_create_fn!(create_merge, FilterPrimitiveMerge, Merge::new);
node_create_fn!(create_merge_node, FilterPrimitiveMergeNode, MergeNode::new);
node_create_fn!(
    create_morphology,
    FilterPrimitiveMorphology,
    Morphology::new
);
node_create_fn!(create_offset, FilterPrimitiveOffset, Offset::new);
node_create_fn!(create_path, Path, NodePath::new);
node_create_fn!(create_pattern, Pattern, NodePattern::new);
node_create_fn!(
    create_point_light,
    LightSource,
    LightSource::new_point_light
);
node_create_fn!(create_polygon, Polygon, NodePoly::new_closed);
node_create_fn!(create_polyline, Polyline, NodePoly::new_open);
node_create_fn!(
    create_radial_gradient,
    RadialGradient,
    NodeGradient::new_radial
);
node_create_fn!(create_rect, Rect, NodeRect::new);
node_create_fn!(
    create_specular_lighting,
    FilterPrimitiveSpecularLighting,
    Lighting::new_specular
);
node_create_fn!(create_spot_light, LightSource, LightSource::new_spot_light);
node_create_fn!(create_stop, Stop, NodeStop::new);
node_create_fn!(create_svg, Svg, NodeSvg::new);
node_create_fn!(create_switch, Switch, NodeSwitch::new);
node_create_fn!(create_symbol, Symbol, NodeSymbol::new);
node_create_fn!(create_text, Text, NodeText::new);
node_create_fn!(create_tref, TRef, NodeTRef::new);
node_create_fn!(create_tspan, TSpan, NodeTSpan::new);
node_create_fn!(create_tile, FilterPrimitiveTile, Tile::new);
node_create_fn!(
    create_turbulence,
    FilterPrimitiveTurbulence,
    Turbulence::new
);
node_create_fn!(create_use, Use, NodeUse::new);

type NodeCreateFn = fn(&str, Option<&str>, Option<&str>, *const RsvgNode) -> *const RsvgNode;

lazy_static! {
    // Lines in comments are elements that we don't support.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    static ref NODE_CREATORS: HashMap<&'static str, (bool, NodeCreateFn)> = {
        let mut h = HashMap::new();
        h.insert("a",                   (true,  create_link as NodeCreateFn));
        /* h.insert("altGlyph",         (true,  as NodeCreateFn)); */
        /* h.insert("altGlyphDef",      (false, as NodeCreateFn)); */
        /* h.insert("altGlyphItem",     (false, as NodeCreateFn)); */
        /* h.insert("animate",          (false, as NodeCreateFn)); */
        /* h.insert("animateColor",     (false, as NodeCreateFn)); */
        /* h.insert("animateMotion",    (false, as NodeCreateFn)); */
        /* h.insert("animateTransform", (false, as NodeCreateFn)); */
        h.insert("circle",              (true,  create_circle as NodeCreateFn));
        h.insert("clipPath",            (true,  create_clip_path as NodeCreateFn));
        /* h.insert("color-profile",    (false, as NodeCreateFn)); */
        h.insert("conicalGradient",     (true,  create_radial_gradient as NodeCreateFn));
        /* h.insert("cursor",           (false, as NodeCreateFn)); */
        h.insert("defs",                (true,  create_defs as NodeCreateFn));
        /* h.insert("desc",             (true,  as NodeCreateFn)); */
        h.insert("ellipse",             (true,  create_ellipse as NodeCreateFn));
        h.insert("feBlend",             (true,  create_blend as NodeCreateFn));
        h.insert("feColorMatrix",       (true,  create_color_matrix as NodeCreateFn));
        h.insert("feComponentTransfer", (true,  create_component_transfer as NodeCreateFn));
        h.insert("feComposite",         (true,  create_composite as NodeCreateFn));
        h.insert("feConvolveMatrix",    (true,  create_convolve_matrix as NodeCreateFn));
        h.insert("feDiffuseLighting",   (true,  create_diffuse_lighting as NodeCreateFn));
        h.insert("feDisplacementMap",   (true,  create_displacement_map as NodeCreateFn));
        h.insert("feDistantLight",      (false, create_distant_light as NodeCreateFn));
        h.insert("feFuncR",             (false, create_component_transfer_func_r as NodeCreateFn));
        h.insert("feFuncG",             (false, create_component_transfer_func_g as NodeCreateFn));
        h.insert("feFuncB",             (false, create_component_transfer_func_b as NodeCreateFn));
        h.insert("feFuncA",             (false, create_component_transfer_func_a as NodeCreateFn));
        h.insert("feFlood",             (true,  create_flood as NodeCreateFn));
        h.insert("feGaussianBlur",      (true,  create_gaussian_blur as NodeCreateFn));
        h.insert("feImage",             (true,  create_fe_image as NodeCreateFn));
        h.insert("feMerge",             (true,  create_merge as NodeCreateFn));
        h.insert("feMergeNode",         (false, create_merge_node as NodeCreateFn));
        h.insert("feMorphology",        (true,  create_morphology as NodeCreateFn));
        h.insert("feOffset",            (true,  create_offset as NodeCreateFn));
        h.insert("fePointLight",        (false, create_point_light as NodeCreateFn));
        h.insert("feSpecularLighting",  (true,  create_specular_lighting as NodeCreateFn));
        h.insert("feSpotLight",         (false, create_spot_light as NodeCreateFn));
        h.insert("feTile",              (true,  create_tile as NodeCreateFn));
        h.insert("feTurbulence",        (true,  create_turbulence as NodeCreateFn));
        h.insert("filter",              (true,  create_filter as NodeCreateFn));
        /* h.insert("font",             (true,  as NodeCreateFn)); */
        /* h.insert("font-face",        (false, as NodeCreateFn)); */
        /* h.insert("font-face-format", (false, as NodeCreateFn)); */
        /* h.insert("font-face-name",   (false, as NodeCreateFn)); */
        /* h.insert("font-face-src",    (false, as NodeCreateFn)); */
        /* h.insert("font-face-uri",    (false, as NodeCreateFn)); */
        /* h.insert("foreignObject",    (true,  as NodeCreateFn)); */
        h.insert("g",                   (true,  create_group as NodeCreateFn));
        /* h.insert("glyph",            (true,  as NodeCreateFn)); */
        /* h.insert("glyphRef",         (true,  as NodeCreateFn)); */
        /* h.insert("hkern",            (false, as NodeCreateFn)); */
        h.insert("image",               (true,  create_image as NodeCreateFn));
        h.insert("line",                (true,  create_line as NodeCreateFn));
        h.insert("linearGradient",      (true,  create_linear_gradient as NodeCreateFn));
        h.insert("marker",              (true,  create_marker as NodeCreateFn));
        h.insert("mask",                (true,  create_mask as NodeCreateFn));
        /* h.insert("metadata",         (false, as NodeCreateFn)); */
        /* h.insert("missing-glyph",    (true,  as NodeCreateFn)); */
        /* h.insert("mpath",            (false, as NodeCreateFn)); */
        h.insert("multiImage",          (false, create_switch as NodeCreateFn)); // hack to make multiImage sort-of work
        h.insert("path",                (true,  create_path as NodeCreateFn));
        h.insert("pattern",             (true,  create_pattern as NodeCreateFn));
        h.insert("polygon",             (true,  create_polygon as NodeCreateFn));
        h.insert("polyline",            (true,  create_polyline as NodeCreateFn));
        h.insert("radialGradient",      (true,  create_radial_gradient as NodeCreateFn));
        h.insert("rect",                (true,  create_rect as NodeCreateFn));
        /* h.insert("script",           (false, as NodeCreateFn)); */
        /* h.insert("set",              (false, as NodeCreateFn)); */
        h.insert("stop",                (true,  create_stop as NodeCreateFn));
        /* h.insert("style",            (false, as NodeCreateFn)); */
        h.insert("subImage",            (false, create_group as NodeCreateFn));
        h.insert("subImageRef",         (false, create_image as NodeCreateFn));
        h.insert("svg",                 (true,  create_svg as NodeCreateFn));
        h.insert("switch",              (true,  create_switch as NodeCreateFn));
        h.insert("symbol",              (true,  create_symbol as NodeCreateFn));
        h.insert("text",                (true,  create_text as NodeCreateFn));
        /* h.insert("textPath",         (true,  as NodeCreateFn)); */
        /* h.insert("title",            (true,  as NodeCreateFn)); */
        h.insert("tref",                (true,  create_tref as NodeCreateFn));
        h.insert("tspan",               (true,  create_tspan as NodeCreateFn));
        h.insert("use",                 (true,  create_use as NodeCreateFn));
        /* h.insert("view",             (false, as NodeCreateFn)); */
        /* h.insert("vkern",            (false, as NodeCreateFn)); */
        h
    };
}

#[no_mangle]
pub extern "C" fn rsvg_load_new_node(
    raw_name: *const libc::c_char,
    parent: *const RsvgNode,
    pbag: *const PropertyBag<'_>,
    defs: *mut Defs,
) -> *const RsvgNode {
    assert!(!raw_name.is_null());
    assert!(!pbag.is_null());
    assert!(!defs.is_null());

    let name = unsafe { utf8_cstr(raw_name) };
    let pbag = unsafe { &*pbag };
    let defs = unsafe { &mut *defs };

    let mut id = None;
    let mut class = None;

    for (_key, attr, value) in pbag.iter() {
        match attr {
            Attribute::Id => id = Some(value),
            Attribute::Class => class = Some(value),
            _ => (),
        }
    }

    let &(supports_class, create_fn) = match NODE_CREATORS.get(name) {
        Some(c) => c,
        // Whenever we encounter a node we don't understand, represent it as a defs.
        // This is like a group, but it doesn't do any rendering of children.  The
        // effect is that we will ignore all children of unknown elements.
        None => &(true, create_defs as NodeCreateFn),
    };

    if !supports_class {
        class = None;
    };

    let node = create_fn(name, id, class, parent);
    assert!(!node.is_null());

    if id.is_some() {
        let n = unsafe { &*node };
        defs.insert(id.unwrap(), n);
    }

    node
}

#[no_mangle]
pub extern "C" fn rsvg_load_set_node_atts(
    handle: *const RsvgHandle,
    raw_node: *mut RsvgNode,
    pbag: *const PropertyBag<'_>,
) {
    assert!(!raw_node.is_null());
    assert!(!pbag.is_null());

    let node: &RsvgNode = unsafe { &*raw_node };
    let pbag = unsafe { &*pbag };

    node.set_atts(node, handle, pbag);

    // The "svg" node is special; it will load its id/class
    // attributes until the end, when sax_end_element_cb() calls
    // rsvg_node_svg_apply_atts()
    if node.get_type() != NodeType::Svg {
        node.set_style(handle, pbag);
    }

    node.set_overridden_properties();
}

#[no_mangle]
pub extern "C" fn rsvg_load_set_svg_node_atts(
    handle: *const RsvgHandle,
    raw_node: *const RsvgNode,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    if node.get_type() != NodeType::Svg {
        return;
    }

    node.with_impl(|svg: &NodeSvg| {
        svg.set_delayed_style(node, handle);
    });
}
