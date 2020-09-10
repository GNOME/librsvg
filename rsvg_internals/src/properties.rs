//! CSS properties, specified values, computed values.
//!
//! To implement support for a CSS property, do the following:
//!
//! * Create a type that will hold the property's values.  Please do this in the file
//! `property_defs.rs`; you should cut-and-paste from the existing property definitions or
//! read the documentation of the [`make_property`] macro.  You should read the
//! documentation for the [`property_defs`] module to see all that is involved in creating
//! a type for a property.
//!
//! * Modify the call to the `make_properties` macro in this module to include the new
//! property's name.
//!
//! * Modify the rest of librsvg wherever the computed value of the property needs to be used.
//! This is available in methods that take an argument of type [`ComputedValues`].
//!
//! [`make_property`]: ../macro.make_property.html
//! [`property_defs`]: ../property_defs/index.html
//! [`ComputedValues`]: ../struct.ComputedValues.html

use cssparser::{
    self, BasicParseErrorKind, DeclarationListParser, ParseErrorKind, Parser, ParserInput, ToCss,
};
use markup5ever::{
    expanded_name, local_name, namespace_url, ns, ExpandedName, LocalName, QualName,
};
use std::collections::HashSet;

use crate::attributes::Attributes;
use crate::css::{DeclParser, Declaration, Origin};
use crate::error::*;
use crate::font_props::*;
use crate::parsers::{Parse, ParseValue};
use crate::property_defs::*;
use crate::property_macros::Property;

/// Representation of a single CSS property value.
///
/// `Unspecified` is the `Default`; it means that the corresponding property is not present.
///
/// `Inherit` means that the property is explicitly set to inherit
/// from the parent element.  This is useful for properties which the
/// SVG or CSS specs mandate that should not be inherited by default.
///
/// `Specified` is a value given by the SVG or CSS stylesheet.  This will later be
/// resolved into part of a `ComputedValues` struct.
#[derive(Clone)]
pub enum SpecifiedValue<T>
where
    T: Property<ComputedValues> + Clone + Default,
{
    Unspecified,
    Inherit,
    Specified(T),
}

impl<T> SpecifiedValue<T>
where
    T: Property<ComputedValues> + Clone + Default,
{
    pub fn compute(&self, src: &T, src_values: &ComputedValues) -> T {
        let value: T = match *self {
            SpecifiedValue::Unspecified => {
                if <T as Property<ComputedValues>>::inherits_automatically() {
                    src.clone()
                } else {
                    Default::default()
                }
            }

            SpecifiedValue::Inherit => src.clone(),

            SpecifiedValue::Specified(ref v) => v.clone(),
        };

        value.compute(src_values)
    }
}

impl PropertyId {
    fn as_u8(&self) -> u8 {
        *self as u8
    }

    fn as_usize(&self) -> usize {
        *self as usize
    }
}

/// Holds the specified values for the CSS properties of an element.
#[derive(Clone)]
pub struct SpecifiedValues {
    indices: [u8; PropertyId::UnsetProperty as usize],
    props: Vec<ParsedProperty>,
}

impl Default for SpecifiedValues {
    fn default() -> Self {
        SpecifiedValues {
            // this many elements, with the same value
            indices: [PropertyId::UnsetProperty.as_u8(); PropertyId::UnsetProperty as usize],
            props: Vec::new(),
        }
    }
}

impl ComputedValues {
    pub fn is_overflow(&self) -> bool {
        match self.overflow() {
            Overflow::Auto | Overflow::Visible => true,
            _ => false,
        }
    }

    pub fn is_visible(&self) -> bool {
        match (self.display(), self.visibility()) {
            (Display::None, _) => false,
            (_, Visibility::Visible) => true,
            _ => false,
        }
    }
}

