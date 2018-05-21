use cairo::{self, MatrixTrait};
use cssparser;
use glib;
use glib::translate::*;
use glib_sys;
use libc;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr;
use std::str::FromStr;

use attributes::Attribute;
use color::rgba_to_argb;
use cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use error::*;
use handle::RsvgHandle;
use iri::IRI;
use length::{Dasharray, LengthDir, RsvgLength};
use node::RsvgNode;
use paint_server::PaintServer;
use parsers::Parse;
use property_bag::PropertyBag;
use property_macros::Property;
use unitinterval::UnitInterval;
use util::{utf8_cstr, utf8_cstr_opt};

// This is only used as *const RsvgState or *mut RsvgState, as an opaque pointer for C
pub enum RsvgState {}

/// Holds the state of CSS properties
///
/// This is used for various purposes:
///
/// * Immutably, to store the attributes of element nodes after parsing.
/// * Mutably, during cascading/rendering.
///
/// Each property should have its own data type, and implement
/// `Default` and `parsers::Parse`.
///
/// If a property is `None`, is means it was not specified and must be
/// inherited from the parent state, or in the end the caller can
/// `.unwrap_or_default()` to get the default value for the property.

// FIXME: #[derive(Clone)] is not correct here; states are not meant
// to be cloned.  We should remove this when we remove the hack in
// state_reinherit_top(), to clone_from() while preserving the parent
#[derive(Clone)]
pub struct State {
    pub parent: *const RsvgState,

    pub affine: cairo::Matrix,

    pub baseline_shift: Option<BaselineShift>,
    pub clip_path: Option<ClipPath>,
    pub clip_rule: Option<ClipRule>,
    pub comp_op: Option<CompOp>,
    pub color: Option<Color>,
    pub direction: Option<Direction>,
    pub display: Option<Display>,
    pub enable_background: Option<EnableBackground>,
    pub fill: Option<Fill>,
    pub fill_opacity: Option<FillOpacity>,
    pub fill_rule: Option<FillRule>,
    pub filter: Option<Filter>,
    pub flood_color: Option<FloodColor>,
    pub flood_opacity: Option<FloodOpacity>,
    pub font_family: Option<FontFamily>,
    pub font_size: Option<FontSize>,
    pub font_stretch: Option<FontStretch>,
    pub font_style: Option<FontStyle>,
    pub font_variant: Option<FontVariant>,
    pub font_weight: Option<FontWeight>,
    pub letter_spacing: Option<LetterSpacing>,
    pub marker_end: Option<MarkerEnd>,
    pub marker_mid: Option<MarkerMid>,
    pub marker_start: Option<MarkerStart>,
    pub mask: Option<Mask>,
    pub opacity: Option<Opacity>,
    pub overflow: Option<Overflow>,
    pub shape_rendering: Option<ShapeRendering>,
    pub stop_color: Option<StopColor>,
    pub stop_opacity: Option<StopOpacity>,
    pub stroke: Option<Stroke>,
    pub stroke_dasharray: Option<StrokeDasharray>,
    pub stroke_dashoffset: Option<StrokeDashoffset>,
    pub stroke_line_cap: Option<StrokeLinecap>,
    pub stroke_line_join: Option<StrokeLinejoin>,
    pub stroke_opacity: Option<StrokeOpacity>,
    pub stroke_miterlimit: Option<StrokeMiterlimit>,
    pub stroke_width: Option<StrokeWidth>,
    pub text_anchor: Option<TextAnchor>,
    pub text_decoration: Option<TextDecoration>,
    pub text_rendering: Option<TextRendering>,
    pub unicode_bidi: Option<UnicodeBidi>,
    pub visibility: Option<Visibility>,
    pub writing_mode: Option<WritingMode>,
    pub xml_lang: Option<XmlLang>,
    pub xml_space: Option<XmlSpace>,

    important_styles: RefCell<HashSet<Attribute>>,
    pub cond: bool,
}

impl State {
    pub fn new_with_parent(parent: Option<&State>) -> State {
        if let Some(parent) = parent {
            State::new(to_c(parent))
        } else {
            State::new(ptr::null())
        }
    }

    fn new(parent: *const RsvgState) -> State {
        State {
            parent,

            affine: cairo::Matrix::identity(),

            // please keep these sorted
            baseline_shift: Default::default(),
            clip_path: Default::default(),
            clip_rule: Default::default(),
            color: Default::default(),
            comp_op: Default::default(),
            direction: Default::default(),
            display: Default::default(),
            enable_background: Default::default(),
            fill: Default::default(),
            fill_opacity: Default::default(),
            fill_rule: Default::default(),
            filter: Default::default(),
            flood_color: Default::default(),
            flood_opacity: Default::default(),
            font_family: Default::default(),
            font_size: Default::default(),
            font_stretch: Default::default(),
            font_style: Default::default(),
            font_variant: Default::default(),
            font_weight: Default::default(),
            letter_spacing: Default::default(),
            marker_end: Default::default(),
            marker_mid: Default::default(),
            marker_start: Default::default(),
            mask: Default::default(),
            opacity: Default::default(),
            overflow: Default::default(),
            shape_rendering: Default::default(),

            // The following two start as None (i.e. inherit).  This
            // is so that the first pass of inherit_run(), called from
            // reconstruct() from the "stop" element code, will
            // correctly initialize the destination state from the
            // toplevel element.
            stop_color: None,
            stop_opacity: None,

            stroke: Default::default(),
            stroke_dasharray: Default::default(),
            stroke_dashoffset: Default::default(),
            stroke_line_cap: Default::default(),
            stroke_line_join: Default::default(),
            stroke_opacity: Default::default(),
            stroke_miterlimit: Default::default(),
            stroke_width: Default::default(),
            text_anchor: Default::default(),
            text_decoration: Default::default(),
            text_rendering: Default::default(),
            unicode_bidi: Default::default(),
            visibility: Default::default(),
            writing_mode: Default::default(),
            xml_lang: Default::default(),
            xml_space: Default::default(),

            important_styles: Default::default(),
            cond: true,
        }
    }

