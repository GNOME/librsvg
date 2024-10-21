//! Utilities for dealing with Cairo paths.
//!
//! Librsvg uses Cairo to render Bézier paths, and also depends on Cairo to
//! compute the extents of those paths.  This module holds a number of utilities
//! to convert between librsvg paths and Cairo paths.

use std::f64::consts::PI;

use crate::error::InternalRenderingError;
use crate::path_builder::{
    arc_segment, ArcParameterization, CubicBezierCurve, EllipticalArc, Path, PathBuilder,
    PathCommand,
};
use crate::rect::Rect;

pub fn compute_path_extents(path: &Path) -> Result<Option<Rect>, InternalRenderingError> {
    if path.is_empty() {
        return Ok(None);
    }

    let surface = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None)?;
    let cr = cairo::Context::new(&surface)?;

    path.to_cairo(&cr, false)?;
    let (x0, y0, x1, y1) = cr.path_extents()?;

    Ok(Some(Rect::new(x0, y0, x1, y1)))
}

impl Path {
    pub fn to_cairo(
        &self,
        cr: &cairo::Context,
        is_square_linecap: bool,
    ) -> Result<(), InternalRenderingError> {
        assert!(!self.is_empty());

        for subpath in self.iter_subpath() {
            // If a subpath is empty and the linecap is a square, then draw a square centered on
            // the origin of the subpath. See #165.
            if is_square_linecap {
                let (x, y) = subpath.origin();
                if subpath.is_zero_length() {
                    let stroke_size = 0.002;

                    cr.move_to(x - stroke_size / 2., y);
                    cr.line_to(x + stroke_size / 2., y);
                }
            }

            for cmd in subpath.iter_commands() {
                cmd.to_cairo(cr);
            }
        }

        // We check the cr's status right after feeding it a new path for a few reasons:
        //
        // * Any of the individual path commands may cause the cr to enter an error state, for
        //   example, if they come with coordinates outside of Cairo's supported range.
        //
        // * The *next* call to the cr will probably be something that actually checks the status
        //   (i.e. in cairo-rs), and we don't want to panic there.

        cr.status().map_err(|e| e.into())
    }

    /// Converts a `cairo::Path` to a librsvg `Path`.
    pub fn from_cairo(cairo_path: cairo::Path) -> Path {
        let mut builder = PathBuilder::default();

        // Cairo has the habit of appending a MoveTo to some paths, but we don't want a
        // path for empty text to generate that lone point.  So, strip out paths composed
        // only of MoveTo.

        if !cairo_path_is_only_move_tos(&cairo_path) {
            for segment in cairo_path.iter() {
                match segment {
                    cairo::PathSegment::MoveTo((x, y)) => builder.move_to(x, y),
                    cairo::PathSegment::LineTo((x, y)) => builder.line_to(x, y),
                    cairo::PathSegment::CurveTo((x2, y2), (x3, y3), (x4, y4)) => {
                        builder.curve_to(x2, y2, x3, y3, x4, y4)
                    }
                    cairo::PathSegment::ClosePath => builder.close_path(),
                }
            }
        }

        builder.into_path()
    }
}

fn cairo_path_is_only_move_tos(path: &cairo::Path) -> bool {
    path.iter()
        .all(|seg| matches!(seg, cairo::PathSegment::MoveTo((_, _))))
}

impl PathCommand {
    fn to_cairo(&self, cr: &cairo::Context) {
        match *self {
            PathCommand::MoveTo(x, y) => cr.move_to(x, y),
            PathCommand::LineTo(x, y) => cr.line_to(x, y),
            PathCommand::CurveTo(ref curve) => curve.to_cairo(cr),
            PathCommand::Arc(ref arc) => arc.to_cairo(cr),
            PathCommand::ClosePath => cr.close_path(),
        }
    }
}

impl EllipticalArc {
    fn to_cairo(&self, cr: &cairo::Context) {
        match self.center_parameterization() {
            ArcParameterization::CenterParameters {
                center,
                radii,
                theta1,
                delta_theta,
            } => {
                let n_segs = (delta_theta / (PI * 0.5 + 0.001)).abs().ceil() as u32;
                let d_theta = delta_theta / f64::from(n_segs);

                let mut theta = theta1;
                for _ in 0..n_segs {
                    arc_segment(center, radii, self.x_axis_rotation, theta, theta + d_theta)
                        .to_cairo(cr);
                    theta += d_theta;
                }
            }
            ArcParameterization::LineTo => {
                let (x2, y2) = self.to;
                cr.line_to(x2, y2);
            }
            ArcParameterization::Omit => {}
        }
    }
}

impl CubicBezierCurve {
    fn to_cairo(&self, cr: &cairo::Context) {
        let Self { pt1, pt2, to } = *self;
        cr.curve_to(pt1.0, pt1.1, pt2.0, pt2.1, to.0, to.1);
    }
}

