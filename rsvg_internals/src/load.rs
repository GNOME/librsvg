use glib::translate::*;
use glib_sys;
use libc;
use std::collections::HashMap;

use clip_path::NodeClipPath;
use filters::composite::Composite;
use filters::merge::{Merge, MergeNode};
use filters::node::NodeFilter;
use filters::offset::Offset;
use gradient::NodeGradient;
use image::NodeImage;
use link::NodeLink;
use marker::NodeMarker;
use mask::NodeMask;
use pattern::NodePattern;
use node::*;
use shapes::{NodeCircle, NodeEllipse, NodePath, NodePoly, NodeLine, NodeRect};
use structure::{NodeDefs, NodeGroup, NodeSymbol, NodeSwitch, NodeSvg, NodeUse};
use stop::NodeStop;
use text::{NodeText, NodeTRef, NodeTSpan};
use util::{utf8_cstr, utf8_cstr_opt};

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_new_filter_primitive_blend(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_color_matrix(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_component_transfer(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_convolve_matrix(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_diffuse_lighting(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_displacement_map(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_node_light_source(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_flood(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_node_component_transfer_function(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_gaussian_blur(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_image(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_erode(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_specular_lighting(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_tile(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
    fn rsvg_new_filter_primitive_turbulence(
        _: *const libc::c_char,
        _: *const RsvgNode,
        _: *const libc::c_char,
    ) -> *const RsvgNode;
}

extern "C" fn rsvg_node_clip_path_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::ClipPath,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeClipPath::new()),
    )
}

extern "C" fn rsvg_node_link_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Link,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeLink::new()),
    )
}

extern "C" fn rsvg_node_marker_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Marker,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeMarker::new()),
    )
}

extern "C" fn rsvg_node_mask_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Mask,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeMask::new()),
    )
}

extern "C" fn rsvg_node_pattern_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Pattern,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodePattern::new()),
    )
}

extern "C" fn rsvg_node_path_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Path,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodePath::new()),
    )
}

extern "C" fn rsvg_node_polygon_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Path,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodePoly::new_closed()),
    )
}

extern "C" fn rsvg_node_polyline_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Path,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodePoly::new_open()),
    )
}

extern "C" fn rsvg_node_line_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Line,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeLine::new()),
    )
}

extern "C" fn rsvg_node_rect_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Rect,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeRect::new()),
    )
}

extern "C" fn rsvg_node_circle_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Circle,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeCircle::new()),
    )
}

extern "C" fn rsvg_node_ellipse_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Ellipse,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeEllipse::new()),
    )
}
extern "C" fn rsvg_node_group_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Group,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeGroup::new()),
    )
}

extern "C" fn rsvg_node_defs_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Defs,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeDefs::new()),
    )
}

extern "C" fn rsvg_node_switch_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Switch,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeSwitch::new()),
    )
}

extern "C" fn rsvg_node_svg_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Svg,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeSvg::new()),
    )
}

extern "C" fn rsvg_node_use_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Use,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeUse::new()),
    )
}

extern "C" fn rsvg_node_symbol_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Symbol,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeSymbol::new()),
    )
}

extern "C" fn rsvg_node_stop_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Stop,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeStop::new()),
    )
}

extern "C" fn rsvg_node_text_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Text,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeText::new()),
    )
}

extern "C" fn rsvg_node_tref_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::TRef,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeTRef::new()),
    )
}

extern "C" fn rsvg_node_tspan_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::TSpan,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeTSpan::new()),
    )
}

// filters

/// Returns a new `feComposite` node.
extern "C" fn rsvg_new_filter_primitive_composite(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    let filter = Composite::new();
    boxed_node_new(
        NodeType::FilterPrimitiveComposite,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(filter),
    )
}

/// Returns a new `feMerge` node.
extern "C" fn rsvg_new_filter_primitive_merge(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    let filter = Merge::new();
    boxed_node_new(
        NodeType::FilterPrimitiveMerge,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(filter),
    )
}

/// Returns a new `feMergeNode` node.
extern "C" fn rsvg_new_filter_primitive_merge_node(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    let filter = MergeNode::new();
    boxed_node_new(
        NodeType::FilterPrimitiveMergeNode,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(filter),
    )
}

/// Returns a new `feOffset` node.
extern "C" fn rsvg_new_filter_primitive_offset(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    let filter = Offset::new();
    boxed_node_new(
        NodeType::FilterPrimitiveOffset,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(filter),
    )
}

extern "C" fn rsvg_node_linear_gradient_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::LinearGradient,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeGradient::new_linear()),
    )
}

extern "C" fn rsvg_node_radial_gradient_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::RadialGradient,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeGradient::new_radial()),
    )
}