    pub fn parent<'a>(&self) -> Option<&'a State> {
        if self.parent.is_null() {
            None
        } else {
            Some(from_c(self.parent))
        }
    }

    pub fn reinherit(&mut self, src: &State) {
        self.inherit_run(src, State::reinheritfunction, false);
    }

    pub fn inherit(&mut self, src: &State) {
        self.inherit_run(src, State::inheritfunction, true);
    }

    pub fn force(&mut self, src: &State) {
        self.inherit_run(src, State::forcefunction, false);
    }

    pub fn dominate(&mut self, src: &State) {
        self.inherit_run(src, State::dominatefunction, false);
    }

    pub fn reconstruct(&mut self, node: &RsvgNode) {
        if let Some(parent) = node.get_parent() {
            self.reconstruct(&parent);
            self.inherit(node.get_state());
        }
    }

    // reinherit is given dst which is the top of the state stack
    // and src which is the layer before in the state stack from
    // which it should be inherited
    fn reinheritfunction(dst: bool, _src: bool) -> bool {
        if !dst {
            true
        } else {
            false
        }
    }

    // put something new on the inheritance stack, dst is the top of the stack,
    // src is the state to be integrated, this is essentially the opposite of
    // reinherit, because it is being given stuff to be integrated on the top,
    // rather than the context underneath.
    fn inheritfunction(_dst: bool, src: bool) -> bool {
        src
    }

    // copy everything inheritable from the src to the dst */
    fn forcefunction(_dst: bool, _src: bool) -> bool {
        true
    }

    // dominate is given dst which is the top of the state stack and
    // src which is the layer before in the state stack from which it
    // should be inherited from, however if anything is directly
    // specified in src (the second last layer) it will override
    // anything on the top layer, this is for overrides in <use> tags
    fn dominatefunction(dst: bool, src: bool) -> bool {
        if !dst || src {
            true
        } else {
            false
        }
    }

    fn inherit_run(
        &mut self,
        src: &State,
        inherit_fn: fn(bool, bool) -> bool,
        inherituninheritables: bool,
    ) {
        // please keep these sorted
        inherit(inherit_fn, &mut self.baseline_shift, &src.baseline_shift);
        inherit(inherit_fn, &mut self.clip_rule, &src.clip_rule);
        inherit(inherit_fn, &mut self.color, &src.color);
        inherit(inherit_fn, &mut self.direction, &src.direction);
        inherit(inherit_fn, &mut self.display, &src.display);
        inherit(inherit_fn, &mut self.fill, &src.fill);
        inherit(inherit_fn, &mut self.fill_opacity, &src.fill_opacity);
        inherit(inherit_fn, &mut self.fill_rule, &src.fill_rule);
        inherit(inherit_fn, &mut self.flood_color, &src.flood_color);
        inherit(inherit_fn, &mut self.flood_opacity, &src.flood_opacity);
        inherit(inherit_fn, &mut self.font_family, &src.font_family);
        inherit(inherit_fn, &mut self.font_size, &src.font_size);
        inherit(inherit_fn, &mut self.font_stretch, &src.font_stretch);
        inherit(inherit_fn, &mut self.font_style, &src.font_style);
        inherit(inherit_fn, &mut self.font_variant, &src.font_variant);
        inherit(inherit_fn, &mut self.font_weight, &src.font_weight);
        inherit(inherit_fn, &mut self.letter_spacing, &src.letter_spacing);
        inherit(inherit_fn, &mut self.marker_end, &src.marker_end);
        inherit(inherit_fn, &mut self.marker_mid, &src.marker_mid);
        inherit(inherit_fn, &mut self.marker_start, &src.marker_start);
        inherit(inherit_fn, &mut self.overflow, &src.overflow);
        inherit(inherit_fn, &mut self.shape_rendering, &src.shape_rendering);
        inherit(inherit_fn, &mut self.stop_color, &src.stop_color);
        inherit(inherit_fn, &mut self.stop_opacity, &src.stop_opacity);
        inherit(inherit_fn, &mut self.stroke, &src.stroke);
        inherit(
            inherit_fn,
            &mut self.stroke_dasharray,
            &src.stroke_dasharray,
        );
        inherit(
            inherit_fn,
            &mut self.stroke_dashoffset,
            &src.stroke_dashoffset,
        );
        inherit(inherit_fn, &mut self.stroke_line_cap, &src.stroke_line_cap);
        inherit(
            inherit_fn,
            &mut self.stroke_line_join,
            &src.stroke_line_join,
        );
        inherit(inherit_fn, &mut self.stroke_opacity, &src.stroke_opacity);
        inherit(
            inherit_fn,
            &mut self.stroke_miterlimit,
            &src.stroke_miterlimit,
        );
        inherit(inherit_fn, &mut self.stroke_width, &src.stroke_width);
        inherit(inherit_fn, &mut self.text_anchor, &src.text_anchor);
        inherit(inherit_fn, &mut self.text_decoration, &src.text_decoration);
        inherit(inherit_fn, &mut self.text_rendering, &src.text_rendering);
        inherit(inherit_fn, &mut self.unicode_bidi, &src.unicode_bidi);
        inherit(inherit_fn, &mut self.visibility, &src.visibility);
        inherit(inherit_fn, &mut self.xml_lang, &src.xml_lang);
        inherit(inherit_fn, &mut self.xml_space, &src.xml_space);

        self.cond = src.cond;

        if inherituninheritables {
            self.clip_path.clone_from(&src.clip_path);
            self.comp_op.clone_from(&src.comp_op);
            self.enable_background.clone_from(&src.enable_background);
            self.filter.clone_from(&src.filter);
            self.mask.clone_from(&src.mask);
            self.opacity.clone_from(&src.opacity);
        }
    }

