use lazy_static::lazy_static;
use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};
use std::collections::HashMap;

use crate::clip_path::ClipPath;
use crate::filters::{
    blend::FeBlend,
    color_matrix::FeColorMatrix,
    component_transfer::{FeComponentTransfer, FeFuncA, FeFuncB, FeFuncG, FeFuncR},
    composite::FeComposite,
    convolve_matrix::FeConvolveMatrix,
    displacement_map::FeDisplacementMap,
    flood::FeFlood,
    gaussian_blur::FeGaussianBlur,
    image::FeImage,
    light::{
        light_source::FeDistantLight, light_source::FePointLight, light_source::FeSpotLight,
        lighting::FeDiffuseLighting, lighting::FeSpecularLighting,
    },
    merge::{FeMerge, FeMergeNode},
    morphology::FeMorphology,
    node::Filter,
    offset::FeOffset,
    tile::FeTile,
    turbulence::FeTurbulence,
};

use crate::gradient::{LinearGradient, RadialGradient, Stop};
use crate::image::Image;
use crate::link::Link;
use crate::marker::Marker;
use crate::mask::Mask;
use crate::node::*;
use crate::pattern::Pattern;
use crate::property_bag::PropertyBag;
use crate::shapes::{Circle, Ellipse, Line, Path, Polygon, Polyline, Rect};
use crate::structure::{Group, NonRendering, Svg, Switch, Symbol, Use};
use crate::style::Style;
use crate::text::{TRef, TSpan, Text};

