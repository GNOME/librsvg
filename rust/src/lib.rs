#[macro_use]
extern crate bitflags;

pub use aspect_ratio::{
    FitMode,
    AlignMode,
    Align,
    AspectRatio,
    ParseAspectRatioError,
    rsvg_aspect_ratio_parse,
    rsvg_aspect_ratio_compute
};

pub use bbox::{
    RsvgBbox,
    rsvg_bbox_init,
    rsvg_bbox_insert,
    rsvg_bbox_clip
};

pub use gradient::{
    gradient_linear_new,
    gradient_radial_new,
    gradient_destroy,
    gradient_add_color_stop,
    gradient_resolve_fallbacks_and_set_pattern
};

pub use path_builder::{
    rsvg_path_builder_new,
    rsvg_path_builder_destroy,
    rsvg_path_builder_move_to,
    rsvg_path_builder_line_to,
    rsvg_path_builder_curve_to,
    rsvg_path_builder_close_path,
    rsvg_path_builder_arc,
    rsvg_path_builder_add_to_cairo_context
};

pub use marker::{
    rsvg_render_markers,
};

pub use path_parser::{
    rsvg_path_parser_from_str_into_builder
};

pub use length::{
    LengthUnit,
    LengthDir,
    RsvgLength,
    rsvg_length_parse,
    rsvg_length_normalize,
    rsvg_length_hand_normalize,
};

pub use viewbox::{
    RsvgViewBox
};


mod aspect_ratio;
mod bbox;
mod drawing_ctx;
mod handle;
mod gradient;
mod length;
mod marker;
mod node;
mod path_builder;
mod path_parser;
mod property_bag;
mod state;
mod shapes;
mod strtod;
mod util;
mod viewbox;
