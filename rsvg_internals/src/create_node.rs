//! Creates tree nodes based on SVG element names.
//!
//! The main [`create_node`] function takes an XML element name, and
//! creates an [`RsvgNode`] for it.
//!
//! [`create_node`]: fn.create_node.html
//! [`RsvgNode`]: ../node/type.RsvgNode.html

use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};
use once_cell::sync::Lazy;
use std::collections::HashMap;

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
        light_source::FeDistantLight,
        light_source::FePointLight,
        light_source::FeSpotLight,
        lighting::FeDiffuseLighting,
        lighting::FeSpecularLighting,
    },
    merge::{FeMerge, FeMergeNode},
    morphology::FeMorphology,
    offset::FeOffset,
    tile::FeTile,
    turbulence::FeTurbulence,
};

use crate::filter::Filter;
use crate::gradient::{LinearGradient, RadialGradient, Stop};
use crate::image::Image;
use crate::link::Link;
use crate::marker::Marker;
use crate::node::*;
use crate::pattern::Pattern;
use crate::property_bag::PropertyBag;
use crate::shapes::{Circle, Ellipse, Line, Path, Polygon, Polyline, Rect};
use crate::structure::{ClipPath, Group, Mask, NonRendering, Svg, Switch, Symbol, Use};
use crate::style::Style;
use crate::text::{TRef, TSpan, Text};

macro_rules! n {
    ($name:ident, $node_type:ident) => {
        pub fn $name(element_name: &QualName, id: Option<&str>, class: Option<&str>) -> RsvgNode {
            RsvgNode::new(NodeData::new(
                NodeType::$node_type,
                element_name,
                id,
                class,
                Box::new(<$node_type>::default()),
            ))
        }
    };
}

#[cfg_attr(rustfmt, rustfmt_skip)]
mod creators {
    use super::*;

    n!(create_circle,                   Circle);
    n!(create_clip_path,                ClipPath);
    n!(create_defs,                     NonRendering);
    n!(create_ellipse,                  Ellipse);
    n!(create_fe_blend,                 FeBlend);
    n!(create_fe_color_matrix,          FeColorMatrix);
    n!(create_fe_component_transfer,    FeComponentTransfer);
    n!(create_fe_func_a,                FeFuncA);
    n!(create_fe_func_b,                FeFuncB);
    n!(create_fe_func_g,                FeFuncG);
    n!(create_fe_func_r,                FeFuncR);
    n!(create_fe_composite,             FeComposite);
    n!(create_fe_convolve_matrix,       FeConvolveMatrix);
    n!(create_fe_diffuse_lighting,      FeDiffuseLighting);
    n!(create_fe_distant_light,         FeDistantLight);
    n!(create_fe_displacement_map,      FeDisplacementMap);
    n!(create_fe_flood,                 FeFlood);
    n!(create_fe_gaussian_blur,         FeGaussianBlur);
    n!(create_fe_image,                 FeImage);
    n!(create_fe_merge,                 FeMerge);
    n!(create_fe_merge_node,            FeMergeNode);
    n!(create_fe_morphology,            FeMorphology);
    n!(create_fe_offset,                FeOffset);
    n!(create_fe_point_light,           FePointLight);
    n!(create_fe_specular_lighting,     FeSpecularLighting);
    n!(create_fe_spot_light,            FeSpotLight);
    n!(create_fe_tile,                  FeTile);
    n!(create_fe_turbulence,            FeTurbulence);
    n!(create_filter,                   Filter);
    n!(create_group,                    Group);
    n!(create_image,                    Image);
    n!(create_line,                     Line);
    n!(create_linear_gradient,          LinearGradient);
    n!(create_link,                     Link);
    n!(create_marker,                   Marker);
    n!(create_mask,                     Mask);
    n!(create_non_rendering,            NonRendering);
    n!(create_path,                     Path);
    n!(create_pattern,                  Pattern);
    n!(create_polygon,                  Polygon);
    n!(create_polyline,                 Polyline);
    n!(create_radial_gradient,          RadialGradient);
    n!(create_rect,                     Rect);
    n!(create_stop,                     Stop);
    n!(create_style,                    Style);
    n!(create_svg,                      Svg);
    n!(create_switch,                   Switch);
    n!(create_symbol,                   Symbol);
    n!(create_text,                     Text);
    n!(create_tref,                     TRef);
    n!(create_tspan,                    TSpan);
    n!(create_use,                      Use);