    fn parse_style_pair(
        &mut self,
        attr: Attribute,
        value: &str,
        important: bool,
        accept_shorthands: bool,
    ) -> Result<(), NodeError> {
        if !important && self.important_styles.borrow().contains(&attr) {
            return Ok(());
        }

        if important {
            self.important_styles.borrow_mut().insert(attr);
        }

        // FIXME: move this to "do catch" when we can bump the rustc version dependency
        let mut parse = || -> Result<(), AttributeError> {
            // please keep these sorted
            match attr {
                Attribute::BaselineShift => {
                    self.baseline_shift = parse_property(value, ())?;
                }

                Attribute::ClipPath => {
                    self.clip_path = parse_property(value, ())?;
                }

                Attribute::ClipRule => {
                    self.clip_rule = parse_property(value, ())?;
                }

                Attribute::Color => {
                    self.color = parse_property(value, ())?;
                }

                Attribute::CompOp => {
                    self.comp_op = parse_property(value, ())?;
                }

                Attribute::Direction => {
                    self.direction = parse_property(value, ())?;
                }

                Attribute::Display => {
                    self.display = parse_property(value, ())?;
                }

                Attribute::EnableBackground => {
                    self.enable_background = parse_property(value, ())?;
                }

                Attribute::Fill => {
                    self.fill = parse_property(value, ())?;
                }

                Attribute::FillOpacity => {
                    self.fill_opacity = parse_property(value, ())?;
                }

                Attribute::FillRule => {
                    self.fill_rule = parse_property(value, ())?;
                }

                Attribute::Filter => {
                    self.filter = parse_property(value, ())?;
                }

                Attribute::FloodColor => {
                    self.flood_color = parse_property(value, ())?;
                }

                Attribute::FloodOpacity => {
                    self.flood_opacity = parse_property(value, ())?;
                }

                Attribute::FontFamily => {
                    self.font_family = parse_property(value, ())?;
                }

                Attribute::FontSize => {
                    self.font_size = parse_property(value, LengthDir::Both)?;
                }

                Attribute::FontStretch => {
                    self.font_stretch = parse_property(value, ())?;
                }

                Attribute::FontStyle => {
                    self.font_style = parse_property(value, ())?;
                }

                Attribute::FontVariant => {
                    self.font_variant = parse_property(value, ())?;
                }

                Attribute::FontWeight => {
                    self.font_weight = parse_property(value, ())?;
                }

                Attribute::LetterSpacing => {
                    self.letter_spacing = parse_property(value, LengthDir::Horizontal)?;
                }

                Attribute::MarkerEnd => {
                    self.marker_end = parse_property(value, ())?;
                }

                Attribute::MarkerMid => {
                    self.marker_mid = parse_property(value, ())?;
                }

                Attribute::MarkerStart => {
                    self.marker_start = parse_property(value, ())?;
                }

                Attribute::Marker if accept_shorthands => {
                    if self.marker_end.is_none() {
                        self.marker_end = parse_property(value, ())?;
                    }

                    if self.marker_mid.is_none() {
                        self.marker_mid = parse_property(value, ())?;
                    }

                    if self.marker_start.is_none() {
                        self.marker_start = parse_property(value, ())?;
                    }
                }

                Attribute::Mask => {
                    self.mask = parse_property(value, ())?;
                }

                Attribute::Opacity => {
                    self.opacity = parse_property(value, ())?;
                }

                Attribute::Overflow => {
                    self.overflow = parse_property(value, ())?;
                }

                Attribute::ShapeRendering => {
                    self.shape_rendering = parse_property(value, ())?;
                }

                Attribute::StopColor => {
                    self.stop_color = parse_property(value, ())?;
                }

                Attribute::StopOpacity => {
                    self.stop_opacity = parse_property(value, ())?;
                }

                Attribute::Stroke => {
                    self.stroke = parse_property(value, ())?;
                }

                Attribute::StrokeDasharray => {
                    self.stroke_dasharray = parse_property(value, ())?;
                }

                Attribute::StrokeDashoffset => {
                    self.stroke_dashoffset = parse_property(value, LengthDir::Both)?;
                }

                Attribute::StrokeLinecap => {
                    self.stroke_line_cap = parse_property(value, ())?;
                }

                Attribute::StrokeLinejoin => {
                    self.stroke_line_join = parse_property(value, ())?;
                }

                Attribute::StrokeOpacity => {
                    self.stroke_opacity = parse_property(value, ())?;
                }

                Attribute::StrokeMiterlimit => {
                    self.stroke_miterlimit = parse_property(value, ())?;
                }

                Attribute::StrokeWidth => {
                    self.stroke_width = parse_property(value, LengthDir::Both)?;
                }

                Attribute::TextAnchor => {
                    self.text_anchor = parse_property(value, ())?;
                }

                Attribute::TextDecoration => {
                    self.text_decoration = parse_property(value, ())?;
                }

                Attribute::TextRendering => {
                    self.text_rendering = parse_property(value, ())?;
                }

                Attribute::UnicodeBidi => {
                    self.unicode_bidi = parse_property(value, ())?;
                }

                Attribute::Visibility => {
                    self.visibility = parse_property(value, ())?;
                }

                Attribute::WritingMode => {
                    self.writing_mode = parse_property(value, ())?;
                }

                Attribute::XmlLang => {
                    // xml:lang is not a property; it is a non-presentation attribute and as such
                    // cannot have the "inherit" value.  So, we don't call parse_property() for it,
                    // but rather call its parser directly.
                    self.xml_lang = Some(XmlLang::parse(value, ())?);
                }

                Attribute::XmlSpace => {
                    // xml:space is not a property; it is a non-presentation attribute and as such
                    // cannot have the "inherit" value.  So, we don't call parse_property() for it,
                    // but rather call its parser directly.
                    self.xml_space = Some(XmlSpace::parse(value, ())?);
                }

                _ => {
                    // Maybe it's an attribute not parsed here, but in the
                    // node implementations.
                }
            }

            Ok(())
        };

        // https://www.w3.org/TR/CSS2/syndata.html#unsupported-values
        // Ignore unsupported / illegal values; don't set the whole
        // node to be in error in that case.
        // parse().map_err(|e| NodeError::attribute_error(attr, e))

        let _ = parse();

        Ok(())
    }

