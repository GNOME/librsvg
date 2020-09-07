//! SVG Elements.

use locale_config::{LanguageRange, Locale};
use markup5ever::{expanded_name, local_name, namespace_url, ns, QualName};
use matches::matches;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Deref;

use crate::attributes::Attributes;
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
    lighting::{FeDiffuseLighting, FeDistantLight, FePointLight, FeSpecularLighting, FeSpotLight},
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

pub trait SetAttributes {
    /// Sets per-element attributes.
    ///
    /// Each element is supposed to iterate the `attributes`, and parse any ones it needs.
    fn set_attributes(&mut self, _attributes: &Attributes) -> ElementResult {
        Ok(())
    }
}

pub trait Draw {
    /// Draw an element
    ///
    /// Each element is supposed to draw itself as needed.
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

pub struct ElementInner<T: SetAttributes + Draw> {
    element_name: QualName,
    id: Option<String>,    // id attribute from XML element
    class: Option<String>, // class attribute from XML element
    attributes: Attributes,
    specified_values: SpecifiedValues,
    important_styles: HashSet<QualName>,
    result: ElementResult,
    transform: Transform,
    values: ComputedValues,
    cond: bool,
    style_attr: String,
    pub element_impl: T,
}

impl<T: SetAttributes + Draw> ElementInner<T> {
    fn new(
        element_name: QualName,
        id: Option<String>,
        class: Option<String>,
        attributes: Attributes,
        result: Result<(), ElementError>,
        element_impl: T,
    ) -> ElementInner<T> {
        let mut e = Self {
            element_name,
            id,
            class,
            attributes,
            specified_values: Default::default(),
            important_styles: Default::default(),
            result,
            transform: Default::default(),
            values: Default::default(),
            cond: true,
            style_attr: String::new(),
            element_impl,
        };

        e.save_style_attribute();

        let mut set_attributes = || -> Result<(), ElementError> {
            e.set_transform_attribute()?;
            e.set_conditional_processing_attributes()?;
            e.set_presentation_attributes()?;
            Ok(())
        };

        if let Err(error) = set_attributes() {
            e.set_error(error);
        }

        e
    }

    fn element_name(&self) -> &QualName {
        &self.element_name
    }

    fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn get_id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    fn get_class(&self) -> Option<&str> {
        self.class.as_deref()
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

    fn save_style_attribute(&mut self) {
        self.style_attr.push_str(
            self.attributes
                .iter()
                .find(|(attr, _)| attr.expanded() == expanded_name!("", "style"))
                .map(|(_, value)| value)
                .unwrap_or(""),
        );
    }

    fn set_transform_attribute(&mut self) -> Result<(), ElementError> {
        self.transform = self
            .attributes
            .iter()
            .find(|(attr, _)| attr.expanded() == expanded_name!("", "transform"))
            .map(|(attr, value)| Transform::parse_str(value).attribute(attr))
            .unwrap_or_else(|| Ok(Transform::default()))?;

        Ok(())
    }

    fn set_conditional_processing_attributes(&mut self) -> Result<(), ElementError> {
        let mut cond = self.cond;

        for (attr, value) in self.attributes.iter() {
            match attr.expanded() {
                expanded_name!("", "requiredExtensions") if cond => {
                    cond = RequiredExtensions::from_attribute(value)
                        .map(|RequiredExtensions(res)| res)
                        .attribute(attr)?;
                }

                expanded_name!("", "requiredFeatures") if cond => {
                    cond = RequiredFeatures::from_attribute(value)
                        .map(|RequiredFeatures(res)| res)
                        .attribute(attr)?;
                }

                expanded_name!("", "systemLanguage") if cond => {
                    cond = SystemLanguage::from_attribute(value, &LOCALE)
                        .map(|SystemLanguage(res)| res)
                        .attribute(attr)?;
                }

                _ => {}
            }
        }

        self.cond = cond;

        Ok(())
    }

    /// Hands the `attrs` to the node's state, to apply the presentation attributes.
    fn set_presentation_attributes(&mut self) -> Result<(), ElementError> {
        match self
            .specified_values
            .parse_presentation_attributes(&self.attributes)
        {
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

    // Applies a style declaration to the node's specified_values
    fn apply_style_declaration(&mut self, declaration: &Declaration, origin: Origin) {
        self.specified_values.set_property_from_declaration(
            declaration,
            origin,
            &mut self.important_styles,
        );
    }

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
}

impl<T: SetAttributes + Draw> Draw for ElementInner<T> {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        if !self.is_in_error() {
            draw_ctx.with_saved_transform(Some(self.get_transform()), &mut |dc| {
                self.element_impl
                    .draw(node, acquired_nodes, cascaded, dc, clipping)
            })
        } else {
            rsvg_log!("(not rendering element {} because it is in error)", self);

            // maybe we should actually return a RenderingError::ElementIsInError here?
            Ok(draw_ctx.empty_bbox())
        }
    }
}

impl<T: SetAttributes + Draw> fmt::Display for ElementInner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.element_name().local)?;
        write!(f, " id={}", self.get_id().unwrap_or("None"))?;
        Ok(())
    }
}

