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
    // (attribute name, Rust enum value)
    //
    // Keep this in sync with rsvg-attributes.h
    #[cfg_attr(rustfmt, rustfmt_skip)]
    let attribute_defs = [
        ( "alternate",          "Alternate" ),
        ( "amplitude",          "Amplitude" ),
        ( "azimuth",            "Azimuth" ),
        ( "baseFrequency",      "BaseFrequency" ),
        ( "baseline-shift",     "BaselineShift" ),
        ( "bias",               "Bias" ),
        ( "class",              "Class" ),
        ( "clip-path",          "ClipPath" ),
        ( "clip-rule",          "ClipRule" ),
        ( "clipPathUnits",      "ClipPathUnits" ),
        ( "color",              "Color" ),
        ( "color-interpolation-filters", "ColorInterpolationFilters" ),
        ( "comp-op",            "CompOp" ),
        ( "cx",                 "Cx" ),
        ( "cy",                 "Cy" ),
        ( "d",                  "D" ),
        ( "diffuseConstant",    "DiffuseConstant" ),
        ( "direction",          "Direction" ),
        ( "display",            "Display" ),
        ( "divisor",            "Divisor" ),
        ( "dx",                 "Dx" ),
        ( "dy",                 "Dy" ),
        ( "edgeMode",           "EdgeMode" ),
        ( "elevation",          "Elevation" ),
        ( "enable-background",  "EnableBackground" ),
        ( "encoding",           "Encoding" ),
        ( "exponent",           "Exponent" ),
        ( "fill",               "Fill" ),
        ( "fill-opacity",       "FillOpacity" ),
        ( "fill-rule",          "FillRule" ),
        ( "filter",             "Filter" ),
        ( "filterUnits",        "FilterUnits" ),
        ( "flood-color",        "FloodColor" ),
        ( "flood-opacity",      "FloodOpacity" ),
        ( "font-family",        "FontFamily" ),
        ( "font-size",          "FontSize" ),
        ( "font-stretch",       "FontStretch" ),
        ( "font-style",         "FontStyle" ),
        ( "font-variant",       "FontVariant" ),
        ( "font-weight",        "FontWeight" ),
        ( "fx",                 "Fx" ),
        ( "fy",                 "Fy" ),
        ( "gradientTransform",  "GradientTransform" ),
        ( "gradientUnits",      "GradientUnits" ),
        ( "height",             "Height" ),
        ( "href",               "Href" ),
        ( "id",                 "Id" ),
        ( "in",                 "In" ),
        ( "in2",                "In2" ),
        ( "intercept",          "Intercept" ),
        ( "k1",                 "K1" ),
        ( "k2",                 "K2" ),
        ( "k3",                 "K3" ),
        ( "k4",                 "K4" ),
        ( "kernelMatrix",       "KernelMatrix" ),
        ( "kernelUnitLength",   "KernelUnitLength" ),
        ( "letter-spacing",     "LetterSpacing" ),
        ( "lighting-color",     "LightingColor" ),
        ( "limitingConeAngle",  "LimitingConeAngle" ),
        ( "marker",             "Marker" ),
        ( "marker-end",         "MarkerEnd" ),
        ( "marker-mid",         "MarkerMid" ),
        ( "marker-start",       "MarkerStart" ),
        ( "markerHeight",       "MarkerHeight" ),
        ( "markerUnits",        "MarkerUnits" ),
        ( "markerWidth",        "MarkerWidth" ),
        ( "mask",               "Mask" ),
        ( "maskContentUnits",   "MaskContentUnits" ),
        ( "maskUnits",          "MaskUnits" ),
        ( "mode",               "Mode" ),
        ( "numOctaves",         "NumOctaves" ),
        ( "offset",             "Offset" ),
        ( "opacity",            "Opacity" ),
        ( "operator",           "Operator" ),
        ( "order",              "Order" ),
        ( "orient",             "Orient" ),
        ( "overflow",           "Overflow" ),
        ( "parse",              "Parse" ),
        ( "path",               "Path" ),
        ( "patternContentUnits", "PatternContentUnits" ),
        ( "patternTransform",   "PatternTransform" ),
        ( "patternUnits",       "PatternUnits" ),
        ( "points",             "Points" ),
        ( "pointsAtX",          "PointsAtX" ),
        ( "pointsAtY",          "PointsAtY" ),
        ( "pointsAtZ",          "PointsAtZ" ),
        ( "preserveAlpha",      "PreserveAlpha" ),
        ( "preserveAspectRatio", "PreserveAspectRatio" ),
        ( "primitiveUnits",     "PrimitiveUnits" ),
        ( "r",                  "R" ),
        ( "radius",             "Radius" ),
        ( "refX",               "RefX" ),
        ( "refY",               "RefY" ),
        ( "requiredExtensions", "RequiredExtensions" ),
        ( "requiredFeatures",   "RequiredFeatures" ),
        ( "result",             "Result" ),
        ( "rx",                 "Rx" ),
        ( "ry",                 "Ry" ),
        ( "scale",              "Scale" ),
        ( "seed",               "Seed" ),
        ( "shape-rendering",    "ShapeRendering" ),
        ( "slope",              "Slope" ),
        ( "specularConstant",   "SpecularConstant" ),
        ( "specularExponent",   "SpecularExponent" ),
        ( "spreadMethod",       "SpreadMethod" ),
        ( "stdDeviation",       "StdDeviation" ),
        ( "stitchTiles",        "StitchTiles" ),
        ( "stop-color",         "StopColor" ),
        ( "stop-opacity",       "StopOpacity" ),
        ( "stroke",             "Stroke" ),
        ( "stroke-dasharray",   "StrokeDasharray" ),
        ( "stroke-dashoffset",  "StrokeDashoffset" ),
        ( "stroke-linecap",     "StrokeLinecap" ),
        ( "stroke-linejoin",    "StrokeLinejoin" ),
        ( "stroke-miterlimit",  "StrokeMiterlimit" ),
        ( "stroke-opacity",     "StrokeOpacity" ),
        ( "stroke-width",       "StrokeWidth" ),
        ( "style",              "Style" ),
        ( "surfaceScale",       "SurfaceScale" ),
        ( "systemLanguage",     "SystemLanguage" ),
        ( "tableValues",        "TableValues" ),
        ( "targetX",            "TargetX" ),
        ( "targetY",            "TargetY" ),
        ( "text-anchor",        "TextAnchor" ),
        ( "text-decoration",    "TextDecoration" ),
        ( "text-rendering",     "TextRendering" ),
        ( "transform",          "Transform" ),
        ( "type",               "Type" ),
        ( "unicode-bidi",       "UnicodeBidi" ),
        ( "values",             "Values" ),
        ( "verts",              "Verts" ),
        ( "viewBox",            "ViewBox" ),
        ( "visibility",         "Visibility" ),
        ( "width",              "Width" ),
        ( "writing-mode",       "WritingMode" ),
        ( "x",                  "X" ),
        ( "x1",                 "X1" ),
        ( "y1",                 "Y1" ),
        ( "x2",                 "X2" ),
        ( "y2",                 "Y2" ),
        ( "xChannelSelector",   "XChannelSelector" ),
        ( "xlink:href",         "XlinkHref" ),
        ( "xml:lang",           "XmlLang" ),
        ( "xml:space",          "XmlSpace" ),
        ( "y",                  "Y" ),
        ( "yChannelSelector",   "YChannelSelector" ),
        ( "z",                  "Z" ),
    ];

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("attributes-codegen.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    writeln!(&mut file, "#[repr(C)]").unwrap();
    writeln!(
        &mut file,
        "#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]"
    )
    .unwrap();
    writeln!(&mut file, "pub enum Attribute {{").unwrap();

    for &(_, valname) in attribute_defs.iter() {
        writeln!(&mut file, "    {},", valname).unwrap();
    }

    writeln!(&mut file, "}}").unwrap();

    writeln!(
        &mut file,
        "static ATTRIBUTES: phf::Map<&'static str, Attribute> = "
    )
    .unwrap();

    let mut map = phf_codegen::Map::new();
    map.phf_path("phf");
    for &(name, valname) in attribute_defs.iter() {
        let valname = ["Attribute::", valname].concat();
        map.entry(name, &valname);
    }

    map.build(&mut file).unwrap();
    writeln!(&mut file, ";").unwrap();

    output_srgb_tables();
}