/// Macro to generate all the machinery for properties.
///
/// This generates the following:
///
/// * `PropertyId`, an fieldless enum with simple values to identify all the properties.
/// * `ParsedProperty`, a variant enum for all the specified property values.
/// * `ComputedValue`, a variant enum for all the computed values.
/// * `parse_property`, the main function to parse a property declaration from user input.
///
/// There is a lot of repetitive code, for example, because sometimes
/// we need to operate on `PropertyId::Foo`, `ParsedProperty::Foo` and
/// `ComputedValue::Foo` together.  This is why all this is done with a macro.
///
/// See the only invocation of this macro to see how it is used; it is just
/// a declarative list of property names.
///
/// **NOTE:** If you get a compiler error similar to this:
///
/// ```
/// 362 |         "mix-blend-mode"              => mix_blend_mode              : MixBlendMode,
///     |         ^^^^^^^^^^^^^^^^ no rules expected this token in macro call
/// ```
///
/// Then it may be that you put the name inside the `longhands` block, when it should be
/// inside the `longhands_not_supported_by_markup5ever` block.  This is because the
/// [`markup5ever`] crate does not have predefined names for every single property out
/// there; just the common ones.
///
/// [`markup5ever`]: https://docs.rs/markup5ever
macro_rules! make_properties {
    {
        shorthands: {
            $($short_str:tt => $short_field:ident: $short_name:ident,)*
        }

        longhands: {
            $($long_str:tt => $long_field:ident: $long_name:ident,)+
        }

        // These are for when expanded_name!("" "foo") is not defined yet
        // in markup5ever.  We create an ExpandedName by hand in that case.
        longhands_not_supported_by_markup5ever: {
            $($long_m5e_str:tt => $long_m5e_field:ident: $long_m5e_name:ident,)+
        }

        non_properties: {
            $($nonprop_field:ident: $nonprop_name:ident,)+
        }
    }=> {
        /// Used to match `ParsedProperty` to their discriminant
        ///
        /// The `PropertyId::UnsetProperty` can be used as a sentinel value, as
        /// it does not match any `ParsedProperty` discriminant; it is really the
        /// number of valid values in this enum.
        #[repr(u8)]
        #[derive(Copy, Clone, PartialEq)]
        enum PropertyId {
            $($short_name,)+
            $($long_name,)+
            $($long_m5e_name,)+
            $($nonprop_name,)+

            UnsetProperty,
        }

        impl PropertyId {
            fn is_shorthand(self) -> bool {
                match self {
                    $(PropertyId::$short_name => true,)+
                    _ => false,
                }
            }
        }

        /// Embodies "which property is this" plus the property's value
        #[derive(Clone)]
        pub enum ParsedProperty {
            // we put all the properties here; these are for SpecifiedValues
            $($short_name(SpecifiedValue<$short_name>),)+
            $($long_name(SpecifiedValue<$long_name>),)+
            $($long_m5e_name(SpecifiedValue<$long_m5e_name>),)+
            $($nonprop_name(SpecifiedValue<$nonprop_name>),)+
        }

        enum ComputedValue {
            $(
                $long_name($long_name),
            )+

            $(
                $long_m5e_name($long_m5e_name),
            )+

            $(
                $nonprop_name($nonprop_name),
            )+
        }

        /// Holds the computed values for the CSS properties of an element.
        #[derive(Debug, Default, Clone)]
        pub struct ComputedValues {
            $(
                $long_field: $long_name,
            )+

            $(
                $long_m5e_field: $long_m5e_name,
            )+

            $(
                $nonprop_field: $nonprop_name,
            )+
        }

        impl ParsedProperty {
            fn get_property_id(&self) -> PropertyId {
                match *self {
                    $(ParsedProperty::$long_name(_) => PropertyId::$long_name,)+
                    $(ParsedProperty::$long_m5e_name(_) => PropertyId::$long_m5e_name,)+
                    $(ParsedProperty::$short_name(_) => PropertyId::$short_name,)+
                    $(ParsedProperty::$nonprop_name(_) => PropertyId::$nonprop_name,)+
                }
            }

            fn unspecified(id: PropertyId) -> Self {
                use SpecifiedValue::Unspecified;

                match id {
                    $(PropertyId::$long_name => ParsedProperty::$long_name(Unspecified),)+
                    $(PropertyId::$long_m5e_name => ParsedProperty::$long_m5e_name(Unspecified),)+
                    $(PropertyId::$short_name => ParsedProperty::$short_name(Unspecified),)+
                    $(PropertyId::$nonprop_name => ParsedProperty::$nonprop_name(Unspecified),)+

                    PropertyId::UnsetProperty => unreachable!(),
                }
            }
        }

        impl ComputedValues {
            $(
                pub fn $long_field(&self) -> $long_name {
                    if let ComputedValue::$long_name(v) = self.get_value(PropertyId::$long_name) {
                        v
                    } else {
                        unreachable!();
                    }
                }
            )+

            $(
                pub fn $long_m5e_field(&self) -> $long_m5e_name {
                    if let ComputedValue::$long_m5e_name(v) = self.get_value(PropertyId::$long_m5e_name) {
                        v
                    } else {
                        unreachable!();
                    }
                }
            )+

            $(
                pub fn $nonprop_field(&self) -> $nonprop_name {
                    if let ComputedValue::$nonprop_name(v) = self.get_value(PropertyId::$nonprop_name) {
                        v
                    } else {
                        unreachable!();
                    }
                }
            )+

            fn set_value(&mut self, computed: ComputedValue) {
                match computed {
                    $(ComputedValue::$long_name(v) => self.$long_field = v,)+
                    $(ComputedValue::$long_m5e_name(v) => self.$long_m5e_field = v,)+
                    $(ComputedValue::$nonprop_name(v) => self.$nonprop_field = v,)+
                }
            }

            fn get_value(&self, id: PropertyId) -> ComputedValue {
                assert!(!id.is_shorthand());

                match id {
                    $(
                        PropertyId::$long_name =>
                            ComputedValue::$long_name(self.$long_field.clone()),
                    )+
                    $(
                        PropertyId::$long_m5e_name =>
                            ComputedValue::$long_m5e_name(self.$long_m5e_field.clone()),
                    )+
                    $(
                        PropertyId::$nonprop_name =>
                            ComputedValue::$nonprop_name(self.$nonprop_field.clone()),
                    )+
                    _ => unreachable!(),
                }
            }
        }

        pub fn parse_property<'i>(
            prop_name: &QualName,
            input: &mut Parser<'i, '_>,
            accept_shorthands: bool
        ) -> Result<ParsedProperty, ParseError<'i>> {
            match prop_name.expanded() {
                $(
                    expanded_name!("", $long_str) =>
                        Ok(ParsedProperty::$long_name(parse_input(input)?)),
                )+

                $(
                    e if e == ExpandedName {
                        ns: &ns!(),
                        local: &LocalName::from($long_m5e_str),
                    } =>
                        Ok(ParsedProperty::$long_m5e_name(parse_input(input)?)),
                )+

                $(
                    expanded_name!("", $short_str) => {
                        if accept_shorthands {
                            Ok(ParsedProperty::$short_name(parse_input(input)?))
                        } else {
                            let loc = input.current_source_location();
                            Err(loc.new_custom_error(ValueErrorKind::UnknownProperty))
                        }
                    }
                )+

                _ => {
                    let loc = input.current_source_location();
                    Err(loc.new_custom_error(ValueErrorKind::UnknownProperty))
                }
            }
        }
    };
}

