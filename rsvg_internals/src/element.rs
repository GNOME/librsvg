//! SVG Elements.
//!
//! The [`create_element`] function takes an XML element name, and
//! creates an [`Element`] for it.
//!
//! [`create_element`]: fn.create_element.html

use locale_config::{LanguageRange, Locale};
use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::bbox::BoundingBox;
use crate::cond::{RequiredExtensions, RequiredFeatures, SystemLanguage};
use crate::css::{Declaration, Origin};
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::filter::Filter;
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
    offset::FeOffset,
    tile::FeTile,
    turbulence::FeTurbulence,
    FilterEffect,
};
use crate::gradient::{LinearGradient, RadialGradient, Stop};
use crate::image::Image;
use crate::marker::Marker;
use crate::node::*;
use crate::parsers::Parse;
use crate::pattern::Pattern;
use crate::properties::{ComputedValues, SpecifiedValues};
use crate::property_bag::PropertyBag;
use crate::shapes::{Circle, Ellipse, Line, Path, Polygon, Polyline, Rect};
use crate::structure::{ClipPath, Group, Link, Mask, NonRendering, Svg, Switch, Symbol, Use};
use crate::style::Style;
use crate::text::{TRef, TSpan, Text};
use crate::transform::Transform;

// After creating/parsing a Element, it will be in a success or an error state.
// We represent this with a Result, aliased as a ElementResult.  There is no
// extra information for the Ok case; all the interesting stuff is in the
// Err case.
//
// https://www.w3.org/TR/SVG/implnote.html#ErrorProcessing
//
// When an element has an error during parsing, the SVG spec calls the element
// to be "in error".  We skip rendering of elements that are in error.
//
// When we parse an element's attributes, we stop as soon as we
// encounter the first error:  a parse error, or an invalid value,
// etc.  No further attributes will be processed, although note that
// the order in which an element's attributes are processed is not
// defined.
//
// Alternatively, we could try to parse/validate all the attributes
// that come in an element and build up a Vec<ElementError>.  However, we
// don't do this now.  Doing that may be more useful for an SVG
// validator, not a renderer like librsvg is.
pub type ElementResult = Result<(), ElementError>;

/// The basic trait that all elements must implement
pub trait ElementTrait {
    /// Sets per-element attributes from the `pbag`
    ///
    /// Each element is supposed to iterate the `pbag`, and parse any attributes it needs.
    fn set_atts(&mut self, _pbag: &PropertyBag<'_>) -> ElementResult {
        Ok(())
    }

    /// Sets any special-cased properties that the element may have, that are different
    /// from defaults in the element's `SpecifiedValues`.
    fn set_overridden_properties(&self, _values: &mut SpecifiedValues) {}

    fn draw(
        &self,
        _node: &Node,
        _acquired_nodes: &mut AcquiredNodes,
        _cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        _clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        // by default elements don't draw themselves
        Ok(draw_ctx.empty_bbox())
    }
}

pub struct ElementInner<T: ElementTrait> {
    element_name: QualName,
    id: Option<String>,    // id attribute from XML element
    class: Option<String>, // class attribute from XML element
    specified_values: SpecifiedValues,
    important_styles: HashSet<QualName>,
    result: ElementResult,
    transform: Transform,
    values: ComputedValues,
    cond: bool,
    style_attr: String,
    pub element_impl: T,
}

impl<T: ElementTrait> ElementInner<T> {
    fn element_name(&self) -> &QualName {
        &self.element_name
    }

    fn get_id(&self) -> Option<&str> {
        self.id.as_ref().map(String::as_str)
    }

    fn get_class(&self) -> Option<&str> {
        self.class.as_ref().map(String::as_str)
    }

    fn get_specified_values(&self) -> &SpecifiedValues {
        &self.specified_values
    }

    fn get_computed_values(&self) -> &ComputedValues {
        &self.values
    }

    fn set_computed_values(&mut self, values: &ComputedValues) {
        self.values = values.clone();
    }

    fn get_cond(&self) -> bool {
        self.cond
    }

    fn get_transform(&self) -> Transform {
        self.transform
    }

