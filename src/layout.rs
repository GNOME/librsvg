//! Layout tree.
//!
//! The idea is to take the DOM tree and produce a layout tree with SVG concepts.

use crate::dasharray::Dasharray;
use crate::document::AcquiredNodes;
use crate::element::Element;
use crate::length::*;
use crate::node::*;
use crate::properties::ComputedValues;
use crate::property_defs::{
    Filter, Opacity, StrokeDasharray, StrokeLinecap, StrokeLinejoin, StrokeMiterlimit,
};
use crate::transform::Transform;
use crate::unit_interval::UnitInterval;

/// SVG Stacking context, an inner node in the layout tree.
///
/// https://www.w3.org/TR/SVG2/render.html#EstablishingStackingContex
///
/// This is not strictly speaking an SVG2 stacking context, but a
/// looser version of it.  For example. the SVG spec mentions that a
/// an element should establish a stacking context if the `filter`
/// property applies to the element and is not `none`.  In that case,
/// the element is rendered as an "isolated group" -
/// https://www.w3.org/TR/2015/CR-compositing-1-20150113/#csscompositingrules_SVG
///
/// Here we store all the parameters that may lead to the decision to actually
/// render an element as an isolated group.
pub struct StackingContext {
    pub element_name: String,
    pub transform: Transform,
    pub opacity: Opacity,
    pub filter: Filter,
    pub mask: Option<Node>,
}

/// Stroke parameters in user-space coordinates.
pub struct Stroke {
    pub width: f64,
    pub miter_limit: StrokeMiterlimit,
    pub line_cap: StrokeLinecap,
    pub line_join: StrokeLinejoin,
    pub dash_offset: f64,
    pub dashes: Box<[f64]>,
}

impl StackingContext {
    pub fn new(
        acquired_nodes: &mut AcquiredNodes<'_>,
        element: &Element,
        transform: Transform,
        values: &ComputedValues,
    ) -> StackingContext {
        let element_name = format!("{}", element);

        let opacity;
        let filter;

        match *element {
            // "The opacity, filter and display properties do not apply to the mask element"
            // https://drafts.fxtf.org/css-masking-1/#MaskElement
            Element::Mask(_) => {
                opacity = Opacity(UnitInterval::clamp(1.0));
                filter = Filter::None;
            }

            _ => {
                opacity = values.opacity();
                filter = values.filter();
            }
        }

        let mask = values.mask().0.get().and_then(|mask_id| {
            if let Ok(acquired) = acquired_nodes.acquire(mask_id) {
                let node = acquired.get();
                match *node.borrow_element() {
                    Element::Mask(_) => Some(node.clone()),

                    _ => {
                        rsvg_log!(
                            "element {} references \"{}\" which is not a mask",
                            element,
                            mask_id
                        );

                        None
                    }
                }
            } else {
                rsvg_log!(
                    "element {} references nonexistent mask \"{}\"",
                    element,
                    mask_id
                );

                None
            }
        });

        StackingContext {
            element_name,
            transform,
            opacity,
            filter,
            mask,
        }
    }
}

impl Stroke {
    pub fn new(values: &ComputedValues, params: &NormalizeParams) -> Stroke {
        let width = values.stroke_width().0.to_user(params);
        let miter_limit = values.stroke_miterlimit();
        let line_cap = values.stroke_line_cap();
        let line_join = values.stroke_line_join();
        let dash_offset = values.stroke_dashoffset().0.to_user(&params);

        let dashes = match values.stroke_dasharray() {
            StrokeDasharray(Dasharray::None) => Box::new([]),
            StrokeDasharray(Dasharray::Array(dashes)) => dashes
                .iter()
                .map(|l| l.to_user(&params))
                .collect::<Box<[f64]>>(),
        };

        Stroke {
            width,
            miter_limit,
            line_cap,
            line_join,
            dash_offset,
            dashes,
        }
    }
}