macro_rules! n {
    ($name:ident, $node_type:ident, $node_trait:ty) => {
        pub fn $name(element_name: &QualName, id: Option<&str>, class: Option<&str>) -> RsvgNode {
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

    n!(create_circle,                    Circle,                     Circle);
    n!(create_clip_path,                 ClipPath,                   ClipPath);
    n!(create_defs,                      Defs,                       NonRendering);
    n!(create_ellipse,                   Ellipse,                    Ellipse);
    n!(create_fe_blend,                  FeBlend,                    FeBlend);
    n!(create_fe_color_matrix,           FeColorMatrix,              FeColorMatrix);
    n!(create_fe_component_transfer,     FeComponentTransfer,        FeComponentTransfer);
    n!(create_fe_func_a,                 FeFuncA,                    FeFuncA);
    n!(create_fe_func_b,                 FeFuncB,                    FeFuncB);
    n!(create_fe_func_g,                 FeFuncG,                    FeFuncG);
    n!(create_fe_func_r,                 FeFuncR,                    FeFuncR);
    n!(create_fe_composite,              FeComposite,                FeComposite);
    n!(create_fe_convolve_matrix,        FeConvolveMatrix,           FeConvolveMatrix);
    n!(create_fe_diffuse_lighting,       FeDiffuseLighting,          FeDiffuseLighting);
    n!(create_fe_distant_light,          FeDistantLight,             FeDistantLight);
    n!(create_fe_displacement_map,       FeDisplacementMap,          FeDisplacementMap);
    n!(create_fe_flood,                  FeFlood,                    FeFlood);
    n!(create_fe_gaussian_blur,          FeGaussianBlur,             FeGaussianBlur);
    n!(create_fe_image,                  FeImage,                    FeImage);
    n!(create_fe_merge,                  FeMerge,                    FeMerge);
    n!(create_fe_merge_node,             FeMergeNode,                FeMergeNode);
    n!(create_fe_morphology,             FeMorphology,               FeMorphology);
    n!(create_fe_offset,                 FeOffset,                   FeOffset);
    n!(create_fe_point_light,            FePointLight,               FePointLight);
    n!(create_fe_specular_lighting,      FeSpecularLighting,         FeSpecularLighting);
    n!(create_fe_spot_light,             FeSpotLight,                FeSpotLight);
    n!(create_fe_tile,                   FeTile,                     FeTile);
    n!(create_fe_turbulence,             FeTurbulence,               FeTurbulence);
    n!(create_filter,                    Filter,                     Filter);
    n!(create_group,                     Group,                      Group);
    n!(create_image,                     Image,                      Image);
    n!(create_line,                      Line,                       Line);
    n!(create_linear_gradient,           LinearGradient,             LinearGradient);
    n!(create_link,                      Link,                       Link);
    n!(create_marker,                    Marker,                     Marker);
    n!(create_mask,                      Mask,                       Mask);
    n!(create_non_rendering,             NonRendering,               NonRendering);
    n!(create_path,                      Path,                       Path);
    n!(create_pattern,                   Pattern,                    Pattern);
    n!(create_polygon,                   Polygon,                    Polygon);
    n!(create_polyline,                  Polyline,                   Polyline);
    n!(create_radial_gradient,           RadialGradient,             RadialGradient);
    n!(create_rect,                      Rect,                       Rect);
    n!(create_stop,                      Stop,                       Stop);
    n!(create_style,                     Style,                      Style);
    n!(create_svg,                       Svg,                        Svg);
    n!(create_switch,                    Switch,                     Switch);
    n!(create_symbol,                    Symbol,                     Symbol);
    n!(create_text,                      Text,                       Text);
    n!(create_tref,                      TRef,                       TRef);
    n!(create_tspan,                     TSpan,                      TSpan);
    n!(create_use,                       Use,                        Use);

    /* Hack to make multiImage sort-of work
     *
     * disabled for now, as markup5ever doesn't have local names for
     * multiImage, subImage, subImageRef.  Maybe we can just... create them ourselves?
     *
     * Is multiImage even in SVG2?
     */
    /*
    n!(create_multi_image,               Switch,                     Switch);
    n!(create_sub_image,                 Group,                      Group);
    n!(create_sub_image_ref,             Image,                      Image);
    */
}

use creators::*;

type NodeCreateFn = fn(element_name: &QualName, id: Option<&str>, class: Option<&str>) -> RsvgNode;

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
        c!(h, "feBlend",             true,  create_fe_blend);
        c!(h, "feColorMatrix",       true,  create_fe_color_matrix);
        c!(h, "feComponentTransfer", true,  create_fe_component_transfer);
        c!(h, "feComposite",         true,  create_fe_composite);
        c!(h, "feConvolveMatrix",    true,  create_fe_convolve_matrix);
        c!(h, "feDiffuseLighting",   true,  create_fe_diffuse_lighting);
        c!(h, "feDisplacementMap",   true,  create_fe_displacement_map);
        c!(h, "feDistantLight",      false, create_fe_distant_light);
        c!(h, "feFuncA",             false, create_fe_func_a);
        c!(h, "feFuncB",             false, create_fe_func_b);
        c!(h, "feFuncG",             false, create_fe_func_g);
        c!(h, "feFuncR",             false, create_fe_func_r);
        c!(h, "feFlood",             true,  create_fe_flood);
        c!(h, "feGaussianBlur",      true,  create_fe_gaussian_blur);
        c!(h, "feImage",             true,  create_fe_image);
        c!(h, "feMerge",             true,  create_fe_merge);
        c!(h, "feMergeNode",         false, create_fe_merge_node);
        c!(h, "feMorphology",        true,  create_fe_morphology);
        c!(h, "feOffset",            true,  create_fe_offset);
        c!(h, "fePointLight",        false, create_fe_point_light);
        c!(h, "feSpecularLighting",  true,  create_fe_specular_lighting);
        c!(h, "feSpotLight",         false, create_fe_spot_light);
        c!(h, "feTile",              true,  create_fe_tile);
        c!(h, "feTurbulence",        true,  create_fe_turbulence);
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
        /* c!(h, "multiImage",          false, create_multi_image); */
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
        /* c!(h, "subImage",            false, create_sub_image); */
        /* c!(h, "subImageRef",         false, create_sub_image_ref); */
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

pub fn create_node(name: &QualName, pbag: &PropertyBag) -> RsvgNode {
    let mut id = None;
    let mut class = None;

    for (attr, value) in pbag.iter() {
        match attr.expanded() {
            expanded_name!("", "id") => id = Some(value),
            expanded_name!(svg "class") => class = Some(value),
            _ => (),
        }
    }

    let (supports_class, create_fn) = if name.ns == ns!(svg) {
        match NODE_CREATORS.get(name.local.as_ref()) {
            // hack in the SVG namespace for supported element names
            Some(&(supports_class, create_fn)) => (supports_class, create_fn),

            // Whenever we encounter a node we don't understand, represent it as a
            // non-rendering node.  This is like a group, but it doesn't do any rendering of
            // children.  The effect is that we will ignore all children of unknown elements.
            None => (true, create_non_rendering as NodeCreateFn),
        }
    } else {
        (true, create_non_rendering as NodeCreateFn)
    };

    if !supports_class {
        class = None;
    };

    create_fn(name, id, class)
}