    fn save_style_attribute(&mut self, pbag: &PropertyBag<'_>) {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "style") => self.style_attr.push_str(value),
                _ => (),
            }
        }
    }

    fn set_transform_attribute(&mut self, pbag: &PropertyBag<'_>) -> Result<(), ElementError> {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "transform") => {
                    return Transform::parse_str(value)
                        .attribute(attr)
                        .and_then(|affine| {
                            self.transform = affine;
                            Ok(())
                        });
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn set_conditional_processing_attributes(
        &mut self,
        pbag: &PropertyBag<'_>,
    ) -> Result<(), ElementError> {
        let mut cond = self.cond;
        let locale = locale_from_environment();

        for (attr, value) in pbag.iter() {
            let mut parse = || -> Result<_, ValueErrorKind> {
                match attr.expanded() {
                    expanded_name!("", "requiredExtensions") if cond => {
                        cond = RequiredExtensions::from_attribute(value)
                            .map(|RequiredExtensions(res)| res)?;
                    }

                    expanded_name!("", "requiredFeatures") if cond => {
                        cond = RequiredFeatures::from_attribute(value)
                            .map(|RequiredFeatures(res)| res)?;
                    }

                    expanded_name!("", "systemLanguage") if cond => {
                        cond = SystemLanguage::from_attribute(value, &locale)
                            .map(|SystemLanguage(res)| res)?;
                    }

                    _ => {}
                }

                Ok(cond)
            };

            parse().map(|c| self.cond = c).attribute(attr)?;
        }

        Ok(())
    }

    /// Hands the pbag to the node's state, to apply the presentation attributes
    fn set_presentation_attributes(&mut self, pbag: &PropertyBag<'_>) -> Result<(), ElementError> {
        match self.specified_values.parse_presentation_attributes(pbag) {
            Ok(_) => Ok(()),
            Err(e) => {
                // FIXME: we'll ignore errors here for now.
                //
                // If we set the node to be in error, we expose buggy handling of the
                // enable-background property; we are not parsing it correctly. This
                // causes tests/fixtures/reftests/bugs/587721-text-transform.svg to fail
                // because it has enable-background="new 0 0 1179.75118 687.74173" in the
                // toplevel svg element.
                //
                //   self.set_error(e);
                //   return;

                rsvg_log!("(attribute error: {})", e);
                Ok(())
            }
        }
    }

    fn set_element_specific_attributes(
        &mut self,
        pbag: &PropertyBag<'_>,
    ) -> Result<(), ElementError> {
        self.element_impl.set_atts(pbag)
    }

    fn set_overridden_properties(&mut self) {
        self.element_impl
            .set_overridden_properties(&mut self.specified_values);
    }

    // Applies a style declaration to the node's specified_values
    /*
        fn apply_style_declaration(&mut self, declaration: &Declaration, origin: Origin) {
            self.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut self.important_styles,
            );
        }
    */

    /// Applies CSS styles from the saved value of the "style" attribute
    fn set_style_attribute(&mut self) {
        if !self.style_attr.is_empty() {
            if let Err(e) = self.specified_values.parse_style_declarations(
                self.style_attr.as_str(),
                Origin::Author,
                &mut self.important_styles,
            ) {
                self.set_error(e);
            }

            self.style_attr.clear();
            self.style_attr.shrink_to_fit();
        }
    }

    fn set_error(&mut self, error: ElementError) {
        rsvg_log!("setting node {} in error: {}", self, error);
        self.result = Err(error);
    }

    fn is_in_error(&self) -> bool {
        self.result.is_err()
    }

    fn as_element_trait(&self) -> &dyn ElementTrait {
        &self.element_impl as &dyn ElementTrait
    }
}

impl<T: ElementTrait> fmt::Display for ElementInner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.element_name().local)?;
        write!(f, " id={}", self.get_id().unwrap_or("None"))?;
        Ok(())
    }
}

/// Contents of an element node in the DOM
/// This enum uses `Box<ElementInner>` in order to make each `Element`
/// the size of a pointer.