#[rustfmt::skip]
make_properties! {
    shorthands: {
        "font"                        => font                        : Font,
        "marker"                      => marker                      : Marker,
    }

    longhands: {
        "baseline-shift"              => baseline_shift              : BaselineShift,
        "clip-path"                   => clip_path                   : ClipPath,
        "clip-rule"                   => clip_rule                   : ClipRule,
        "color"                       => color                       : Color,
        "color-interpolation-filters" => color_interpolation_filters : ColorInterpolationFilters,
        "direction"                   => direction                   : Direction,
        "display"                     => display                     : Display,
        "enable-background"           => enable_background           : EnableBackground,
        "fill"                        => fill                        : Fill,
        "fill-opacity"                => fill_opacity                : FillOpacity,
        "fill-rule"                   => fill_rule                   : FillRule,
        "filter"                      => filter                      : Filter,
        "flood-color"                 => flood_color                 : FloodColor,
        "flood-opacity"               => flood_opacity               : FloodOpacity,
        "font-family"                 => font_family                 : FontFamily,
        "font-size"                   => font_size                   : FontSize,
        "font-stretch"                => font_stretch                : FontStretch,
        "font-style"                  => font_style                  : FontStyle,
        "font-variant"                => font_variant                : FontVariant,
        "font-weight"                 => font_weight                 : FontWeight,
        "letter-spacing"              => letter_spacing              : LetterSpacing,
        "lighting-color"              => lighting_color              : LightingColor,
        "marker-end"                  => marker_end                  : MarkerEnd,
        "marker-mid"                  => marker_mid                  : MarkerMid,
        "marker-start"                => marker_start                : MarkerStart,
        "mask"                        => mask                        : Mask,
        "opacity"                     => opacity                     : Opacity,
        "overflow"                    => overflow                    : Overflow,
        "shape-rendering"             => shape_rendering             : ShapeRendering,
        "stop-color"                  => stop_color                  : StopColor,
        "stop-opacity"                => stop_opacity                : StopOpacity,
        "stroke"                      => stroke                      : Stroke,
        "stroke-dasharray"            => stroke_dasharray            : StrokeDasharray,
        "stroke-dashoffset"           => stroke_dashoffset           : StrokeDashoffset,
        "stroke-linecap"              => stroke_line_cap             : StrokeLinecap,
        "stroke-linejoin"             => stroke_line_join            : StrokeLinejoin,
        "stroke-miterlimit"           => stroke_miterlimit           : StrokeMiterlimit,
        "stroke-opacity"              => stroke_opacity              : StrokeOpacity,
        "stroke-width"                => stroke_width                : StrokeWidth,
        "text-anchor"                 => text_anchor                 : TextAnchor,
        "text-decoration"             => text_decoration             : TextDecoration,
        "text-rendering"              => text_rendering              : TextRendering,
        "unicode-bidi"                => unicode_bidi                : UnicodeBidi,
        "visibility"                  => visibility                  : Visibility,
        "writing-mode"                => writing_mode                : WritingMode,
    }

    longhands_not_supported_by_markup5ever: {
        "line-height"                 => line_height                 : LineHeight,
        "mix-blend-mode"              => mix_blend_mode              : MixBlendMode,
        "paint-order"                 => paint_order                 : PaintOrder,
    }

    // These are not properties, but presentation attributes.  However,
    // both xml:lang and xml:space *do* inherit.  We are abusing the
    // property inheritance code for these XML-specific attributes.
    non_properties: {
        xml_lang: XmlLang,
        xml_space: XmlSpace,
    }
}