    pub fn parse_presentation_attributes(&mut self, pbag: &PropertyBag) -> Result<(), NodeError> {
        for (_key, attr, value) in pbag.iter() {
            self.parse_style_pair(attr, value, false, false)?;
        }

        Ok(())
    }

    pub fn parse_conditional_processing_attributes(
        &mut self,
        pbag: &PropertyBag,
    ) -> Result<(), NodeError> {
        for (_key, attr, value) in pbag.iter() {
            // FIXME: move this to "do catch" when we can bump the rustc version dependency
            let mut parse = || {
                match attr {
                    Attribute::RequiredExtensions if self.cond => {
                        self.cond = RequiredExtensions::parse(value, ())
                            .map(|RequiredExtensions(res)| res)?;
                    }

                    Attribute::RequiredFeatures if self.cond => {
                        self.cond =
                            RequiredFeatures::parse(value, ()).map(|RequiredFeatures(res)| res)?;
                    }

                    Attribute::SystemLanguage if self.cond => {
                        self.cond = SystemLanguage::parse(value, &glib::get_language_names())
                            .map(|SystemLanguage(res, _)| res)?;
                    }

                    _ => {}
                }

                Ok(())
            };

            parse().map_err(|e| NodeError::attribute_error(attr, e))?;
        }

        Ok(())
    }

    pub fn parse_style_declarations(&mut self, declarations: &str) -> Result<(), NodeError> {
        // Split an attribute value like style="foo: bar; baz: beep;" into
        // individual CSS declarations ("foo: bar" and "baz: beep") and
        // set them onto the state struct.
        //
        // FIXME: It's known that this is _way_ out of spec. A more complete
        // CSS2 implementation will happen later.

        for decl in declarations.split(';') {
            if let Some(colon_pos) = decl.find(':') {
                let (prop_name, value) = decl.split_at(colon_pos);

                let prop_name = prop_name.trim();
                let value = value[1..].trim();

                if !prop_name.is_empty() && !value.is_empty() {
                    // Just remove single quotes in a trivial way.  No handling for any
                    // special character inside the quotes is done.  This relates
                    // especially to font-family names.
                    let value = value.replace('\'', "");

                    let mut important = false;

                    let value = if let Some(bang_pos) = value.find('!') {
                        let (before_bang, bang_and_after) = value.split_at(bang_pos);

                        if bang_and_after[1..].trim() == "important" {
                            important = true;
                        }

                        before_bang.trim()
                    } else {
                        &value
                    };

                    if let Ok(attr) = Attribute::from_str(prop_name) {
                        self.parse_style_pair(attr, value, important, true)?;
                    }
                    // else unknown property name; ignore
                }
            }
        }

        Ok(())
    }

    pub fn is_overflow(&self) -> bool {
        match self.overflow {
            Some(Overflow::Auto) | Some(Overflow::Visible) => true,
            _ => false,
        }
    }

    pub fn is_visible(&self) -> bool {
        match (self.display, self.visibility) {
            (Some(Display::None), _) => false,
            (_, None) | (_, Some(Visibility::Visible)) => true,
            _ => false,
        }
    }