pub enum Element {
    Circle(Box<ElementInner<Circle>>),
    ClipPath(Box<ElementInner<ClipPath>>),
    Ellipse(Box<ElementInner<Ellipse>>),
    Filter(Box<ElementInner<Filter>>),
    Group(Box<ElementInner<Group>>),
    Image(Box<ElementInner<Image>>),
    Line(Box<ElementInner<Line>>),
    LinearGradient(Box<ElementInner<LinearGradient>>),
    Link(Box<ElementInner<Link>>),
    Marker(Box<ElementInner<Marker>>),
    Mask(Box<ElementInner<Mask>>),
    NonRendering(Box<ElementInner<NonRendering>>),
    Path(Box<ElementInner<Path>>),
    Pattern(Box<ElementInner<Pattern>>),
    Polygon(Box<ElementInner<Polygon>>),
    Polyline(Box<ElementInner<Polyline>>),
    RadialGradient(Box<ElementInner<RadialGradient>>),
    Rect(Box<ElementInner<Rect>>),
    Stop(Box<ElementInner<Stop>>),
    Style(Box<ElementInner<Style>>),
    Svg(Box<ElementInner<Svg>>),
    Switch(Box<ElementInner<Switch>>),
    Symbol(Box<ElementInner<Symbol>>),
    Text(Box<ElementInner<Text>>),
    TRef(Box<ElementInner<TRef>>),
    TSpan(Box<ElementInner<TSpan>>),
    Use(Box<ElementInner<Use>>),

    // Filter primitives, these start with "Fe" as element names are e.g. "feBlend"
    FeBlend(Box<ElementInner<FeBlend>>),
    FeColorMatrix(Box<ElementInner<FeColorMatrix>>),
    FeComponentTransfer(Box<ElementInner<FeComponentTransfer>>),
    FeComposite(Box<ElementInner<FeComposite>>),
    FeConvolveMatrix(Box<ElementInner<FeConvolveMatrix>>),
    FeDiffuseLighting(Box<ElementInner<FeDiffuseLighting>>),
    FeDisplacementMap(Box<ElementInner<FeDisplacementMap>>),
    FeDistantLight(Box<ElementInner<FeDistantLight>>),
    FeFlood(Box<ElementInner<FeFlood>>),
    FeFuncA(Box<ElementInner<FeFuncA>>),
    FeFuncB(Box<ElementInner<FeFuncB>>),
    FeFuncG(Box<ElementInner<FeFuncG>>),
    FeFuncR(Box<ElementInner<FeFuncR>>),
    FeGaussianBlur(Box<ElementInner<FeGaussianBlur>>),
    FeImage(Box<ElementInner<FeImage>>),
    FeMerge(Box<ElementInner<FeMerge>>),
    FeMergeNode(Box<ElementInner<FeMergeNode>>),
    FeMorphology(Box<ElementInner<FeMorphology>>),
    FeOffset(Box<ElementInner<FeOffset>>),
    FePointLight(Box<ElementInner<FePointLight>>),
    FeSpecularLighting(Box<ElementInner<FeSpecularLighting>>),
    FeSpotLight(Box<ElementInner<FeSpotLight>>),
    FeTile(Box<ElementInner<FeTile>>),
    FeTurbulence(Box<ElementInner<FeTurbulence>>),
}