impl<T: SetAttributes + Draw> Deref for ElementInner<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.element_impl
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
    // end recursion, call the method
    ($element:ident, $method:ident [$($args:expr),*]) => {
        match $element {
            Element::Circle(i) => i.$method($($args),*),
            Element::ClipPath(i) => i.$method($($args),*),
            Element::Ellipse(i) => i.$method($($args),*),
            Element::Filter(i) => i.$method($($args),*),
            Element::Group(i) => i.$method($($args),*),
            Element::Image(i) => i.$method($($args),*),
            Element::Line(i) => i.$method($($args),*),
            Element::LinearGradient(i) => i.$method($($args),*),
            Element::Link(i) => i.$method($($args),*),
            Element::Marker(i) => i.$method($($args),*),
            Element::Mask(i) => i.$method($($args),*),
            Element::NonRendering(i) => i.$method($($args),*),
            Element::Path(i) => i.$method($($args),*),
            Element::Pattern(i) => i.$method($($args),*),
            Element::Polygon(i) => i.$method($($args),*),
            Element::Polyline(i) => i.$method($($args),*),
            Element::RadialGradient(i) => i.$method($($args),*),
            Element::Rect(i) => i.$method($($args),*),
            Element::Stop(i) => i.$method($($args),*),
            Element::Style(i) => i.$method($($args),*),
            Element::Svg(i) => i.$method($($args),*),
            Element::Switch(i) => i.$method($($args),*),
            Element::Symbol(i) => i.$method($($args),*),
            Element::Text(i) => i.$method($($args),*),
            Element::TRef(i) => i.$method($($args),*),
            Element::TSpan(i) => i.$method($($args),*),
            Element::Use(i) => i.$method($($args),*),
            Element::FeBlend(i) => i.$method($($args),*),
            Element::FeColorMatrix(i) => i.$method($($args),*),
            Element::FeComponentTransfer(i) => i.$method($($args),*),
            Element::FeComposite(i) => i.$method($($args),*),
            Element::FeConvolveMatrix(i) => i.$method($($args),*),
            Element::FeDiffuseLighting(i) => i.$method($($args),*),
            Element::FeDisplacementMap(i) => i.$method($($args),*),
            Element::FeDistantLight(i) => i.$method($($args),*),
            Element::FeFlood(i) => i.$method($($args),*),
            Element::FeFuncA(i) => i.$method($($args),*),
            Element::FeFuncB(i) => i.$method($($args),*),
            Element::FeFuncG(i) => i.$method($($args),*),
            Element::FeFuncR(i) => i.$method($($args),*),
            Element::FeGaussianBlur(i) => i.$method($($args),*),
            Element::FeImage(i) => i.$method($($args),*),
            Element::FeMerge(i) => i.$method($($args),*),
            Element::FeMergeNode(i) => i.$method($($args),*),
            Element::FeMorphology(i) => i.$method($($args),*),
            Element::FeOffset(i) => i.$method($($args),*),
            Element::FePointLight(i) => i.$method($($args),*),
            Element::FeSpecularLighting(i) => i.$method($($args),*),
            Element::FeSpotLight(i) => i.$method($($args),*),
            Element::FeTile(i) => i.$method($($args),*),
            Element::FeTurbulence(i) => i.$method($($args),*),
        }
    };

    // munch munch
    ($element:ident, $method:ident [$($args:expr),*] $arg:expr, $($rest:tt)*) => {
        call_inner!($element, $method [$($args,)*$arg] $($rest)*)
    };

    // entry point with args
    ($element:ident, $method:ident, $arg:expr, $($args:expr),*) => {
        call_inner!($element, $method [$arg] $($args,)*)
    };

    // entry point with one arg
    ($element:ident, $method:ident, $arg:expr) => {
        call_inner!($element, $method [$arg])
    };

    // entry point without args
    ($element:ident, $method:ident) => {
        call_inner!($element, $method [])
    };
}