impl SpecifiedValues {
    fn property_index(&self, id: PropertyId) -> Option<usize> {
        let v = self.indices[id.as_usize()];

        if v == PropertyId::UnsetProperty.as_u8() {
            None
        } else {
            Some(v as usize)
        }
    }

    fn set_property(&mut self, prop: &ParsedProperty, replace: bool) {
        let id = prop.get_property_id();
        assert!(!id.is_shorthand());

        if let Some(index) = self.property_index(id) {
            if replace {
                self.props[index] = prop.clone();
            }
        } else {
            self.props.push(prop.clone());
            let pos = self.props.len() - 1;
            self.indices[id.as_usize()] = pos as u8;
        }
    }

    fn get_property(&self, id: PropertyId) -> ParsedProperty {
        assert!(!id.is_shorthand());

        if let Some(index) = self.property_index(id) {
            self.props[index].clone()
        } else {
            ParsedProperty::unspecified(id)
        }
    }

    fn set_property_expanding_shorthands(&mut self, prop: &ParsedProperty, replace: bool) {
        match *prop {
            ParsedProperty::Font(SpecifiedValue::Specified(ref f)) => {
                self.expand_font_shorthand(f, replace)
            }
            ParsedProperty::Marker(SpecifiedValue::Specified(ref m)) => {
                self.expand_marker_shorthand(m, replace)
            }

            _ => self.set_property(prop, replace),
        }
    }