macro_rules! call_inner {
    ($element:ident, $method:ident) => {
        match $element {
            Element::Circle(i) => i.$method(),
            Element::ClipPath(i) => i.$method(),
            Element::Ellipse(i) => i.$method(),
            Element::Filter(i) => i.$method(),
            Element::Group(i) => i.$method(),
            Element::Image(i) => i.$method(),
            Element::Line(i) => i.$method(),
            Element::LinearGradient(i) => i.$method(),
            Element::Link(i) => i.$method(),
            Element::Marker(i) => i.$method(),
            Element::Mask(i) => i.$method(),
            Element::NonRendering(i) => i.$method(),
            Element::Path(i) => i.$method(),
            Element::Pattern(i) => i.$method(),
            Element::Polygon(i) => i.$method(),
            Element::Polyline(i) => i.$method(),
            Element::RadialGradient(i) => i.$method(),
            Element::Rect(i) => i.$method(),
            Element::Stop(i) => i.$method(),
            Element::Style(i) => i.$method(),
            Element::Svg(i) => i.$method(),
            Element::Switch(i) => i.$method(),
            Element::Symbol(i) => i.$method(),
            Element::Text(i) => i.$method(),
            Element::TRef(i) => i.$method(),
            Element::TSpan(i) => i.$method(),
            Element::Use(i) => i.$method(),
            Element::FeBlend(i) => i.$method(),
            Element::FeColorMatrix(i) => i.$method(),
            Element::FeComponentTransfer(i) => i.$method(),
            Element::FeComposite(i) => i.$method(),
            Element::FeConvolveMatrix(i) => i.$method(),
            Element::FeDiffuseLighting(i) => i.$method(),
            Element::FeDisplacementMap(i) => i.$method(),
            Element::FeDistantLight(i) => i.$method(),
            Element::FeFlood(i) => i.$method(),
            Element::FeFuncA(i) => i.$method(),
            Element::FeFuncB(i) => i.$method(),
            Element::FeFuncG(i) => i.$method(),
            Element::FeFuncR(i) => i.$method(),
            Element::FeGaussianBlur(i) => i.$method(),
            Element::FeImage(i) => i.$method(),
            Element::FeMerge(i) => i.$method(),
            Element::FeMergeNode(i) => i.$method(),
            Element::FeMorphology(i) => i.$method(),
            Element::FeOffset(i) => i.$method(),
            Element::FePointLight(i) => i.$method(),
            Element::FeSpecularLighting(i) => i.$method(),
            Element::FeSpotLight(i) => i.$method(),
            Element::FeTile(i) => i.$method(),
            Element::FeTurbulence(i) => i.$method(),
        }
    };

    ($element:ident, $method:ident, $arg:ident) => {
        match $element {
            Element::Circle(i) => i.$method($arg),
            Element::ClipPath(i) => i.$method($arg),
            Element::Ellipse(i) => i.$method($arg),
            Element::Filter(i) => i.$method($arg),
            Element::Group(i) => i.$method($arg),
            Element::Image(i) => i.$method($arg),
            Element::Line(i) => i.$method($arg),
            Element::LinearGradient(i) => i.$method($arg),
            Element::Link(i) => i.$method($arg),
            Element::Marker(i) => i.$method($arg),
            Element::Mask(i) => i.$method($arg),
            Element::NonRendering(i) => i.$method($arg),
            Element::Path(i) => i.$method($arg),
            Element::Pattern(i) => i.$method($arg),
            Element::Polygon(i) => i.$method($arg),
            Element::Polyline(i) => i.$method($arg),
            Element::RadialGradient(i) => i.$method($arg),
            Element::Rect(i) => i.$method($arg),
            Element::Stop(i) => i.$method($arg),
            Element::Style(i) => i.$method($arg),
            Element::Svg(i) => i.$method($arg),
            Element::Switch(i) => i.$method($arg),
            Element::Symbol(i) => i.$method($arg),
            Element::Text(i) => i.$method($arg),
            Element::TRef(i) => i.$method($arg),
            Element::TSpan(i) => i.$method($arg),
            Element::Use(i) => i.$method($arg),
            Element::FeBlend(i) => i.$method($arg),
            Element::FeColorMatrix(i) => i.$method($arg),
            Element::FeComponentTransfer(i) => i.$method($arg),
            Element::FeComposite(i) => i.$method($arg),
            Element::FeConvolveMatrix(i) => i.$method($arg),
            Element::FeDiffuseLighting(i) => i.$method($arg),
            Element::FeDisplacementMap(i) => i.$method($arg),
            Element::FeDistantLight(i) => i.$method($arg),
            Element::FeFlood(i) => i.$method($arg),
            Element::FeFuncA(i) => i.$method($arg),
            Element::FeFuncB(i) => i.$method($arg),
            Element::FeFuncG(i) => i.$method($arg),
            Element::FeFuncR(i) => i.$method($arg),
            Element::FeGaussianBlur(i) => i.$method($arg),
            Element::FeImage(i) => i.$method($arg),
            Element::FeMerge(i) => i.$method($arg),
            Element::FeMergeNode(i) => i.$method($arg),
            Element::FeMorphology(i) => i.$method($arg),
            Element::FeOffset(i) => i.$method($arg),
            Element::FePointLight(i) => i.$method($arg),
            Element::FeSpecularLighting(i) => i.$method($arg),
            Element::FeSpotLight(i) => i.$method($arg),
            Element::FeTile(i) => i.$method($arg),
            Element::FeTurbulence(i) => i.$method($arg),
        }
    };
}

