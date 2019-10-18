use lazy_static::lazy_static;
use markup5ever::{local_name, namespace_url, ns, LocalName, Prefix, QualName};
use std::collections::HashMap;

use crate::clip_path::NodeClipPath;
use crate::filters::{
    blend::Blend,
    color_matrix::ColorMatrix,
    component_transfer::{ComponentTransfer, FuncA, FuncB, FuncG, FuncR},
    composite::Composite,
    convolve_matrix::ConvolveMatrix,
    displacement_map::DisplacementMap,
    flood::Flood,
    gaussian_blur::GaussianBlur,
    image::Image,
    light::{
        light_source::DistantLight, light_source::PointLight, light_source::SpotLight,
        lighting::DiffuseLighting, lighting::SpecularLighting,
    },
    merge::{Merge, MergeNode},
    morphology::Morphology,
    node::NodeFilter,
    offset::Offset,
    tile::Tile,
    turbulence::Turbulence,
};

use crate::gradient::{NodeLinearGradient, NodeRadialGradient, NodeStop};
use crate::image::NodeImage;
use crate::link::NodeLink;
use crate::marker::NodeMarker;
use crate::mask::NodeMask;
use crate::node::*;
use crate::pattern::NodePattern;
use crate::property_bag::PropertyBag;
use crate::shapes::{
    NodeCircle, NodeEllipse, NodeLine, NodePath, NodePolygon, NodePolyline, NodeRect,
};
use crate::structure::{NodeGroup, NodeNonRendering, NodeSvg, NodeSwitch, NodeSymbol, NodeUse};
use crate::style::NodeStyle;
use crate::text::{NodeTRef, NodeTSpan, NodeText};

macro_rules! n {
    ($name:ident, $node_type:ident, $node_trait:ty) => {
        pub fn $name(element_name: QualName, id: Option<&str>, class: Option<&str>) -> RsvgNode {
            RsvgNode::new(NodeData::new(
                NodeType::$node_type,
                element_name,
                id,
                class,
                Box::new(<$node_trait>::default()),
            ))
        }
    };
}

#[cfg_attr(rustfmt, rustfmt_skip)]
mod creators {
    use super::*;

    n!(create_circle,                    Circle,                     NodeCircle);
    n!(create_clip_path,                 ClipPath,                   NodeClipPath);
    n!(create_blend,                     FeBlend,                    Blend);
    n!(create_color_matrix,              FeColorMatrix,              ColorMatrix);
    n!(create_component_transfer,        FeComponentTransfer,        ComponentTransfer);
    n!(create_component_transfer_func_a, ComponentTransferFunctionA, FuncA);
    n!(create_component_transfer_func_b, ComponentTransferFunctionB, FuncB);
    n!(create_component_transfer_func_g, ComponentTransferFunctionG, FuncG);
    n!(create_component_transfer_func_r, ComponentTransferFunctionR, FuncR);
    n!(create_composite,                 FeComposite,                Composite);
    n!(create_convolve_matrix,           FeConvolveMatrix,           ConvolveMatrix);
    n!(create_defs,                      Defs,                       NodeNonRendering);
    n!(create_diffuse_lighting,          FeDiffuseLighting,          DiffuseLighting);
    n!(create_distant_light,             FeDistantLight,             DistantLight);
    n!(create_displacement_map,          FeDisplacementMap,          DisplacementMap);
    n!(create_ellipse,                   Ellipse,                    NodeEllipse);
    n!(create_filter,                    Filter,                     NodeFilter);
    n!(create_flood,                     FeFlood,                    Flood);
    n!(create_gaussian_blur,             FeGaussianBlur,             GaussianBlur);
    n!(create_group,                     Group,                      NodeGroup);
    n!(create_image,                     Image,                      NodeImage);
    n!(create_fe_image,                  FeImage,                    Image);
    n!(create_line,                      Line,                       NodeLine);
    n!(create_linear_gradient,           LinearGradient,             NodeLinearGradient);
    n!(create_link,                      Link,                       NodeLink);
    n!(create_marker,                    Marker,                     NodeMarker);
    n!(create_mask,                      Mask,                       NodeMask);
    n!(create_merge,                     FeMerge,                    Merge);
    n!(create_merge_node,                FeMergeNode,                MergeNode);
    n!(create_morphology,                FeMorphology,               Morphology);
    n!(create_non_rendering,             NonRendering,               NodeNonRendering);
    n!(create_offset,                    FeOffset,                   Offset);
    n!(create_path,                      Path,                       NodePath);
    n!(create_pattern,                   Pattern,                    NodePattern);
    n!(create_point_light,               FePointLight,               PointLight);
    n!(create_polygon,                   Polygon,                    NodePolygon);
    n!(create_polyline,                  Polyline,                   NodePolyline);
    n!(create_radial_gradient,           RadialGradient,             NodeRadialGradient);
    n!(create_rect,                      Rect,                       NodeRect);
    n!(create_specular_lighting,         FeSpecularLighting,         SpecularLighting);
    n!(create_spot_light,                FeSpotLight,                SpotLight);
    n!(create_stop,                      Stop,                       NodeStop);
    n!(create_style,                     Style,                      NodeStyle);
    n!(create_svg,                       Svg,                        NodeSvg);
    n!(create_switch,                    Switch,                     NodeSwitch);
    n!(create_symbol,                    Symbol,                     NodeSymbol);
    n!(create_text,                      Text,                       NodeText);
    n!(create_tref,                      TRef,                       NodeTRef);
    n!(create_tspan,                     TSpan,                      NodeTSpan);
    n!(create_tile,                      FeTile,                     Tile);
    n!(create_turbulence,                FeTurbulence,               Turbulence);
    n!(create_use,                       Use,                        NodeUse);

    // hack to make multiImage sort-of work
    n!(create_multi_image,               Switch,                     NodeSwitch);
    n!(create_sub_image,                 Group,                      NodeGroup);
    n!(create_sub_image_ref,             Image,                      NodeImage);
}

use creators::*;

type NodeCreateFn = fn(element_name: QualName, id: Option<&str>, class: Option<&str>) -> RsvgNode;

macro_rules! c {
    ($hashset:expr, $str_name:expr, $supports_class:expr, $fn_name:ident) => {
        $hashset.insert($str_name, ($supports_class, $fn_name as NodeCreateFn));
    };
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

    let (supports_class, create_fn, prefix, namespace) = match NODE_CREATORS.get(name) {
        // hack in the SVG namespace for supported element names
        Some(&(supports_class, create_fn)) => (supports_class, create_fn, Some("svg"), ns!(svg)),

        // Whenever we encounter a node we don't understand, represent it as a
        // non-rendering node.  This is like a group, but it doesn't do any rendering of
        // children.  The effect is that we will ignore all children of unknown elements.
        None => (true, create_non_rendering as NodeCreateFn, None, ns!()),
    };

    let element_name = QualName::new(prefix.map(Prefix::from), namespace, LocalName::from(name));

    if !supports_class {
        class = None;
    };

    let node = create_fn(element_name, id, class);

    if let Some(id) = id {
        // This is so we don't overwrite an existing id
        ids.entry(id.to_string()).or_insert_with(|| node.clone());
    }

    node
}