    fn expand_font_shorthand(&mut self, font: &Font, replace: bool) {
        let FontSpec {
            style,
            variant,
            weight,
            stretch,
            size,
            line_height,
            family,
        } = font.to_font_spec();

        self.set_property(
            &ParsedProperty::FontStyle(SpecifiedValue::Specified(style)),
            replace,
        );
        self.set_property(
            &ParsedProperty::FontVariant(SpecifiedValue::Specified(variant)),
            replace,
        );
        self.set_property(
            &ParsedProperty::FontWeight(SpecifiedValue::Specified(weight)),
            replace,
        );
        self.set_property(
            &ParsedProperty::FontStretch(SpecifiedValue::Specified(stretch)),
            replace,
        );
        self.set_property(
            &ParsedProperty::FontSize(SpecifiedValue::Specified(size)),
            replace,
        );
        self.set_property(
            &ParsedProperty::LineHeight(SpecifiedValue::Specified(line_height)),
            replace,
        );
        self.set_property(
            &ParsedProperty::FontFamily(SpecifiedValue::Specified(family)),
            replace,
        );
    }

    fn expand_marker_shorthand(&mut self, marker: &Marker, replace: bool) {
        let Marker(v) = marker;

        self.set_property(
            &ParsedProperty::MarkerStart(SpecifiedValue::Specified(MarkerStart(v.clone()))),
            replace,
        );
        self.set_property(
            &ParsedProperty::MarkerMid(SpecifiedValue::Specified(MarkerMid(v.clone()))),
            replace,
        );
        self.set_property(
            &ParsedProperty::MarkerEnd(SpecifiedValue::Specified(MarkerEnd(v.clone()))),
            replace,
        );
    }

    pub fn set_parsed_property(&mut self, prop: &ParsedProperty) {
        self.set_property_expanding_shorthands(prop, true);
    }

    /* user agent property have less priority than presentation attributes */
    pub fn set_parsed_property_user_agent(&mut self, prop: &ParsedProperty) {
        self.set_property_expanding_shorthands(prop, false);
    }

