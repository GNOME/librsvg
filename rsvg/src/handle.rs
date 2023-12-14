//! Toplevel handle for a loaded SVG document.
//!
//! This module provides the primitives on which the public APIs are implemented.

use crate::accept_language::UserLanguage;
use crate::bbox::BoundingBox;
use crate::document::{AcquiredNodes, Document};
use crate::dpi::Dpi;
use crate::drawing_ctx::{draw_tree, with_saved_cr, DrawingMode, SvgNesting};
use crate::error::InternalRenderingError;
use crate::node::Node;
use crate::rect::Rect;
use crate::session::Session;
