use glib::translate::*;
use libc;
use std::collections::HashMap;
use std::ptr;

use attributes::Attribute;
use clip_path::NodeClipPath;
use filters::composite::Composite;
use filters::image::Image;
use filters::merge::{Merge, MergeNode};
use filters::node::NodeFilter;
use filters::offset::Offset;
use gradient::NodeGradient;
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

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_new_filter_primitive_blend(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_color_matrix(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_component_transfer(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_convolve_matrix(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_diffuse_lighting(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_displacement_map(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_node_light_source(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_flood(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_node_component_transfer_function(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_gaussian_blur(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_erode(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_specular_lighting(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_tile(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_turbulence(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
}

type NodeCreateCFn = unsafe extern "C" fn(
    *const libc::c_char,
    *const RsvgNode,
    *const libc::c_char,
    *const libc::c_char,
) -> *const RsvgNode;

lazy_static! {
    // Lines in comments are elements that we don't support.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    static ref NODE_CREATORS_C: HashMap<&'static str, (bool, NodeCreateCFn)> = {
        let mut h = HashMap::new();
        h.insert("feBlend",             (true,  rsvg_new_filter_primitive_blend as NodeCreateCFn));
        h.insert("feColorMatrix",       (true,  rsvg_new_filter_primitive_color_matrix as NodeCreateCFn));
        h.insert("feComponentTransfer", (true,  rsvg_new_filter_primitive_component_transfer as NodeCreateCFn));
        h.insert("feConvolveMatrix",    (true,  rsvg_new_filter_primitive_convolve_matrix as NodeCreateCFn));
        h.insert("feDiffuseLighting",   (true,  rsvg_new_filter_primitive_diffuse_lighting as NodeCreateCFn));
        h.insert("feDisplacementMap",   (true,  rsvg_new_filter_primitive_displacement_map as NodeCreateCFn));
        h.insert("feDistantLight",      (false, rsvg_new_node_light_source as NodeCreateCFn));
        h.insert("feFlood",             (true,  rsvg_new_filter_primitive_flood as NodeCreateCFn));
        h.insert("feFuncA",             (false, rsvg_new_node_component_transfer_function as NodeCreateCFn));
        h.insert("feFuncB",             (false, rsvg_new_node_component_transfer_function as NodeCreateCFn));
        h.insert("feFuncG",             (false, rsvg_new_node_component_transfer_function as NodeCreateCFn));
        h.insert("feFuncR",             (false, rsvg_new_node_component_transfer_function as NodeCreateCFn));
        h.insert("feGaussianBlur",      (true,  rsvg_new_filter_primitive_gaussian_blur as NodeCreateCFn));
        h.insert("feMorphology",        (true,  rsvg_new_filter_primitive_erode as NodeCreateCFn));
        h.insert("fePointLight",        (false, rsvg_new_node_light_source as NodeCreateCFn));
        h.insert("feSpecularLighting",  (true,  rsvg_new_filter_primitive_specular_lighting as NodeCreateCFn));
        h.insert("feSpotLight",         (false, rsvg_new_node_light_source as NodeCreateCFn));
        h.insert("feTile",              (true,  rsvg_new_filter_primitive_tile as NodeCreateCFn));
        h.insert("feTurbulence",        (true,  rsvg_new_filter_primitive_turbulence as NodeCreateCFn));
        h
    };
}

macro_rules! node_create_fn {
    ($name:ident, $node_type:ident, $new_fn:expr) => {
        fn $name(
            id: Option<&str>,
            class: Option<&str>,
            parent: *const RsvgNode,
        ) -> *const RsvgNode {
            boxed_node_new(NodeType::$node_type, parent, id, class, Box::new($new_fn()))
        }
    };
}

node_create_fn!(create_circle, Circle, NodeCircle::new);
node_create_fn!(create_clip_path, ClipPath, NodeClipPath::new);
node_create_fn!(create_composite, FilterPrimitiveComposite, Composite::new);
node_create_fn!(create_defs, Defs, NodeDefs::new);
node_create_fn!(create_ellipse, Ellipse, NodeEllipse::new);
node_create_fn!(create_filter, Filter, NodeFilter::new);
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
node_create_fn!(create_offset, FilterPrimitiveOffset, Offset::new);
node_create_fn!(create_path, Path, NodePath::new);
node_create_fn!(create_pattern, Pattern, NodePattern::new);
node_create_fn!(create_polygon, Polygon, NodePoly::new_closed);
node_create_fn!(create_polyline, Polyline, NodePoly::new_open);
node_create_fn!(
    create_radial_gradient,
    RadialGradient,
    NodeGradient::new_radial
);
node_create_fn!(create_rect, Rect, NodeRect::new);
node_create_fn!(create_stop, Stop, NodeStop::new);
node_create_fn!(create_svg, Svg, NodeSvg::new);
node_create_fn!(create_switch, Switch, NodeSwitch::new);
node_create_fn!(create_symbol, Symbol, NodeSymbol::new);
node_create_fn!(create_text, Text, NodeText::new);
node_create_fn!(create_tref, TRef, NodeTRef::new);
node_create_fn!(create_tspan, TSpan, NodeTSpan::new);
node_create_fn!(create_use, Use, NodeUse::new);

type NodeCreateFn = fn(Option<&str>, Option<&str>, *const RsvgNode) -> *const RsvgNode;

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
        h.insert("feComposite",         (true,  create_composite as NodeCreateFn));
        h.insert("feImage",             (true,  create_fe_image as NodeCreateFn));
        h.insert("feMerge",             (true,  create_merge as NodeCreateFn));
        h.insert("feMergeNode",         (false, create_merge_node as NodeCreateFn));
        h.insert("feOffset",            (true,  create_offset as NodeCreateFn));
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
    pbag: *const PropertyBag,
) -> *const RsvgNode {
    assert!(!raw_name.is_null());
    assert!(!pbag.is_null());

    let name = unsafe { utf8_cstr(raw_name) };
    let pbag = unsafe { &*pbag };

    let mut id = None;
    let mut class = None;

    for (_key, attr, value) in pbag.iter() {
        match attr {
            Attribute::Id => id = Some(value),
            Attribute::Class => class = Some(value),
            _ => (),
        }
    }

    // Legacy C creators
    if let Some(&(supports_class, create_fn)) = NODE_CREATORS_C.get(name) {
        let id = match id {
            Some(id) => id.to_glib_none().0,
            None => ptr::null(),
        };
        let class = match class {
            Some(class) if supports_class => class.to_glib_none().0,
            _ => ptr::null(),
        };

        unsafe {
            return create_fn(raw_name, parent, id, class);
        }
    }

    let &(supports_class, create_fn) = match NODE_CREATORS.get(name) {
        Some(c) => c,
        // Whenever we encounter a node we don't understand, represent it as a defs.
        // This is like a group, but it doesn't do any rendering of children.  The
        // effect is that we will ignore all children of unknown elements.
        //
        None => &(true, create_defs as NodeCreateFn),
    };

    if !supports_class {
        class = None;
    };

    create_fn(id, class, parent)
}