    pub fn to_computed_values(&self, computed: &mut ComputedValues) {
        macro_rules! compute {
            ($name:ident, $field:ident) => {
                let prop_val = self.get_property(PropertyId::$name);
                if let ParsedProperty::$name(s) = prop_val {
                    computed.set_value(ComputedValue::$name(
                        s.compute(&computed.$field(), computed),
                    ));
                } else {
                    unreachable!();
                }
            };
        }

        // First, compute font_size.  It needs to be done before everything
        // else, so that properties that depend on its computed value
        // will be able to use it.  For example, baseline-shift
        // depends on font-size.

        compute!(FontSize, font_size);

        // Then, do all the other properties.

        compute!(BaselineShift, baseline_shift);
        compute!(ClipPath, clip_path);
        compute!(ClipRule, clip_rule);
        compute!(Color, color);
        compute!(ColorInterpolationFilters, color_interpolation_filters);
        compute!(Direction, direction);
        compute!(Display, display);
        compute!(EnableBackground, enable_background);
        compute!(Fill, fill);
        compute!(FillOpacity, fill_opacity);
        compute!(FillRule, fill_rule);
        compute!(Filter, filter);
        compute!(FloodColor, flood_color);
        compute!(FloodOpacity, flood_opacity);
        compute!(FontFamily, font_family);
        compute!(FontStretch, font_stretch);
        compute!(FontStyle, font_style);
        compute!(FontVariant, font_variant);
        compute!(FontWeight, font_weight);
        compute!(LetterSpacing, letter_spacing);
        compute!(LightingColor, lighting_color);
        compute!(MarkerEnd, marker_end);
        compute!(MarkerMid, marker_mid);
        compute!(MarkerStart, marker_start);
        compute!(Mask, mask);
        compute!(MixBlendMode, mix_blend_mode);
        compute!(Opacity, opacity);
        compute!(Overflow, overflow);
        compute!(PaintOrder, paint_order);
        compute!(ShapeRendering, shape_rendering);
        compute!(StopColor, stop_color);
        compute!(StopOpacity, stop_opacity);
        compute!(Stroke, stroke);
        compute!(StrokeDasharray, stroke_dasharray);
        compute!(StrokeDashoffset, stroke_dashoffset);
        compute!(StrokeLinecap, stroke_line_cap);
        compute!(StrokeLinejoin, stroke_line_join);
        compute!(StrokeOpacity, stroke_opacity);
        compute!(StrokeMiterlimit, stroke_miterlimit);
        compute!(StrokeWidth, stroke_width);
        compute!(TextAnchor, text_anchor);
        compute!(TextDecoration, text_decoration);
        compute!(TextRendering, text_rendering);
        compute!(UnicodeBidi, unicode_bidi);
        compute!(Visibility, visibility);
        compute!(WritingMode, writing_mode);
        compute!(XmlLang, xml_lang);
        compute!(XmlSpace, xml_space);
    }

    pub fn is_overflow(&self) -> bool {
        if let Some(overflow_index) = self.property_index(PropertyId::Overflow) {
            match self.props[overflow_index] {
                ParsedProperty::Overflow(SpecifiedValue::Specified(Overflow::Auto)) => true,
                ParsedProperty::Overflow(SpecifiedValue::Specified(Overflow::Visible)) => true,
                ParsedProperty::Overflow(_) => false,
                _ => unreachable!(),
            }
        } else {
            false
        }
    }

    fn parse_one_presentation_attribute(
        &mut self,
        attr: QualName,
        value: &str,
    ) -> Result<(), ElementError> {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);

        // Presentation attributes don't accept shorthands, e.g. there is no
        // attribute like marker="#foo" and it needs to be set in the style attribute
        // like style="marker: #foo;".  So, pass false for accept_shorthands here.
        match parse_property(&attr, &mut parser, false) {
            Ok(prop) => {
                if parser.expect_exhausted().is_ok() {
                    self.set_parsed_property(&prop);
                } else {
                    rsvg_log!(
                        "(ignoring invalid presentation attribute {:?}\n    value=\"{}\")\n",
                        attr.expanded(),
                        value,
                    );
                }
            }

            // not a presentation attribute; just ignore it
            Err(ParseError {
                kind: ParseErrorKind::Custom(ValueErrorKind::UnknownProperty),
                ..
            }) => (),

            // https://www.w3.org/TR/CSS2/syndata.html#unsupported-values
            // For all the following cases, ignore illegal values; don't set the whole node to
            // be in error in that case.
            Err(ParseError {
                kind: ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(ref t)),
                ..
            }) => {
                let mut tok = String::new();

                t.to_css(&mut tok).unwrap(); // FIXME: what do we do with a fmt::Error?
                rsvg_log!(
                    "(ignoring invalid presentation attribute {:?}\n    value=\"{}\"\n    \
                     unexpected token '{}')",
                    attr.expanded(),
                    value,
                    tok,
                );
            }

            Err(ParseError {
                kind: ParseErrorKind::Basic(BasicParseErrorKind::EndOfInput),
                ..
            }) => {
                rsvg_log!(
                    "(ignoring invalid presentation attribute {:?}\n    value=\"{}\"\n    \
                     unexpected end of input)",
                    attr.expanded(),
                    value,
                );
            }