extern "C" fn rsvg_node_image_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Image,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeImage::new()),
    )
}

extern "C" fn rsvg_new_filter(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
    id: *const libc::c_char,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Filter,
        raw_parent,
        unsafe { utf8_cstr_opt(id) },
        Box::new(NodeFilter::new()),
    )
}

type NodeCreateFn =
    unsafe extern "C" fn(*const libc::c_char, *const RsvgNode, id: *const libc::c_char)
        -> *const RsvgNode;

lazy_static! {
    // Lines in comments are elements that we don't support.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    static ref NODE_CREATORS: HashMap<&'static str, (bool, NodeCreateFn)> = {
        let mut h = HashMap::new();
        h.insert("a",                   (true,  rsvg_node_link_new as NodeCreateFn));
        /* h.insert("altGlyph",         (true,   as NodeCreateFn)); */
        /* h.insert("altGlyphDef",      (false,  as NodeCreateFn)); */
        /* h.insert("altGlyphItem",     (false,  as NodeCreateFn)); */
        /* h.insert("animate",          (false,  as NodeCreateFn)); */
        /* h.insert("animateColor",     (false,  as NodeCreateFn)); */
        /* h.insert("animateMotion",    (false,  as NodeCreateFn)); */
        /* h.insert("animateTransform", (false,  as NodeCreateFn)); */
        h.insert("circle",              (true,  rsvg_node_circle_new as NodeCreateFn));
        h.insert("clipPath",            (true,  rsvg_node_clip_path_new as NodeCreateFn));
        /* h.insert("color-profile",    (false,  as NodeCreateFn)); */
        h.insert("conicalGradient",     (true,  rsvg_node_radial_gradient_new as NodeCreateFn));
        /* h.insert("cursor",           (false,  as NodeCreateFn)); */
        h.insert("defs",                (true,  rsvg_node_defs_new as NodeCreateFn));
        /* h.insert("desc",             (true,   as NodeCreateFn)); */
        h.insert("ellipse",             (true,  rsvg_node_ellipse_new as NodeCreateFn));
        h.insert("feBlend",             (true,  rsvg_new_filter_primitive_blend as NodeCreateFn));
        h.insert("feColorMatrix",       (true,  rsvg_new_filter_primitive_color_matrix as NodeCreateFn));
        h.insert("feComponentTransfer", (true,  rsvg_new_filter_primitive_component_transfer as NodeCreateFn));
        h.insert("feComposite",         (true,  rsvg_new_filter_primitive_composite as NodeCreateFn));
        h.insert("feConvolveMatrix",    (true,  rsvg_new_filter_primitive_convolve_matrix as NodeCreateFn));
        h.insert("feDiffuseLighting",   (true,  rsvg_new_filter_primitive_diffuse_lighting as NodeCreateFn));
        h.insert("feDisplacementMap",   (true,  rsvg_new_filter_primitive_displacement_map as NodeCreateFn));
        h.insert("feDistantLight",      (false, rsvg_new_node_light_source as NodeCreateFn));
        h.insert("feFlood",             (true,  rsvg_new_filter_primitive_flood as NodeCreateFn));
        h.insert("feFuncA",             (false, rsvg_new_node_component_transfer_function as NodeCreateFn));
        h.insert("feFuncB",             (false, rsvg_new_node_component_transfer_function as NodeCreateFn));
        h.insert("feFuncG",             (false, rsvg_new_node_component_transfer_function as NodeCreateFn));
        h.insert("feFuncR",             (false, rsvg_new_node_component_transfer_function as NodeCreateFn));
        h.insert("feGaussianBlur",      (true,  rsvg_new_filter_primitive_gaussian_blur as NodeCreateFn));
        h.insert("feImage",             (true,  rsvg_new_filter_primitive_image as NodeCreateFn));
        h.insert("feMerge",             (true,  rsvg_new_filter_primitive_merge as NodeCreateFn));
        h.insert("feMergeNode",         (false, rsvg_new_filter_primitive_merge_node as NodeCreateFn));
        h.insert("feMorphology",        (true,  rsvg_new_filter_primitive_erode as NodeCreateFn));
        h.insert("feOffset",            (true,  rsvg_new_filter_primitive_offset as NodeCreateFn));
        h.insert("fePointLight",        (false, rsvg_new_node_light_source as NodeCreateFn));
        h.insert("feSpecularLighting",  (true,  rsvg_new_filter_primitive_specular_lighting as NodeCreateFn));
        h.insert("feSpotLight",         (false, rsvg_new_node_light_source as NodeCreateFn));
        h.insert("feTile",              (true,  rsvg_new_filter_primitive_tile as NodeCreateFn));
        h.insert("feTurbulence",        (true,  rsvg_new_filter_primitive_turbulence as NodeCreateFn));
        h.insert("filter",              (true,  rsvg_new_filter as NodeCreateFn));
        /* h.insert("font",             (true,   as NodeCreateFn)); */
        /* h.insert("font-face",        (false,  as NodeCreateFn)); */
        /* h.insert("font-face-format", (false,  as NodeCreateFn)); */
        /* h.insert("font-face-name",   (false,  as NodeCreateFn)); */
        /* h.insert("font-face-src",    (false,  as NodeCreateFn)); */
        /* h.insert("font-face-uri",    (false,  as NodeCreateFn)); */
        /* h.insert("foreignObject",    (true,   as NodeCreateFn)); */
        h.insert("g",                   (true,  rsvg_node_group_new as NodeCreateFn));
        /* h.insert("glyph",            (true,   as NodeCreateFn)); */
        /* h.insert("glyphRef",         (true,   as NodeCreateFn)); */
        /* h.insert("hkern",            (false,  as NodeCreateFn)); */
        h.insert("image",               (true,  rsvg_node_image_new as NodeCreateFn));
        h.insert("line",                (true,  rsvg_node_line_new as NodeCreateFn));
        h.insert("linearGradient",      (true,  rsvg_node_linear_gradient_new as NodeCreateFn));
        h.insert("marker",              (true,  rsvg_node_marker_new as NodeCreateFn));
        h.insert("mask",                (true,  rsvg_node_mask_new as NodeCreateFn));
        /* h.insert("metadata",         (false,  as NodeCreateFn)); */
        /* h.insert("missing-glyph",    (true,   as NodeCreateFn)); */
        /* h.insert("mpath",            (false,  as NodeCreateFn)); */
        h.insert("multiImage",          (false, rsvg_node_switch_new as NodeCreateFn)); /* hack to make multiImage sort-of work */
        h.insert("path",                (true,  rsvg_node_path_new as NodeCreateFn));
        h.insert("pattern",             (true,  rsvg_node_pattern_new as NodeCreateFn));
        h.insert("polygon",             (true,  rsvg_node_polygon_new as NodeCreateFn));
        h.insert("polyline",            (true,  rsvg_node_polyline_new as NodeCreateFn));
        h.insert("radialGradient",      (true,  rsvg_node_radial_gradient_new as NodeCreateFn));
        h.insert("rect",                (true,  rsvg_node_rect_new as NodeCreateFn));
        /* h.insert("script",           (false,  as NodeCreateFn)); */
        /* h.insert("set",              (false,  as NodeCreateFn)); */
        h.insert("stop",                (true,  rsvg_node_stop_new as NodeCreateFn));
        /* h.insert("style",            (false,  as NodeCreateFn)); */
        h.insert("subImage",            (false, rsvg_node_group_new as NodeCreateFn));
        h.insert("subImageRef",         (false, rsvg_node_image_new as NodeCreateFn));
        h.insert("svg",                 (true,  rsvg_node_svg_new as NodeCreateFn));
        h.insert("switch",              (true,  rsvg_node_switch_new as NodeCreateFn));
        h.insert("symbol",              (true,  rsvg_node_symbol_new as NodeCreateFn));
        h.insert("text",                (true,  rsvg_node_text_new as NodeCreateFn));
        /* h.insert("textPath",         (true,   as NodeCreateFn)); */
        /* h.insert("title",            (true,   as NodeCreateFn)); */
        h.insert("tref",                (true,  rsvg_node_tref_new as NodeCreateFn));
        h.insert("tspan",               (true,  rsvg_node_tspan_new as NodeCreateFn));
        h.insert("use",                 (true,  rsvg_node_use_new as NodeCreateFn));
        /* h.insert("view",             (false,  as NodeCreateFn)); */
        /* h.insert("vkern",            (false,  as NodeCreateFn)); */
        h
    };
}

#[no_mangle]
pub extern "C" fn rsvg_load_new_node(
    _name: *const libc::c_char,
    parent: *const RsvgNode,
    id: *const libc::c_char,
    supports_class_attribute: *mut glib_sys::gboolean,
) -> *const RsvgNode {
    assert!(!_name.is_null());
    assert!(!supports_class_attribute.is_null());

    let name = unsafe { utf8_cstr(_name) };
    let creator = match NODE_CREATORS.get(name) {
        Some(c) => c,
        // Whenever we encounter a node we don't understand, represent it as a defs.
        // This is like a group, but it doesn't do any rendering of children.  The
        // effect is that we will ignore all children of unknown elements.
        //
        None => &(true, rsvg_node_defs_new as NodeCreateFn),
    };

    unsafe {
        *supports_class_attribute = creator.0.to_glib();
        creator.1(_name, parent, id)
    }
}
