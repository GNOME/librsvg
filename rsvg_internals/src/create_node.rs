use lazy_static::lazy_static;
use markup5ever::{local_name, LocalName};
use std::collections::HashMap;

use crate::clip_path::NodeClipPath;
use crate::filters::{
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

use crate::gradient::{NodeGradient, NodeStop};
use crate::image::NodeImage;
use crate::link::NodeLink;
use crate::marker::NodeMarker;
use crate::mask::NodeMask;
use crate::node::*;
use crate::pattern::NodePattern;
use crate::property_bag::PropertyBag;
use crate::shapes::{NodeCircle, NodeEllipse, NodeLine, NodePath, NodePoly, NodeRect};
use crate::structure::{NodeGroup, NodeNonRendering, NodeSvg, NodeSwitch, NodeSymbol, NodeUse};
use crate::style::NodeStyle;
use crate::text::{NodeTRef, NodeTSpan, NodeText};

macro_rules! n {
    ($name:ident, $node_type:ident, $new_fn:expr) => {
        pub fn $name(
            element_name: LocalName,
            id: Option<&str>,
            class: Option<&str>,
            parent: Option<&RsvgNode>,
        ) -> RsvgNode {
            RsvgNode::new(
                NodeData::new(
                    NodeType::$node_type,
                    element_name,
                    id,
                    class,
                    Box::new($new_fn()),
                ),
                parent,
            )
        }
    };
}

#[cfg_attr(rustfmt, rustfmt_skip)]
mod creators {
    use super::*;

    n!(create_circle,                    Circle,                     NodeCircle::default);
    n!(create_clip_path,                 ClipPath,                   NodeClipPath::default);
    n!(create_blend,                     FeBlend,                    Blend::default);
    n!(create_color_matrix,              FeColorMatrix,              ColorMatrix::default);
    n!(create_component_transfer,        FeComponentTransfer,        ComponentTransfer::default);
    n!(create_component_transfer_func_a, ComponentTransferFunctionA, FuncX::new_a);
    n!(create_component_transfer_func_b, ComponentTransferFunctionB, FuncX::new_b);
    n!(create_component_transfer_func_g, ComponentTransferFunctionG, FuncX::new_g);
    n!(create_component_transfer_func_r, ComponentTransferFunctionR, FuncX::new_r);
    n!(create_composite,                 FeComposite,                Composite::default);
    n!(create_convolve_matrix,           FeConvolveMatrix,           ConvolveMatrix::default);
    n!(create_defs,                      Defs,                       NodeNonRendering::default);
    n!(create_diffuse_lighting,          FeDiffuseLighting,          Lighting::new_diffuse);
    n!(create_distant_light,             DistantLight,               LightSource::new_distant_light);
    n!(create_displacement_map,          FeDisplacementMap,          DisplacementMap::default);
    n!(create_ellipse,                   Ellipse,                    NodeEllipse::default);
    n!(create_filter,                    Filter,                     NodeFilter::default);
    n!(create_flood,                     FeFlood,                    Flood::default);
    n!(create_gaussian_blur,             FeGaussianBlur,             GaussianBlur::default);
    n!(create_group,                     Group,                      NodeGroup::default);
    n!(create_image,                     Image,                      NodeImage::default);
    n!(create_fe_image,                  FeImage,                    Image::default);
    n!(create_linear_gradient,           LinearGradient,             NodeGradient::new_linear);
    n!(create_line,                      Line,                       NodeLine::default);
    n!(create_link,                      Link,                       NodeLink::default);
    n!(create_marker,                    Marker,                     NodeMarker::default);
    n!(create_mask,                      Mask,                       NodeMask::default);
    n!(create_merge,                     FeMerge,                    Merge::default);
    n!(create_merge_node,                FeMergeNode,                MergeNode::default);
    n!(create_morphology,                FeMorphology,               Morphology::default);
    n!(create_non_rendering,             NonRendering,               NodeNonRendering::default);
    n!(create_offset,                    FeOffset,                   Offset::default);
    n!(create_path,                      Path,                       NodePath::default);
    n!(create_pattern,                   Pattern,                    NodePattern::default);
    n!(create_point_light,               PointLight,                 LightSource::new_point_light);
    n!(create_polygon,                   Polygon,                    NodePoly::new_closed);
    n!(create_polyline,                  Polyline,                   NodePoly::new_open);
    n!(create_radial_gradient,           RadialGradient,             NodeGradient::new_radial);
    n!(create_rect,                      Rect,                       NodeRect::default);
    n!(create_specular_lighting,         FeSpecularLighting,         Lighting::new_specular);
    n!(create_spot_light,                SpotLight,                  LightSource::new_spot_light);
    n!(create_stop,                      Stop,                       NodeStop::default);
    n!(create_style,                     Style,                      NodeStyle::default);
    n!(create_svg,                       Svg,                        NodeSvg::default);
    n!(create_switch,                    Switch,                     NodeSwitch::default);
    n!(create_symbol,                    Symbol,                     NodeSymbol::default);
    n!(create_text,                      Text,                       NodeText::default);
    n!(create_tref,                      TRef,                       NodeTRef::default);
    n!(create_tspan,                     TSpan,                      NodeTSpan::default);
    n!(create_tile,                      FeTile,                     Tile::default);
    n!(create_turbulence,                FeTurbulence,               Turbulence::default);
    n!(create_use,                       Use,                        NodeUse::default);

    // hack to partially support conical gradient
    n!(create_conical_gradient,          RadialGradient,             NodeGradient::new_radial);

    // hack to make multiImage sort-of work
    n!(create_multi_image,               Switch,                     NodeSwitch::default);
    n!(create_sub_image,                 Group,                      NodeGroup::default);
    n!(create_sub_image_ref,             Image,                      NodeImage::default);
}

use creators::*;

type NodeCreateFn = fn(
    element_name: LocalName,
    id: Option<&str>,
    class: Option<&str>,
    parent: Option<&RsvgNode>,
) -> RsvgNode;

macro_rules! c {
    ($hashset:expr, $str_name:expr, $supports_class:expr, $fn_name:ident) => {
        $hashset.insert($str_name, ($supports_class, $fn_name as NodeCreateFn));
    }
}

lazy_static! {
    // Lines in comments are elements that we don't support.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    static ref NODE_CREATORS: HashMap<&'static str, (bool, NodeCreateFn)> = {
        let mut h = HashMap::new();
        // name, supports_class, create_fn
        c!(h, "a",                   true,  create_link);
        /* c!(h, "altGlyph",         true,  ); */
        /* c!(h, "altGlyphDef",      false, ); */
        /* c!(h, "altGlyphItem",     false, ); */
        /* c!(h, "animate",          false, ); */
        /* c!(h, "animateColor",     false, ); */
        /* c!(h, "animateMotion",    false, ); */
        /* c!(h, "animateTransform", false, ); */
        c!(h, "circle",              true,  create_circle);
        c!(h, "clipPath",            true,  create_clip_path);
        /* c!(h, "color-profile",    false, ); */
        c!(h, "conicalGradient",     true,  create_conical_gradient);
        /* c!(h, "cursor",           false, ); */
        c!(h, "defs",                true,  create_defs);
        /* c!(h, "desc",             true,  ); */
        c!(h, "ellipse",             true,  create_ellipse);
        c!(h, "feBlend",             true,  create_blend);
        c!(h, "feColorMatrix",       true,  create_color_matrix);
        c!(h, "feComponentTransfer", true,  create_component_transfer);
        c!(h, "feComposite",         true,  create_composite);
        c!(h, "feConvolveMatrix",    true,  create_convolve_matrix);
        c!(h, "feDiffuseLighting",   true,  create_diffuse_lighting);
        c!(h, "feDisplacementMap",   true,  create_displacement_map);
        c!(h, "feDistantLight",      false, create_distant_light);
        c!(h, "feFuncA",             false, create_component_transfer_func_a);
        c!(h, "feFuncB",             false, create_component_transfer_func_b);
        c!(h, "feFuncG",             false, create_component_transfer_func_g);
        c!(h, "feFuncR",             false, create_component_transfer_func_r);
        c!(h, "feFlood",             true,  create_flood);
        c!(h, "feGaussianBlur",      true,  create_gaussian_blur);
        c!(h, "feImage",             true,  create_fe_image);
        c!(h, "feMerge",             true,  create_merge);
        c!(h, "feMergeNode",         false, create_merge_node);
        c!(h, "feMorphology",        true,  create_morphology);
        c!(h, "feOffset",            true,  create_offset);
        c!(h, "fePointLight",        false, create_point_light);
        c!(h, "feSpecularLighting",  true,  create_specular_lighting);
        c!(h, "feSpotLight",         false, create_spot_light);
        c!(h, "feTile",              true,  create_tile);
        c!(h, "feTurbulence",        true,  create_turbulence);
        c!(h, "filter",              true,  create_filter);
        /* c!(h, "font",             true,  ); */
        /* c!(h, "font-face",        false, ); */
        /* c!(h, "font-face-format", false, ); */
        /* c!(h, "font-face-name",   false, ); */
        /* c!(h, "font-face-src",    false, ); */
        /* c!(h, "font-face-uri",    false, ); */
        /* c!(h, "foreignObject",    true,  ); */
        c!(h, "g",                   true,  create_group);
        /* c!(h, "glyph",            true,  ); */
        /* c!(h, "glyphRef",         true,  ); */
        /* c!(h, "hkern",            false, ); */
        c!(h, "image",               true,  create_image);
        c!(h, "line",                true,  create_line);
        c!(h, "linearGradient",      true,  create_linear_gradient);
        c!(h, "marker",              true,  create_marker);
        c!(h, "mask",                true,  create_mask);
        /* c!(h, "metadata",         false, ); */
        /* c!(h, "missing-glyph",    true,  ); */
        /* c!(h, "mpath",            false, ); */
        c!(h, "multiImage",          false, create_multi_image);
        c!(h, "path",                true,  create_path);
        c!(h, "pattern",             true,  create_pattern);
        c!(h, "polygon",             true,  create_polygon);
        c!(h, "polyline",            true,  create_polyline);
        c!(h, "radialGradient",      true,  create_radial_gradient);
        c!(h, "rect",                true,  create_rect);
        /* c!(h, "script",           false, ); */
        /* c!(h, "set",              false, ); */
        c!(h, "stop",                true,  create_stop);
        c!(h, "style",               false, create_style);
        c!(h, "subImage",            false, create_sub_image);
        c!(h, "subImageRef",         false, create_sub_image_ref);
        c!(h, "svg",                 true,  create_svg);
        c!(h, "switch",              true,  create_switch);
        c!(h, "symbol",              true,  create_symbol);
        c!(h, "text",                true,  create_text);
        /* c!(h, "textPath",         true,  ); */
        /* c!(h, "title",            true,  ); */
        c!(h, "tref",                true,  create_tref);
        c!(h, "tspan",               true,  create_tspan);
        c!(h, "use",                 true,  create_use);
        /* c!(h, "view",             false, ); */
        /* c!(h, "vkern",            false, ); */
        h
    };
}

pub fn create_node_and_register_id(
    name: &str,
    parent: Option<&RsvgNode>,
    pbag: &PropertyBag,
    ids: &mut HashMap<String, RsvgNode>,
) -> RsvgNode {
    let mut id = None;
    let mut class = None;

    for (attr, value) in pbag.iter() {
        match attr {
            local_name!("id") => id = Some(value),
            local_name!("class") => class = Some(value),
            _ => (),
        }
    }

    let &(supports_class, create_fn) = match NODE_CREATORS.get(name) {
        Some(c) => c,

        // Whenever we encounter a node we don't understand, represent it as a
        // non-rendering node.  This is like a group, but it doesn't do any rendering of
        // children.  The effect is that we will ignore all children of unknown elements.
        None => &(true, create_non_rendering as NodeCreateFn),
    };

    let element_name = LocalName::from(name);

    if !supports_class {
        class = None;
    };

    let node = create_fn(element_name, id, class, parent);

    if let Some(id) = id {
        // This is so we don't overwrite an existing id
        ids.entry(id.to_string()).or_insert_with(|| node.clone());
    }

    node
}