            Err(ParseError {
                kind: ParseErrorKind::Basic(_),
                ..
            }) => {
                rsvg_log!(
                    "(ignoring invalid presentation attribute {:?}\n    value=\"{}\"\n    \
                     unexpected error)",
                    attr.expanded(),
                    value,
                );
            }

            Err(ParseError {
                kind: ParseErrorKind::Custom(ref v),
                ..
            }) => {
                rsvg_log!(
                    "(ignoring invalid presentation attribute {:?}\n    value=\"{}\"\n    {})",
                    attr.expanded(),
                    value,
                    v
                );
            }
        }

        Ok(())
    }

    pub fn parse_presentation_attributes(
        &mut self,
        attrs: &Attributes,
    ) -> Result<(), ElementError> {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!(xml "lang") => {
                    // xml:lang is a non-presentation attribute and as such cannot have the
                    // "inherit" value.  So, we don't call parse_one_presentation_attribute()
                    // for it, but rather call its parser directly.
                    self.set_parsed_property(&ParsedProperty::XmlLang(SpecifiedValue::Specified(
                        attr.parse(value)?,
                    )));
                }

                expanded_name!(xml "space") => {
                    // xml:space is a non-presentation attribute and as such cannot have the
                    // "inherit" value.  So, we don't call parse_one_presentation_attribute()
                    // for it, but rather call its parser directly.
                    self.set_parsed_property(&ParsedProperty::XmlSpace(SpecifiedValue::Specified(
                        attr.parse(value)?,
                    )));
                }

                _ => self.parse_one_presentation_attribute(attr, value)?,
            }
        }

        Ok(())
    }

    pub fn set_property_from_declaration(
        &mut self,
        declaration: &Declaration,
        origin: Origin,
        important_styles: &mut HashSet<QualName>,
    ) {
        if !declaration.important && important_styles.contains(&declaration.prop_name) {
            return;
        }

        if declaration.important {
            important_styles.insert(declaration.prop_name.clone());
        }

        if origin == Origin::UserAgent {
            self.set_parsed_property_user_agent(&declaration.property);
        } else {
            self.set_parsed_property(&declaration.property);
        }
    }

    pub fn parse_style_declarations(
        &mut self,
        declarations: &str,
        origin: Origin,
        important_styles: &mut HashSet<QualName>,
    ) -> Result<(), ElementError> {
        let mut input = ParserInput::new(declarations);
        let mut parser = Parser::new(&mut input);

        DeclarationListParser::new(&mut parser, DeclParser)
            .filter_map(|r| match r {
                Ok(decl) => Some(decl),
                Err(e) => {
                    rsvg_log!("Invalid declaration; ignoring: {:?}", e);
                    None
                }
            })
            .for_each(|decl| self.set_property_from_declaration(&decl, origin, important_styles));

        Ok(())
    }
}