// FIXME: find better name
#[macro_export]
macro_rules! get_element_impl {
    ($element:expr, $element_type:ident) => {
        match $element {
            Element::$element_type(ref e) => &e.element_impl,
            _ => unreachable!(),
        }
    };
}

impl Element {
    pub fn element_name(&self) -> &QualName {
        call_inner!(self, element_name)
    }

    pub fn get_id(&self) -> Option<&str> {
        call_inner!(self, get_id)
    }

    pub fn get_class(&self) -> Option<&str> {
        call_inner!(self, get_class)
    }

    pub fn get_specified_values(&self) -> &SpecifiedValues {
        call_inner!(self, get_specified_values)
    }

    pub fn get_computed_values(&self) -> &ComputedValues {
        call_inner!(self, get_computed_values)
    }

    pub fn set_computed_values(&mut self, values: &ComputedValues) {
        call_inner!(self, set_computed_values, values);
    }

    pub fn get_cond(&self) -> bool {
        call_inner!(self, get_cond)
    }

    pub fn get_transform(&self) -> Transform {
        call_inner!(self, get_transform)
    }

    fn save_style_attribute(&mut self, pbag: &PropertyBag<'_>) {
        call_inner!(self, save_style_attribute, pbag);
    }

    fn set_transform_attribute(&mut self, pbag: &PropertyBag<'_>) -> Result<(), ElementError> {
        call_inner!(self, set_transform_attribute, pbag)
    }

    fn set_conditional_processing_attributes(
        &mut self,
        pbag: &PropertyBag<'_>,
    ) -> Result<(), ElementError> {
        call_inner!(self, set_conditional_processing_attributes, pbag)
    }

    fn set_presentation_attributes(&mut self, pbag: &PropertyBag<'_>) -> Result<(), ElementError> {
        call_inner!(self, set_presentation_attributes, pbag)
    }

    fn set_element_specific_attributes(
        &mut self,
        pbag: &PropertyBag<'_>,
    ) -> Result<(), ElementError> {
        call_inner!(self, set_element_specific_attributes, pbag)
    }

    fn set_overridden_properties(&mut self) {
        call_inner!(self, set_overridden_properties)
    }