impl Element {
    /// Takes an XML element name and consumes a list of attribute/value pairs to create an [`Element`].
    ///
    /// This operation does not fail.  Unknown element names simply produce a [`NonRendering`]
    /// element.
    ///
    /// [`Element`]: type.Element.html
    /// [`NonRendering`]: ../structure/struct.NonRendering.html
    pub fn new(name: &QualName, attrs: Attributes) -> Element {
        let mut id = None;
        let mut class = None;

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "id") => id = Some(String::from(value)),
                expanded_name!("", "class") => class = Some(String::from(value)),
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

        create_fn(name, attrs, id, class)
    }

    pub fn element_name(&self) -> &QualName {
        call_inner!(self, element_name)
    }

    pub fn get_attributes(&self) -> &Attributes {
        call_inner!(self, get_attributes)
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

    pub fn apply_style_declaration(&mut self, declaration: &Declaration, origin: Origin) {
        call_inner!(self, apply_style_declaration, declaration, origin)
    }

    pub fn set_style_attribute(&mut self) {
        call_inner!(self, set_style_attribute);
    }

    pub fn is_in_error(&self) -> bool {
        call_inner!(self, is_in_error)
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

impl Draw for Element {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        call_inner!(
            self,
            draw,
            node,
            acquired_nodes,
            cascaded,
            draw_ctx,
            clipping
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
        pub fn $name(
            element_name: &QualName,
            attributes: Attributes,
            id: Option<String>,
            class: Option<String>,
        ) -> Element {
            let mut element_impl = <$element_type>::default();

            let result = element_impl.set_attributes(&attributes);

            let element = Element::$element_type(Box::new(ElementInner::new(
                element_name.clone(),
                id,
                class,
                attributes,
                result,
                element_impl,
            )));

            element
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

type ElementCreateFn = fn(
    element_name: &QualName,
    attributes: Attributes,
    id: Option<String>,
    class: Option<String>,
) -> Element;

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

/// Gets the user's preferred locale from the environment and
/// translates it to a `Locale` with `LanguageRange` fallbacks.
///
/// The `Locale::current()` call only contemplates a single language,
/// but glib is smarter, and `g_get_langauge_names()` can provide
/// fallbacks, for example, when LC_MESSAGES="en_US.UTF-8:de" (USA
/// English and German).  This function converts the output of
/// `g_get_language_names()` into a `Locale` with appropriate
/// fallbacks.
static LOCALE: Lazy<Locale> = Lazy::new(|| {
    let mut locale = Locale::invariant();

    // This call has a lot of memory churn internally with many
    // short-lived allocations, so we do this only once.
    for name in glib::get_language_names() {
        if let Ok(range) = LanguageRange::from_unix(&name) {
            locale.add(&range);
        }
    }

    locale
});

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