    /* Hack to make multiImage sort-of work
     *
     * disabled for now, as markup5ever doesn't have local names for
     * multiImage, subImage, subImageRef.  Maybe we can just... create them ourselves?
     *
     * Is multiImage even in SVG2?
     */
    /*
    n!(create_multi_image,              Switch);
    n!(create_sub_image,                Group);
n!(create_sub_image_ref,                Image);
    */
}

use creators::*;

type NodeCreateFn = fn(element_name: &QualName, id: Option<&str>, class: Option<&str>) -> RsvgNode;

// Lines in comments are elements that we don't support.
#[cfg_attr(rustfmt, rustfmt_skip)]
static NODE_CREATORS: Lazy<HashMap<&'static str, (bool, NodeCreateFn)>> = Lazy::new(|| {
    let creators_table: Vec<(&str, bool, NodeCreateFn)> = vec![
        // name, supports_class, create_fn
        ("a",                   true,  create_link),
        /* ("altGlyph",         true,  ), */
        /* ("altGlyphDef",      false, ), */
        /* ("altGlyphItem",     false, ), */
        /* ("animate",          false, ), */
        /* ("animateColor",     false, ), */
        /* ("animateMotion",    false, ), */
        /* ("animateTransform", false, ), */
        ("circle",              true,  create_circle),
        ("clipPath",            true,  create_clip_path),
        /* ("color-profile",    false, ), */
        /* ("cursor",           false, ), */
        ("defs",                true,  create_defs),
        /* ("desc",             true,  ), */
        ("ellipse",             true,  create_ellipse),
        ("feBlend",             true,  create_fe_blend),
        ("feColorMatrix",       true,  create_fe_color_matrix),
        ("feComponentTransfer", true,  create_fe_component_transfer),
        ("feComposite",         true,  create_fe_composite),
        ("feConvolveMatrix",    true,  create_fe_convolve_matrix),
        ("feDiffuseLighting",   true,  create_fe_diffuse_lighting),
        ("feDisplacementMap",   true,  create_fe_displacement_map),
        ("feDistantLight",      false, create_fe_distant_light),
        ("feFuncA",             false, create_fe_func_a),
        ("feFuncB",             false, create_fe_func_b),
        ("feFuncG",             false, create_fe_func_g),
        ("feFuncR",             false, create_fe_func_r),
        ("feFlood",             true,  create_fe_flood),
        ("feGaussianBlur",      true,  create_fe_gaussian_blur),
        ("feImage",             true,  create_fe_image),
        ("feMerge",             true,  create_fe_merge),
        ("feMergeNode",         false, create_fe_merge_node),
        ("feMorphology",        true,  create_fe_morphology),
        ("feOffset",            true,  create_fe_offset),
        ("fePointLight",        false, create_fe_point_light),
        ("feSpecularLighting",  true,  create_fe_specular_lighting),
        ("feSpotLight",         false, create_fe_spot_light),
        ("feTile",              true,  create_fe_tile),
        ("feTurbulence",        true,  create_fe_turbulence),
        ("filter",              true,  create_filter),
        /* ("font",             true,  ), */
        /* ("font-face",        false, ), */
        /* ("font-face-format", false, ), */
        /* ("font-face-name",   false, ), */
        /* ("font-face-src",    false, ), */
        /* ("font-face-uri",    false, ), */
        /* ("foreignObject",    true,  ), */
        ("g",                   true,  create_group),
        /* ("glyph",            true,  ), */
        /* ("glyphRef",         true,  ), */
        /* ("hkern",            false, ), */
        ("image",               true,  create_image),
        ("line",                true,  create_line),
        ("linearGradient",      true,  create_linear_gradient),
        ("marker",              true,  create_marker),
        ("mask",                true,  create_mask),
        /* ("metadata",         false, ), */
        /* ("missing-glyph",    true,  ), */
        /* ("mpath",            false, ), */
        /* ("multiImage",       false, create_multi_image), */
        ("path",                true,  create_path),
        ("pattern",             true,  create_pattern),
        ("polygon",             true,  create_polygon),
        ("polyline",            true,  create_polyline),
        ("radialGradient",      true,  create_radial_gradient),
        ("rect",                true,  create_rect),
        /* ("script",           false, ), */
        /* ("set",              false, ), */
        ("stop",                true,  create_stop),
        ("style",               false, create_style),
        /* ("subImage",         false, create_sub_image), */
        /* ("subImageRef",      false, create_sub_image_ref), */
        ("svg",                 true,  create_svg),
        ("switch",              true,  create_switch),
        ("symbol",              true,  create_symbol),
        ("text",                true,  create_text),
        /* ("textPath",         true,  ), */
        /* ("title",            true,  ), */
        ("tref",                true,  create_tref),
        ("tspan",               true,  create_tspan),
        ("use",                 true,  create_use),
        /* ("view",             false, ), */
        /* ("vkern",            false, ), */
    ];

    creators_table.into_iter().map(|(n, s, f)| (n, (s, f))).collect()
});