    pub fn text_gravity_is_vertical(&self) -> bool {
        match self.writing_mode {
            Some(WritingMode::Tb) | Some(WritingMode::TbRl) => true,
            _ => false,
        }
    }
}

// Parses the `value` for the type `T` of the property, including `inherit` values.
//
// If the `value` is `inherit`, returns `Ok(None)`; otherwise returns
// `Ok(Some(T))`.
fn parse_property<T>(value: &str, data: <T as Parse>::Data) -> Result<Option<T>, <T as Parse>::Err>
where
    T: Property + Parse,
{
    if value.trim() == "inherit" {
        Ok(None)
    } else {
        Parse::parse(value, data).map(Some)
    }
}

make_property!(
    BaselineShift,
    default: 0f64,
    inherits_automatically: true,
    newtype: f64
);

impl Parse for BaselineShift {
    type Data = ();
    type Err = AttributeError;

    // These values come from Inkscape's SP_CSS_BASELINE_SHIFT_(SUB/SUPER/BASELINE);
    // see sp_style_merge_baseline_shift_from_parent()
    fn parse(s: &str, _: Self::Data) -> Result<BaselineShift, ::error::AttributeError> {
        match s.trim() {
            "baseline" => Ok(BaselineShift(0f64)),
            "sub" => Ok(BaselineShift(-0.2f64)),
            "super" => Ok(BaselineShift(0.4f64)),

            _ => Err(::error::AttributeError::from(::parsers::ParseError::new(
                "invalid value",
            ))),
        }
    }
}