/// Converts an sRGB color value to a linear sRGB color value (undoes the gamma correction).
///
/// The input and the output are supposed to be in the [0, 1] range.
#[inline]
fn linearize(c: f64) -> f64 {
    if c <= (12.92 * 0.0031308) {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts a linear sRGB color value to a normal sRGB color value (applies the gamma correction).
///
/// The input and the output are supposed to be in the [0, 1] range.
#[inline]
fn unlinearize(c: f64) -> f64 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1f64 / 2.4) - 0.055
    }
}

fn compute_table<F: Fn(f64) -> f64>(f: F) -> [u8; 256] {
    let mut table = [0; 256];

    for i in 0..=255 {
        let c = i as f64 / 255.0;
        let x = f(c);
        table[i] = (x * 255.0).round() as u8;
    }

    table
}

fn print_table<W: Write>(w: &mut W, name: &str, table: &[u8]) {
    writeln!(w, "const {}: [u8; {}] = [", name, table.len()).unwrap();

    for x in table {
        writeln!(w, "    {},", x).unwrap();
    }

    writeln!(w, "];").unwrap();
}

fn output_srgb_tables() {
    let linearize_table = compute_table(linearize);
    let unlinearize_table = compute_table(unlinearize);

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("srgb-codegen.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    print_table(&mut file, "LINEARIZE", &linearize_table);
    print_table(&mut file, "UNLINEARIZE", &unlinearize_table);
}