/// Takes an XML element name and a list of attribute/value pairs and creates an [`RsvgNode`].
///
/// This operation does not fail.  Unknown element names simply produce a [`NonRendering`]
/// node.
///
/// [`RsvgNode`]: ../node/type.RsvgNode.html
/// [`NonRendering`]: ../structure/struct.NonRendering.html
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

    //    sizes::print_sizes();

    create_fn(name, id, class)
}

#[cfg(ignore)]
mod sizes {
    //! This module is in this file just because here we have all the imports.

    use super::*;

    macro_rules! print_size {
        ($ty:ty) => {
            println!("sizeof {}: {}", stringify!($ty), mem::size_of::<$ty>());
        };
    }

    pub fn print_sizes() {
        use crate::properties::{ComputedValues, ParsedProperty, SpecifiedValues};
        use std::mem;

        print_size!(NodeData);
        print_size!(SpecifiedValues);
        print_size!(ComputedValues);
        print_size!(ParsedProperty);

        print_size!(Circle);
        print_size!(ClipPath);
        print_size!(NonRendering);
        print_size!(Ellipse);
        print_size!(FeBlend);
        print_size!(FeColorMatrix);
        print_size!(FeComponentTransfer);
        print_size!(FeFuncA);
        print_size!(FeFuncB);
        print_size!(FeFuncG);
        print_size!(FeFuncR);
        print_size!(FeComposite);
        print_size!(FeConvolveMatrix);
        print_size!(FeDiffuseLighting);
        print_size!(FeDistantLight);
        print_size!(FeDisplacementMap);
        print_size!(FeFlood);
        print_size!(FeGaussianBlur);
        print_size!(FeImage);
        print_size!(FeMerge);
        print_size!(FeMergeNode);
        print_size!(FeMorphology);
        print_size!(FeOffset);
        print_size!(FePointLight);
        print_size!(FeSpecularLighting);
        print_size!(FeSpotLight);
        print_size!(FeTile);
        print_size!(FeTurbulence);
        print_size!(Filter);
        print_size!(Group);
        print_size!(Image);
        print_size!(Line);
        print_size!(LinearGradient);
        print_size!(Link);
        print_size!(Marker);
        print_size!(Mask);
        print_size!(NonRendering);
        print_size!(Path);
        print_size!(Pattern);
        print_size!(Polygon);
        print_size!(Polyline);
        print_size!(RadialGradient);
        print_size!(Rect);
        print_size!(Stop);
        print_size!(Style);
        print_size!(Svg);
        print_size!(Switch);
        print_size!(Symbol);
        print_size!(Text);
        print_size!(TRef);
        print_size!(TSpan);
        print_size!(Use);
    }
}