make_property!(
    ClipPath,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    ClipRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

// See bgo#764808: we don't inherit CSS from the public API,
// so start off with opaque black instead of transparent.
make_property!(
    Color,
    default: cssparser::RGBA::new(0, 0, 0, 0xff),
    inherits_automatically: true,
    newtype_parse: cssparser::RGBA,
    parse_data_type: ()
);

make_property!(
    CompOp,
    default: SrcOver,
    inherits_automatically: false,

    identifiers:
    "clear" => Clear,
    "src" => Src,
    "dst" => Dst,
    "src-over" => SrcOver,
    "dst-over" => DstOver,
    "src-in" => SrcIn,
    "dst-in" => DstIn,
    "src-out" => SrcOut,
    "dst-out" => DstOut,
    "src-atop" => SrcAtop,
    "dst-atop" => DstAtop,
    "xor" => Xor,
    "plus" => Plus,
    "multiply" => Multiply,
    "screen" => Screen,
    "overlay" => Overlay,
    "darken" => Darken,
    "lighten" => Lighten,
    "color-dodge" => ColorDodge,
    "color-burn" => ColorBurn,
    "hard-light" => HardLight,
    "soft-light" => SoftLight,
    "difference" => Difference,
    "exclusion" => Exclusion,
);

make_property!(
    Direction,
    default: Ltr,
    inherits_automatically: true,

    identifiers:
    "ltr" => Ltr,
    "rtl" => Rtl,
);

make_property!(
    Display,
    default: Inline,
    inherits_automatically: true,

    identifiers:
    "inline" => Inline,
    "block" => Block,
    "list-item" => ListItem,
    "run-in" => RunIn,
    "compact" => Compact,
    "marker" => Marker,
    "table" => Table,
    "inline-table" => InlineTable,
    "table-row-group" => TableRowGroup,
    "table-header-group" => TableHeaderGroup,
    "table-footer-group" => TableFooterGroup,
    "table-row" => TableRow,
    "table-column-group" => TableColumnGroup,
    "table-column" => TableColumn,
    "table-cell" => TableCell,
    "table-caption" => TableCaption,
    "none" => None,
);

make_property!(
    EnableBackground,
    default: Accumulate,
    inherits_automatically: false,

    identifiers:
    "accumulate" => Accumulate,
    "new" => New,
);

make_property!(
    Fill,
    default: PaintServer::parse("#000", ()).unwrap(),
    inherits_automatically: true,
    newtype_parse: PaintServer,
    parse_data_type: ()
);

make_property!(
    FillOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_from_str: UnitInterval
);

make_property!(
    FillRule,
    default: NonZero,
    inherits_automatically: true,

    identifiers:
    "nonzero" => NonZero,
    "evenodd" => EvenOdd,
);

make_property!(
    Filter,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    FloodColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
    inherits_automatically: true,
    newtype_parse: cssparser::Color,
    parse_data_type: ()
);

make_property!(
    FloodOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_from_str: UnitInterval
);

make_property!(
    FontFamily,
    default: "Times New Roman".to_string(),
    inherits_automatically: true,
    newtype_from_str: String
);

make_property!(
    FontSize,
    default: RsvgLength::parse("12.0", LengthDir::Both).unwrap(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

make_property!(
    FontStretch,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "wider" => Wider,
    "narrower" => Narrower,
    "ultra-condensed" => UltraCondensed,
    "extra-condensed" => ExtraCondensed,
    "condensed" => Condensed,
    "semi-condensed" => SemiCondensed,
    "semi-expanded" => SemiExpanded,
    "expanded" => Expanded,
    "extra-expanded" => ExtraExpanded,
    "ultra-expanded" => UltraExpanded,
);

make_property!(
    FontStyle,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "italic" => Italic,
    "oblique" => Oblique,
);

make_property!(
    FontVariant,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "small-caps" => SmallCaps,
);

make_property!(
    FontWeight,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "bold" => Bold,
    "bolder" => Bolder,
    "lighter" => Lighter,
    "100" => W100, // FIXME: we should use Weight(100),
    "200" => W200, // but we need a smarter macro for that
    "300" => W300,
    "400" => W400,
    "500" => W500,
    "600" => W600,
    "700" => W700,
    "800" => W800,
    "900" => W900,
);

make_property!(
    LetterSpacing,
    default: RsvgLength::default(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

make_property!(
    MarkerEnd,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    MarkerMid,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    MarkerStart,
    default: IRI::None,
    inherits_automatically: true,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    Mask,
    default: IRI::None,
    inherits_automatically: false,
    newtype_parse: IRI,
    parse_data_type: ()
);

make_property!(
    Opacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_from_str: UnitInterval
);

make_property!(
    Overflow,
    default: Visible,
    inherits_automatically: true,

    identifiers:
    "visible" => Visible,
    "hidden" => Hidden,
    "scroll" => Scroll,
    "auto" => Auto,
);

make_property!(
    ShapeRendering,
    default: Auto,
    inherits_automatically: true,

    identifiers:
    "auto" => Auto,
    "optimizeSpeed" => OptimizeSpeed,
    "geometricPrecision" => GeometricPrecision,
    "crispEdges" => CrispEdges,
);

make_property!(
    StopColor,
    default: cssparser::Color::RGBA(cssparser::RGBA::new(0, 0, 0, 0)),
    inherits_automatically: false,
    newtype_parse: cssparser::Color,
    parse_data_type: ()
);

make_property!(
    StopOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: false,
    newtype_from_str: UnitInterval
);

make_property!(
    Stroke,
    default: PaintServer::parse("#000", ()).unwrap(),
    inherits_automatically: true,
    newtype_parse: PaintServer,
    parse_data_type: ()
);

make_property!(
    StrokeDasharray,
    default: Dasharray::default(),
    inherits_automatically: true,
    newtype_parse: Dasharray,
    parse_data_type: ()
);

make_property!(
    StrokeDashoffset,
    default: RsvgLength::default(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

make_property!(
    StrokeLinecap,
    default: Butt,
    inherits_automatically: true,

    identifiers:
    "butt" => Butt,
    "round" => Round,
    "square" => Square,
);

make_property!(
    StrokeLinejoin,
    default: Miter,
    inherits_automatically: true,

    identifiers:
    "miter" => Miter,
    "round" => Round,
    "bevel" => Bevel,
);

make_property!(
    StrokeOpacity,
    default: UnitInterval(1.0),
    inherits_automatically: true,
    newtype_from_str: UnitInterval
);

make_property!(
    StrokeMiterlimit,
    default: 4f64,
    inherits_automatically: true,
    newtype_from_str: f64
);

make_property!(
    StrokeWidth,
    default: RsvgLength::parse("1.0", LengthDir::Both).unwrap(),
    inherits_automatically: true,
    newtype_parse: RsvgLength,
    parse_data_type: LengthDir
);

make_property!(
    TextAnchor,
    default: Start,
    inherits_automatically: true,

    identifiers:
    "start" => Start,
    "middle" => Middle,
    "end" => End,
);

make_property!(
    TextDecoration,
    inherits_automatically: true,

    fields:
    overline: bool, default: false,
    underline: bool, default: false,
    strike: bool, default: false,
);

impl Parse for TextDecoration {
    type Data = ();
    type Err = AttributeError;

    fn parse(s: &str, _: Self::Data) -> Result<TextDecoration, AttributeError> {
        Ok(TextDecoration {
            overline: s.contains("overline"),
            underline: s.contains("underline"),
            strike: s.contains("strike") || s.contains("line-through"),
        })
    }
}

make_property!(
    TextRendering,
    default: Auto,
    inherits_automatically: true,

    identifiers:
    "auto" => Auto,
    "optimizeSpeed" => OptimizeSpeed,
    "optimizeLegibility" => OptimizeLegibility,
    "geometricPrecision" => GeometricPrecision,
);

make_property!(
    UnicodeBidi,
    default: Normal,
    inherits_automatically: true,

    identifiers:
    "normal" => Normal,
    "embed" => Embed,
    "bidi-override" => Override,
);

make_property!(
    Visibility,
    default: Visible,
    inherits_automatically: true,

    identifiers:
    "visible" => Visible,
    "hidden" => Hidden,
    "collapse" => Collapse,
);

make_property!(
    WritingMode,
    default: LrTb,
    inherits_automatically: true,

    identifiers:
    "lr" => Lr,
    "lr-tb" => LrTb,
    "rl" => Rl,
    "rl-tb" => RlTb,
    "tb" => Tb,
    "tb-rl" => TbRl,
);

make_property!(
    XmlLang,
    default: "C".to_string(),
    inherits_automatically: true,
    newtype_from_str: String
);

make_property!(
    XmlSpace,
    default: Default,
    inherits_automatically: true,

    identifiers:
    "default" => Default,
    "preserve" => Preserve,
);

// C state API implemented in rust

#[no_mangle]
pub extern "C" fn rsvg_state_reconstruct(state: *mut RsvgState, raw_node: *const RsvgNode) {
    let state = from_c_mut(state);

    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    state.reconstruct(node);
}

#[no_mangle]
pub extern "C" fn rsvg_state_is_visible(state: *const RsvgState) -> glib_sys::gboolean {
    let state = from_c(state);

    state.is_visible().to_glib()
}

#[no_mangle]
pub extern "C" fn rsvg_state_parse_presentation_attributes(
    state: *mut RsvgState,
    pbag: *const PropertyBag,
) -> glib_sys::gboolean {
    let state = from_c_mut(state);

    let pbag = unsafe { &*pbag };

    match state.parse_presentation_attributes(pbag) {
        Ok(_) => true.to_glib(),
        Err(_) => false.to_glib(),
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_parse_conditional_processing_attributes(
    state: *mut RsvgState,
    pbag: *const PropertyBag,
) -> glib_sys::gboolean {
    let state = from_c_mut(state);

    let pbag = unsafe { &*pbag };

    match state.parse_conditional_processing_attributes(pbag) {
        Ok(_) => true.to_glib(),
        Err(_) => false.to_glib(),
    }
}

// Rust State API for consumption from C ----------------------------------------

pub fn from_c<'a>(state: *const RsvgState) -> &'a State {
    assert!(!state.is_null());

    unsafe { &*(state as *const State) }
}

pub fn from_c_mut<'a>(state: *mut RsvgState) -> &'a mut State {
    assert!(!state.is_null());

    unsafe { &mut *(state as *mut State) }
}

pub fn to_c(state: &State) -> *const RsvgState {
    state as *const State as *const RsvgState
}

pub fn to_c_mut(state: &mut State) -> *mut RsvgState {
    state as *mut State as *mut RsvgState
}

#[no_mangle]
pub extern "C" fn rsvg_state_new(parent: *mut RsvgState) -> *mut RsvgState {
    Box::into_raw(Box::new(State::new(parent))) as *mut RsvgState
}

#[no_mangle]
pub extern "C" fn rsvg_state_free(state: *mut RsvgState) {
    let state = from_c_mut(state);

    unsafe {
        Box::from_raw(state);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_parent(state: *const RsvgState) -> *mut RsvgState {
    let state = from_c(state);

    state.parent as *mut _
}

#[no_mangle]
pub extern "C" fn rsvg_state_parse_style_pair(
    state: *mut RsvgState,
    attr: Attribute,
    value: *const libc::c_char,
    important: glib_sys::gboolean,
    accept_shorthands: glib_sys::gboolean,
) -> glib_sys::gboolean {
    let state = from_c_mut(state);

    assert!(!value.is_null());

    let value = unsafe { utf8_cstr(value) };

    match state.parse_style_pair(
        attr,
        value,
        from_glib(important),
        from_glib(accept_shorthands),
    ) {
        Ok(_) => true.to_glib(),
        Err(_) => false.to_glib(),
    }
}

fn inherit<T>(inherit_fn: fn(bool, bool) -> bool, dst: &mut Option<T>, src: &Option<T>)
where
    T: Property + Clone,
{
    if inherit_fn(dst.is_some(), src.is_some()) {
        dst.clone_from(src);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_affine(state: *const RsvgState) -> cairo::Matrix {
    let state = from_c(state);

    state.affine
}

#[no_mangle]
pub extern "C" fn rsvg_state_set_affine(state: *mut RsvgState, affine: cairo::Matrix) {
    let state = from_c_mut(state);
    state.affine = affine;
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_current_color(state: *const RsvgState) -> u32 {
    let state = from_c(state);

    let current_color = state
        .color
        .as_ref()
        .map_or_else(|| Color::default().0, |c| c.0);

    rgba_to_argb(current_color)
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_comp_op(state: *const RsvgState) -> cairo::Operator {
    let state = from_c(state);
    cairo::Operator::from(state.comp_op.unwrap_or_default())
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_flood_color(state: *const RsvgState) -> u32 {
    let state = from_c(state);

    match state.flood_color {
        Some(FloodColor(cssparser::Color::RGBA(rgba))) => rgba_to_argb(rgba),
        // FIXME: fallback to current color if Color::inherit and current color is set
        _ => 0xff000000,
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_flood_opacity(state: *const RsvgState) -> u8 {
    let state = from_c(state);

    u8::from(
        state
            .flood_opacity
            .as_ref()
            .map_or_else(|| FloodOpacity::default().0, |o| o.0),
    )
}

// Keep in sync with rsvg-styles.h:RsvgEnableBackgroundType
#[allow(dead_code)]
#[repr(C)]
pub enum EnableBackgroundC {
    Accumulate,
    New,
}

impl From<EnableBackground> for EnableBackgroundC {
    fn from(e: EnableBackground) -> EnableBackgroundC {
        match e {
            EnableBackground::Accumulate => EnableBackgroundC::Accumulate,
            EnableBackground::New => EnableBackgroundC::New,
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_enable_background(state: *const RsvgState) -> EnableBackgroundC {
    let state = from_c(state);
    EnableBackgroundC::from(state.enable_background.unwrap_or_default())
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_clip_path(state: *const RsvgState) -> *mut libc::c_char {
    let state = from_c(state);

    match state.clip_path {
        Some(ClipPath(IRI::Resource(ref p))) => p.to_glib_full(),
        _ => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_filter(state: *const RsvgState) -> *mut libc::c_char {
    let state = from_c(state);

    match state.filter {
        Some(Filter(IRI::Resource(ref f))) => f.to_glib_full(),
        _ => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_mask(state: *const RsvgState) -> *mut libc::c_char {
    let state = from_c(state);

    match state.mask {
        Some(Mask(IRI::Resource(ref m))) => m.to_glib_full(),
        _ => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rsvg_state_get_opacity(state: *const RsvgState) -> u8 {
    let state = from_c(state);

    u8::from(
        state
            .opacity
            .as_ref()
            .map_or_else(|| FloodOpacity::default().0, |o| o.0),
    )
}

extern "C" {
    fn rsvg_lookup_apply_css_style(
        handle: *const RsvgHandle,
        target: *const libc::c_char,
        state: *mut RsvgState,
    ) -> glib_sys::gboolean;
}

#[no_mangle]
pub extern "C" fn rsvg_parse_style_attrs(
    handle: *const RsvgHandle,
    raw_node: *const RsvgNode,
    tag: *const libc::c_char,
    klazz: *const libc::c_char,
    id: *const libc::c_char,
    pbag: *const PropertyBag,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    let tag = unsafe { utf8_cstr(tag) };

    let klazz = unsafe { utf8_cstr_opt(klazz) };
    let id = unsafe { utf8_cstr_opt(id) };

    let pbag = unsafe { &*pbag };

    parse_style_attrs(handle, node, tag, klazz, id, pbag);
}

// Sets the node's state from the attributes in the pbag.  Also
// applies CSS rules in our limited way based on the node's
// tag/klazz/id.
fn parse_style_attrs(
    handle: *const RsvgHandle,
    node: &RsvgNode,
    tag: &str,
    klazz: Option<&str>,
    id: Option<&str>,
    pbag: &PropertyBag,
) {
    let state = node.get_state_mut();

    match state.parse_presentation_attributes(pbag) {
        Ok(_) => (),
        Err(_) => (),
        /* FIXME: we'll ignore errors here for now.  If we return, we expose
         * buggy handling of the enable-background property; we are not parsing it correctly.
         * This causes tests/fixtures/reftests/bugs/587721-text-transform.svg to fail
         * because it has enable-background="new 0 0 1179.75118 687.74173" in the toplevel svg
         * element.
         *        Err(e) => (),
         *        {
         *            node.set_error(e);
         *            return;
         *        } */
    }

    match state.parse_conditional_processing_attributes(pbag) {
        Ok(_) => (),
        Err(e) => {
            node.set_error(e);
            return;
        }
    }

    // Try to properly support all of the following, including inheritance:
    // *
    // #id
    // tag
    // tag#id
    // tag.class
    // tag.class#id
    //
    // This is basically a semi-compliant CSS2 selection engine

    unsafe {
        // *
        rsvg_lookup_apply_css_style(handle, "*".to_glib_none().0, to_c_mut(state));

        // tag
        rsvg_lookup_apply_css_style(handle, tag.to_glib_none().0, to_c_mut(state));

        if let Some(klazz) = klazz {
            for cls in klazz.split_whitespace() {
                let mut found = false;

                if !cls.is_empty() {
                    // tag.class#id
                    if let Some(id) = id {
                        let target = format!("{}.{}#{}", tag, cls, id);
                        found = found
                            || from_glib(rsvg_lookup_apply_css_style(
                                handle,
                                target.to_glib_none().0,
                                to_c_mut(state),
                            ));
                    }

                    // .class#id
                    if let Some(id) = id {
                        let target = format!(".{}#{}", cls, id);
                        found = found
                            || from_glib(rsvg_lookup_apply_css_style(
                                handle,
                                target.to_glib_none().0,
                                to_c_mut(state),
                            ));
                    }

                    // tag.class
                    let target = format!("{}.{}", tag, cls);
                    found = found
                        || from_glib(rsvg_lookup_apply_css_style(
                            handle,
                            target.to_glib_none().0,
                            to_c_mut(state),
                        ));

                    if !found {
                        // didn't find anything more specific, just apply the class style
                        let target = format!(".{}", cls);
                        rsvg_lookup_apply_css_style(
                            handle,
                            target.to_glib_none().0,
                            to_c_mut(state),
                        );
                    }
                }
            }
        }

        if let Some(id) = id {
            // id
            let target = format!("#{}", id);
            rsvg_lookup_apply_css_style(handle, target.to_glib_none().0, to_c_mut(state));

            // tag#id
            let target = format!("{}#{}", tag, id);
            rsvg_lookup_apply_css_style(handle, target.to_glib_none().0, to_c_mut(state));
        }

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Style => {
                    if let Err(e) = state.parse_style_declarations(value) {
                        node.set_error(e);
                        break;
                    }
                }

                Attribute::Transform => match cairo::Matrix::parse(value, ()) {
                    Ok(affine) => state.affine = cairo::Matrix::multiply(&affine, &state.affine),

                    Err(e) => {
                        node.set_error(NodeError::attribute_error(Attribute::Transform, e));
                        break;
                    }
                },

                _ => (),
            }
        }
    }
}
