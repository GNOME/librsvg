extern crate phf_codegen;

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

fn main() {
    generate_phf_of_svg_attributes();
}

/// Creates a perfect hash function (PHF) to map SVG attribute names to enum values.
fn generate_phf_of_svg_attributes() {
    // (attribute name, Rust enum value, C enum value suffix
    let attribute_defs = [
        ( "alternate",          "Alternate",          "ALTERNATE" ),
        ( "amplitude",          "Amplitude",          "AMPLITUDE" ),
        ( "azimuth",            "Azimuth",            "AZIMUTH" ),
        ( "baseFrequency",      "BaseFrequency",      "BASE_FREQUENCY" ),
        ( "baseline-shift",     "BaselineShift",      "BASELINE_SHIFT" ),
        ( "bias",               "Bias",               "BIAS" ),
        ( "class",              "Class",              "CLASS" ),
        ( "clip-path",          "ClipPath",           "CLIP_PATH" ),
        ( "clip-rule",          "ClipRule",           "CLIP_RULE" ),
        ( "color",              "Color",              "COLOR" ),
        ( "comp-op",            "CompOp",             "COMP_OP" ),
        ( "diffuseConstant",    "DiffuseConstant",    "DIFFUSE_CONSTANT" ),
        ( "direction",          "Direction",          "DIRECTION" ),
        ( "display",            "Display",            "DISPLAY" ),
        ( "divisor",            "Divisor",            "DIVISOR" ),
        ( "dx",                 "Dx",                 "DX" ),
        ( "dy",                 "Dy",                 "DY" ),
        ( "edgeMode",           "EdgeMode",           "EDGE_MODE" ),
        ( "elevation",          "Elevation",          "ELEVATION" ),
        ( "enable-background",  "EnableBackground",   "ENABLE_BACKGROUND" ),
        ( "encoding",           "Encoding",           "ENCODING" ),
        ( "exponent",           "Exponent",           "EXPONENT" ),
        ( "fill",               "Fill",               "FILL" ),
        ( "fill-opacity",       "FillOpacity",        "FILL_OPACITY" ),
        ( "fill-rule",          "FillRule",           "FILL_RULE" ),
        ( "filter",             "Filter",             "FILTER" ),
        ( "filterUnits",        "FilterUnits",        "FILTERUNITS" ),
        ( "flood-color",        "FloodColor",         "FLOOD_COLOR" ),
        ( "flood-opacity",      "FloodOpacity",       "FLOOD_OPACITY" ),
        ( "font-family",        "FontFamily",         "FONT_FAMILY" ),
        ( "font-size",          "FontSize",           "FONT_SIZE" ),
        ( "font-stretch",       "FontStretch",        "FONT_STRETCH" ),
        ( "font-style",         "FontStyle",          "FONT_STYLE" ),
        ( "font-variant",       "FontVariant",        "FONT_VARIANT" ),
        ( "font-weight",        "FontWeight",         "FONT_WEIGHT" ),
        ( "height",             "Height",             "HEIGHT" ),
        ( "href",               "Href",               "HREF" ),
        ( "id",                 "Id",                 "ID" ),
        ( "in",                 "In",                 "IN" ),
        ( "in2",                "In2",                "IN2" ),
        ( "intercept",          "Intercept",          "INTERCEPT" ),
        ( "k1",                 "K1",                 "K1" ),
        ( "k2",                 "K2",                 "K2" ),
        ( "k3",                 "K3",                 "K3" ),
        ( "k4",                 "K4",                 "K4" ),
        ( "kernelMatrix",       "KernelMatrix",       "KERNEL_MATRIX" ),
        ( "kernelUnitLength",   "KernelUnitLength",   "KERNEL_UNIT_LENGTH" ),
        ( "letter-spacing",     "LetterSpacing",      "LETTER_SPACING" ),
        ( "lighting-color",     "LightingColor",      "LIGHTING_COLOR" ),
        ( "limitingConeAngle",  "LimitingConeAngle",  "LIMITING_CONE_ANGLE" ),
        ( "marker",             "Marker",             "MARKER" ),
        ( "marker-end",         "MarkerEnd",          "MARKER_END" ),
        ( "marker-mid",         "MarkerMid",          "MARKER_MID" ),
        ( "marker-start",       "MarkerStart",        "MARKER_START" ),
        ( "mask",               "Mask",               "MASK" ),
        ( "mode",               "Mode",               "MODE" ),
        ( "numOctaves",         "NumOctaves",         "NUM_OCTAVES" ),
        ( "offset",             "Offset",             "OFFSET" ),
        ( "opacity",            "Opacity",            "OPACITY" ),
        ( "operator",           "Operator",           "OPERATOR" ),
        ( "order",              "Order",              "ORDER" ),
        ( "overflow",           "Overflow",           "OVERFLOW" ),
        ( "parse",              "Parse",              "PARSE" ),
        ( "pointsAtX",          "PointsAtX",          "POINTS_AT_X" ),
        ( "pointsAtY",          "PointsAtY",          "POINTS_AT_Y" ),
        ( "pointsAtZ",          "PointsAtZ",          "POINTS_AT_Z" ),
        ( "preserveAlpha",      "PreserveAlpha",      "PRESERVE_ALPHA" ),
        ( "primitiveUnits",     "PrimitiveUnits",     "PRIMITIVE_UNITS" ),
        ( "radius",             "Radius",             "RADIUS" ),
        ( "requiredExtensions", "RequiredExtensions", "REQUIRED_EXTENSIONS" ),
        ( "requiredFeatures",   "RequiredFeatures",   "REQUIRED_FEATURES" ),
        ( "result",             "Result",             "RESULT" ),
        ( "scale",              "Scale",              "SCALE" ),
        ( "seed",               "Seed",               "SEED" ),
        ( "shape-rendering",    "ShapeRendering",     "SHAPE_RENDERING" ),
        ( "slope",              "Slope",              "SLOPE" ),
        ( "specularConstant",   "SpecularConstant",   "SPECULAR_CONSTANT" ),
        ( "specularExponent",   "SpecularExponent",   "SPECULAR_EXPONENT" ),
        ( "stdDeviation",       "StdDeviation",       "STD_DEVIATION" ),
        ( "stitchTiles",        "StitchTiles",        "STITCH_TILES" ),
        ( "stop-color",         "StopColor",          "STOP_COLOR" ),
        ( "stop-opacity",       "StopOpacity",        "STOP_OPACITY" ),
        ( "stroke",             "Stroke",             "STROKE" ),
        ( "stroke-dasharray",   "StrokeDasharray",    "STROKE_DASHARRAY" ),
        ( "stroke-dashoffset",  "StrokeDashoffset",   "STROKE_DASHOFFSET" ),
        ( "stroke-linecap",     "StrokeLinecap",      "STROKE_LINECAP" ),
        ( "stroke-linejoin",    "StrokeLinejoin",     "STROKE_LINEJOIN" ),
        ( "stroke-miterlimit",  "StrokeMiterlimit",   "STROKE_MITERLIMIT" ),
        ( "stroke-opacity",     "StrokeOpacity",      "STROKE_OPACITY" ),
        ( "stroke-width",       "StrokeWidth",        "STROKE_WIDTH" ),
        ( "style",              "Style",              "STYLE" ),
        ( "surfaceScale",       "SurfaceScale",       "SURFACE_SCALE" ),
        ( "systemLanguage",     "SystemLanguage",     "SYSTEM_LANGUAGE" ),
        ( "tableValues",        "TableValues",        "TABLE_VALUES" ),
        ( "targetX",            "TargetX",            "TARGET_X" ),
        ( "targetY",            "TargetY",            "TARGET_Y" ),
        ( "text-anchor",        "TextAnchor",         "TEXT_ANCHOR" ),
        ( "text-decoration",    "TextDecoration",     "TEXT_DECORATION" ),
        ( "text-rendering",     "TextRendering",      "TEXT_RENDERING" ),
        ( "transform",          "Transform",          "TRANSFORM" ),
        ( "type",               "Type",               "TYPE" ),
        ( "unicode-bidi",       "UnicodeBidi",        "UNICODE_BIDI" ),
        ( "values",             "Values",             "VALUES" ),
        ( "visibility",         "Visibility",         "VISIBILITY" ),
        ( "width",              "Width",              "WIDTH" ),
        ( "writing-mode",       "WritingMode",        "WRITING_MODE" ),
        ( "x",                  "X",                  "X" ),
        ( "xChannelSelector",   "XChannelSelector",   "X_CHANNEL_SELECTOR" ),
        ( "xlink:href",         "XlinkHref",          "XLINK_HREF" ),
        ( "xml:lang",           "XmlLang",            "XML_LANG" ),
        ( "xml:space",          "XmlSpace",           "XML_SPACE" ),
        ( "y",                  "Y",                  "Y" ),
        ( "yChannelSelector",   "YChannelSelector",   "Y_CHANNEL_SELECTOR" ),
        ( "z",                  "Z",                  "Z" ),
    ];

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("attributes-codegen.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    writeln!(&mut file, "#[repr(C)]").unwrap();
    writeln!(&mut file, "#[derive(Debug, Clone, Copy, PartialEq)]").unwrap();
    writeln!(&mut file, "pub enum Attribute {{").unwrap();

    for &(_, rust, _) in attribute_defs.iter() {
        writeln!(&mut file, "    {},", rust).unwrap();
    }
    
    writeln!(&mut file, "}}").unwrap();

    writeln!(&mut file, "static ATTRIBUTES: phf::Map<&'static str, Attribute> = ").unwrap();

    let mut map = phf_codegen::Map::new();
    map.phf_path("phf");
    for &(name, rust, _) in attribute_defs.iter() {
        let rust = ["Attribute::", rust].concat();
        map.entry(name, &rust);
    }

    map.build(&mut file).unwrap();
    writeln!(&mut file, ";").unwrap();
}