// Parses the value for the type `T` of the property out of the Parser, including `inherit` values.
fn parse_input<'i, T>(input: &mut Parser<'i, '_>) -> Result<SpecifiedValue<T>, ParseError<'i>>
where
    T: Property<ComputedValues> + Clone + Default + Parse,
{
    if input
        .try_parse(|p| p.expect_ident_matching("inherit"))
        .is_ok()
    {
        Ok(SpecifiedValue::Inherit)
    } else {
        Parse::parse(input).map(SpecifiedValue::Specified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iri::IRI;
    use crate::length::*;

    #[test]
    fn empty_values_computes_to_defaults() {
        let specified = SpecifiedValues::default();

        let mut computed = ComputedValues::default();
        specified.to_computed_values(&mut computed);

        assert_eq!(computed.stroke_width(), StrokeWidth::default());
    }

    #[test]
    fn set_one_property() {
        let length = Length::<Both>::new(42.0, LengthUnit::Px);

        let mut specified = SpecifiedValues::default();
        specified.set_parsed_property(&ParsedProperty::StrokeWidth(SpecifiedValue::Specified(
            StrokeWidth(length),
        )));

        let mut computed = ComputedValues::default();
        specified.to_computed_values(&mut computed);

        assert_eq!(computed.stroke_width(), StrokeWidth(length));
    }

    #[test]
    fn replace_existing_property() {
        let length1 = Length::<Both>::new(42.0, LengthUnit::Px);
        let length2 = Length::<Both>::new(42.0, LengthUnit::Px);

        let mut specified = SpecifiedValues::default();

        specified.set_parsed_property(&ParsedProperty::StrokeWidth(SpecifiedValue::Specified(
            StrokeWidth(length1),
        )));

        specified.set_parsed_property(&ParsedProperty::StrokeWidth(SpecifiedValue::Specified(
            StrokeWidth(length2),
        )));

        let mut computed = ComputedValues::default();
        specified.to_computed_values(&mut computed);

        assert_eq!(computed.stroke_width(), StrokeWidth(length2));
    }

    #[test]
    fn expands_marker_shorthand() {
        let mut specified = SpecifiedValues::default();
        let iri = IRI::parse_str("url(#foo)").unwrap();

        let marker = Marker(iri.clone());
        specified.set_parsed_property(&ParsedProperty::Marker(SpecifiedValue::Specified(marker)));

        let mut computed = ComputedValues::default();
        specified.to_computed_values(&mut computed);

        assert_eq!(computed.marker_start(), MarkerStart(iri.clone()));
        assert_eq!(computed.marker_mid(), MarkerMid(iri.clone()));
        assert_eq!(computed.marker_end(), MarkerEnd(iri.clone()));
    }

    #[test]
    fn replaces_marker_shorthand() {
        let mut specified = SpecifiedValues::default();
        let iri1 = IRI::parse_str("url(#foo)").unwrap();
        let iri2 = IRI::None;

        let marker1 = Marker(iri1.clone());
        specified.set_parsed_property(&ParsedProperty::Marker(SpecifiedValue::Specified(marker1)));

        let marker2 = Marker(iri2.clone());
        specified.set_parsed_property(&ParsedProperty::Marker(SpecifiedValue::Specified(marker2)));

        let mut computed = ComputedValues::default();
        specified.to_computed_values(&mut computed);

        assert_eq!(computed.marker_start(), MarkerStart(iri2.clone()));
        assert_eq!(computed.marker_mid(), MarkerMid(iri2.clone()));
        assert_eq!(computed.marker_end(), MarkerEnd(iri2.clone()));
    }

    #[test]
    fn computes_property_that_does_not_inherit_automatically() {
        assert_eq!(
            <Opacity as Property<ComputedValues>>::inherits_automatically(),
            false
        );

        let half_opacity = Opacity::parse_str("0.5").unwrap();

        // first level, as specified with opacity

        let mut with_opacity = SpecifiedValues::default();
        with_opacity.set_parsed_property(&ParsedProperty::Opacity(SpecifiedValue::Specified(
            half_opacity.clone(),
        )));

        let mut computed_0_5 = ComputedValues::default();
        with_opacity.to_computed_values(&mut computed_0_5);

        assert_eq!(computed_0_5.opacity(), half_opacity.clone());

        // second level, no opacity specified, and it doesn't inherit

        let without_opacity = SpecifiedValues::default();

        let mut computed = computed_0_5.clone();
        without_opacity.to_computed_values(&mut computed);

        assert_eq!(computed.opacity(), Opacity::default());

        // another at second level, opacity set to explicitly inherit

        let mut with_inherit_opacity = SpecifiedValues::default();
        with_inherit_opacity.set_parsed_property(&ParsedProperty::Opacity(SpecifiedValue::Inherit));

        let mut computed = computed_0_5.clone();
        with_inherit_opacity.to_computed_values(&mut computed);

        assert_eq!(computed.opacity(), half_opacity.clone());
    }
}
