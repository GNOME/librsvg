//! Layout tree.
//!
//! The idea is to take the DOM tree and produce a layout tree with SVG concepts.

use crate::transform::Transform;

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
    pub transform: Transform,
}

impl StackingContext {
    pub fn new(transform: Transform) -> StackingContext {
        StackingContext {
            transform,
        }
    }
}