    // Applies a style declaration to the node's specified_values
    // FIXME: done here inline because I do not know how to generalize the
    // call_inner macro for n args
    pub fn apply_style_declaration(&mut self, declaration: &Declaration, origin: Origin) {
        match self {
            Element::Circle(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::ClipPath(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Ellipse(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Filter(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Group(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Image(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Line(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::LinearGradient(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Link(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Marker(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Mask(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::NonRendering(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Path(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Pattern(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Polygon(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Polyline(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::RadialGradient(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Rect(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Stop(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Style(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Svg(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Switch(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Symbol(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Text(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::TRef(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::TSpan(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::Use(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeBlend(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeColorMatrix(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeComponentTransfer(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeComposite(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeConvolveMatrix(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeDiffuseLighting(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeDisplacementMap(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeDistantLight(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeFlood(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeFuncA(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeFuncB(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeFuncG(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeFuncR(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeGaussianBlur(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeImage(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeMerge(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeMergeNode(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeMorphology(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeOffset(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FePointLight(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeSpecularLighting(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeSpotLight(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeTile(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
            Element::FeTurbulence(i) => i.specified_values.set_property_from_declaration(
                declaration,
                origin,
                &mut i.important_styles,
            ),
        }
    }

    /// Applies CSS styles from the saved value of the "style" attribute
    pub fn set_style_attribute(&mut self) {
        call_inner!(self, set_style_attribute);
    }

    fn set_error(&mut self, error: ElementError) {
        call_inner!(self, set_error, error);
    }

    pub fn is_in_error(&self) -> bool {
        call_inner!(self, is_in_error)
    }

    // FIXME: done here inline because I do not know how to generalize
    // call_inner macro for n args. If we do that we can remove as_element_trait
    pub fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        if !self.is_in_error() {
            draw_ctx.with_saved_transform(Some(self.get_transform()), &mut |dc| {
                call_inner!(self, as_element_trait).draw(
                    node,
                    acquired_nodes,
                    cascaded,
                    dc,
                    clipping,
                )
            })
        } else {
            rsvg_log!("(not rendering element {} because it is in error)", self);

            // maybe we should actually return a RenderingError::ElementIsInError here?
            Ok(draw_ctx.empty_bbox())
        }
    }

    pub fn as_filter_effect(&self) -> Option<&dyn FilterEffect> {
        match self {
            Element::FeBlend(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeColorMatrix(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeComponentTransfer(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeComposite(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeConvolveMatrix(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeDiffuseLighting(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeDisplacementMap(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeFlood(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeGaussianBlur(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeImage(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeMerge(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeMorphology(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeOffset(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeSpecularLighting(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeTile(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            Element::FeTurbulence(ref fe) => Some(&fe.element_impl as &dyn FilterEffect),
            _ => None,
        }
    }

    /// Returns whether an element of a particular type is only accessed by reference
    // from other elements' attributes.  The element could in turn cause other nodes
    // to get referenced, potentially causing reference cycles.
    pub fn is_accessed_by_reference(&self) -> bool {
        matches!(
            self,
            Element::ClipPath(_) |
            Element::Filter(_) |
            Element::LinearGradient(_) |
            Element::Marker(_) |
            Element::Mask(_) |
            Element::Pattern(_) |
            Element::RadialGradient(_)
        )
    }
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        call_inner!(self, fmt, f)
    }
}

macro_rules! e {
    ($name:ident, $element_type:ident) => {
        pub fn $name(element_name: &QualName, id: Option<&str>, class: Option<&str>) -> Element {
            Element::$element_type(Box::new(ElementInner {
                element_name: element_name.clone(),
                id: id.map(str::to_string),
                class: class.map(str::to_string),
                specified_values: Default::default(),
                important_styles: Default::default(),
                transform: Default::default(),
                result: Ok(()),
                values: ComputedValues::default(),
                cond: true,
                style_attr: String::new(),
                element_impl: <$element_type>::default(),
            }))
        }
    };
}

#[rustfmt::skip]
mod creators {
    use super::*;

    e!(create_circle,                   Circle);
    e!(create_clip_path,                ClipPath);
    e!(create_defs,                     NonRendering);
    e!(create_ellipse,                  Ellipse);
    e!(create_fe_blend,                 FeBlend);
    e!(create_fe_color_matrix,          FeColorMatrix);
    e!(create_fe_component_transfer,    FeComponentTransfer);
    e!(create_fe_func_a,                FeFuncA);
    e!(create_fe_func_b,                FeFuncB);
    e!(create_fe_func_g,                FeFuncG);
    e!(create_fe_func_r,                FeFuncR);
    e!(create_fe_composite,             FeComposite);
    e!(create_fe_convolve_matrix,       FeConvolveMatrix);
    e!(create_fe_diffuse_lighting,      FeDiffuseLighting);
    e!(create_fe_distant_light,         FeDistantLight);
    e!(create_fe_displacement_map,      FeDisplacementMap);
    e!(create_fe_flood,                 FeFlood);
    e!(create_fe_gaussian_blur,         FeGaussianBlur);
    e!(create_fe_image,                 FeImage);
    e!(create_fe_merge,                 FeMerge);
    e!(create_fe_merge_node,            FeMergeNode);
    e!(create_fe_morphology,            FeMorphology);
    e!(create_fe_offset,                FeOffset);
    e!(create_fe_point_light,           FePointLight);
    e!(create_fe_specular_lighting,     FeSpecularLighting);
    e!(create_fe_spot_light,            FeSpotLight);
    e!(create_fe_tile,                  FeTile);
    e!(create_fe_turbulence,            FeTurbulence);
    e!(create_filter,                   Filter);
    e!(create_group,                    Group);
    e!(create_image,                    Image);
    e!(create_line,                     Line);
    e!(create_linear_gradient,          LinearGradient);
    e!(create_link,                     Link);
    e!(create_marker,                   Marker);
    e!(create_mask,                     Mask);
    e!(create_non_rendering,            NonRendering);
    e!(create_path,                     Path);
    e!(create_pattern,                  Pattern);
    e!(create_polygon,                  Polygon);
    e!(create_polyline,                 Polyline);
    e!(create_radial_gradient,          RadialGradient);
    e!(create_rect,                     Rect);
    e!(create_stop,                     Stop);
    e!(create_style,                    Style);
    e!(create_svg,                      Svg);
    e!(create_switch,                   Switch);
    e!(create_symbol,                   Symbol);
    e!(create_text,                     Text);
    e!(create_tref,                     TRef);
    e!(create_tspan,                    TSpan);
    e!(create_use,                      Use);

    /* Hack to make multiImage sort-of work
     *
     * disabled for now, as markup5ever doesn't have local names for
     * multiImage, subImage, subImageRef.  Maybe we can just... create them ourselves?
     *
     * Is multiImage even in SVG2?
     */
    /*
    e!(create_multi_image,              Switch);
    e!(create_sub_image,                Group);
    e!(create_sub_image_ref,            Image);
    */
}

use creators::*;

type ElementCreateFn =
    fn(element_name: &QualName, id: Option<&str>, class: Option<&str>) -> Element;

#[derive(Copy, Clone, PartialEq)]
enum ElementCreateFlags {
    Default,
    IgnoreClass,
}

// Lines in comments are elements that we don't support.
#[rustfmt::skip]
static ELEMENT_CREATORS: Lazy<HashMap<&'static str, (ElementCreateFn, ElementCreateFlags)>> = Lazy::new(|| {
    use ElementCreateFlags::*;

    let creators_table: Vec<(&str, ElementCreateFn, ElementCreateFlags)> = vec![
        // name, supports_class, create_fn
        ("a",                   create_link,                  Default),
        /* ("altGlyph",         ), */
        /* ("altGlyphDef",      ), */
        /* ("altGlyphItem",     ), */
        /* ("animate",          ), */
        /* ("animateColor",     ), */
        /* ("animateMotion",    ), */
        /* ("animateTransform", ), */
        ("circle",              create_circle,                Default),
        ("clipPath",            create_clip_path,             Default),
        /* ("color-profile",    ), */
        /* ("cursor",           ), */
        ("defs",                create_defs,                  Default),
        /* ("desc",             ), */
        ("ellipse",             create_ellipse,               Default),
        ("feBlend",             create_fe_blend,              Default),
        ("feColorMatrix",       create_fe_color_matrix,       Default),
        ("feComponentTransfer", create_fe_component_transfer, Default),
        ("feComposite",         create_fe_composite,          Default),
        ("feConvolveMatrix",    create_fe_convolve_matrix,    Default),
        ("feDiffuseLighting",   create_fe_diffuse_lighting,   Default),
        ("feDisplacementMap",   create_fe_displacement_map,   Default),
        ("feDistantLight",      create_fe_distant_light,      IgnoreClass),
        ("feFuncA",             create_fe_func_a,             IgnoreClass),
        ("feFuncB",             create_fe_func_b,             IgnoreClass),
        ("feFuncG",             create_fe_func_g,             IgnoreClass),
        ("feFuncR",             create_fe_func_r,             IgnoreClass),
        ("feFlood",             create_fe_flood,              Default),
        ("feGaussianBlur",      create_fe_gaussian_blur,      Default),
        ("feImage",             create_fe_image,              Default),
        ("feMerge",             create_fe_merge,              Default),
        ("feMergeNode",         create_fe_merge_node,         IgnoreClass),
        ("feMorphology",        create_fe_morphology,         Default),
        ("feOffset",            create_fe_offset,             Default),
        ("fePointLight",        create_fe_point_light,        IgnoreClass),
        ("feSpecularLighting",  create_fe_specular_lighting,  Default),
        ("feSpotLight",         create_fe_spot_light,         IgnoreClass),
        ("feTile",              create_fe_tile,               Default),
        ("feTurbulence",        create_fe_turbulence,         Default),
        ("filter",              create_filter,                Default),
        /* ("font",             ), */
        /* ("font-face",        ), */
        /* ("font-face-format", ), */
        /* ("font-face-name",   ), */
        /* ("font-face-src",    ), */
        /* ("font-face-uri",    ), */
        /* ("foreignObject",    ), */
        ("g",                   create_group,                 Default),
        /* ("glyph",            ), */
        /* ("glyphRef",         ), */
        /* ("hkern",            ), */
        ("image",               create_image,                 Default),
        ("line",                create_line,                  Default),
        ("linearGradient",      create_linear_gradient,       Default),
        ("marker",              create_marker,                Default),
        ("mask",                create_mask,                  Default),
        /* ("metadata",         ), */
        /* ("missing-glyph",    ), */
        /* ("mpath",            ), */
        /* ("multiImage",       ), */
        ("path",                create_path,                  Default),
        ("pattern",             create_pattern,               Default),
        ("polygon",             create_polygon,               Default),
        ("polyline",            create_polyline,              Default),
        ("radialGradient",      create_radial_gradient,       Default),
        ("rect",                create_rect,                  Default),
        /* ("script",           ), */
        /* ("set",              ), */
        ("stop",                create_stop,                  Default),
        ("style",               create_style,                 IgnoreClass),
        /* ("subImage",         ), */
        /* ("subImageRef",      ), */
        ("svg",                 create_svg,                   Default),
        ("switch",              create_switch,                Default),
        ("symbol",              create_symbol,                Default),
        ("text",                create_text,                  Default),
        /* ("textPath",         ), */
        /* ("title",            ), */
        ("tref",                create_tref,                  Default),
        ("tspan",               create_tspan,                 Default),
        ("use",                 create_use,                   Default),
        /* ("view",             ), */
        /* ("vkern",            ), */
    ];

    creators_table.into_iter().map(|(n, c, f)| (n, (c, f))).collect()
});

/// Takes an XML element name and a list of attribute/value pairs and creates an [`Element`].
///
/// This operation does not fail.  Unknown element names simply produce a [`NonRendering`]
/// element.
///
/// [`Element`]: type.Element.html
/// [`NonRendering`]: ../structure/struct.NonRendering.html
pub fn create_element(name: &QualName, pbag: &PropertyBag) -> Element {
    let mut id = None;
    let mut class = None;

    for (attr, value) in pbag.iter() {
        match attr.expanded() {
            expanded_name!("", "id") => id = Some(value),
            expanded_name!("", "class") => class = Some(value),
            _ => (),
        }
    }

    let (create_fn, flags) = if name.ns == ns!(svg) {
        match ELEMENT_CREATORS.get(name.local.as_ref()) {
            // hack in the SVG namespace for supported element names
            Some(&(create_fn, flags)) => (create_fn, flags),

            // Whenever we encounter a element name we don't understand, represent it as a
            // non-rendering element.  This is like a group, but it doesn't do any rendering
            // of children.  The effect is that we will ignore all children of unknown elements.
            None => (
                create_non_rendering as ElementCreateFn,
                ElementCreateFlags::Default,
            ),
        }
    } else {
        (
            create_non_rendering as ElementCreateFn,
            ElementCreateFlags::Default,
        )
    };

    if flags == ElementCreateFlags::IgnoreClass {
        class = None;
    };

    //    sizes::print_sizes();

    let mut element = create_fn(name, id, class);

    element.save_style_attribute(pbag);

    if let Err(e) = element
        .set_transform_attribute(pbag)
        .and_then(|_| element.set_conditional_processing_attributes(pbag))
        .and_then(|_| element.set_element_specific_attributes(pbag))
        .and_then(|_| element.set_presentation_attributes(pbag))
    {
        element.set_error(e);
    }

    element.set_overridden_properties();

    element
}

/// Gets the user's preferred locale from the environment and
/// translates it to a `Locale` with `LanguageRange` fallbacks.
///
/// The `Locale::current()` call only contemplates a single language,
/// but glib is smarter, and `g_get_langauge_names()` can provide
/// fallbacks, for example, when LC_MESSAGES="en_US.UTF-8:de" (USA
/// English and German).  This function converts the output of
/// `g_get_language_names()` into a `Locale` with appropriate
/// fallbacks.
fn locale_from_environment() -> Locale {
    let mut locale = Locale::invariant();

    for name in glib::get_language_names() {
        if let Ok(range) = LanguageRange::from_unix(&name) {
            locale.add(&range);
        }
    }

    locale
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
        print_size!(Element);
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
