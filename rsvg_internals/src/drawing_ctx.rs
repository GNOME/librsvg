use cairo;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::rc::{Rc, Weak};

use crate::allowed_url::Fragment;
use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::clip_path::{ClipPath, ClipPathUnits};
use crate::coord_units::CoordUnits;
use crate::dasharray::Dasharray;
use crate::document::Document;
use crate::dpi::Dpi;
use crate::error::{AcquireError, RenderingError};
use crate::filters;
use crate::gradient::{LinearGradient, RadialGradient};
use crate::limits;
use crate::mask::Mask;
use crate::node::{CascadedValues, NodeDraw, NodeType, RsvgNode};
use crate::paint_server::{PaintServer, PaintSource};
use crate::pattern::Pattern;
use crate::properties::ComputedValues;
use crate::property_defs::{
    ClipRule, FillRule, ShapeRendering, StrokeDasharray, StrokeLinecap, StrokeLinejoin,
};
use crate::rect::Rect;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::unit_interval::UnitInterval;
use crate::viewbox::ViewBox;

/// Holds values that are required to normalize `Length` values to a current viewport.
///
/// This struct is created by calling `DrawingCtx::push_view_box()` or
/// `DrawingCtx::get_view_params()`.
///
/// This struct holds the size of the current viewport in the user's coordinate system.  A
/// viewport pushed with `DrawingCtx::push_view_box()` will remain in place until the
/// returned `ViewParams` is dropped; at that point, the `DrawingCtx` will resume using its
/// previous viewport.
pub struct ViewParams {
    pub dpi_x: f64,
    pub dpi_y: f64,
    pub view_box_width: f64,
    pub view_box_height: f64,
    view_box_stack: Option<Weak<RefCell<Vec<ViewBox>>>>,
}

impl ViewParams {
    pub fn new(dpi_x: f64, dpi_y: f64, view_box_width: f64, view_box_height: f64) -> ViewParams {
        ViewParams {
            dpi_x,
            dpi_y,
            view_box_width,
            view_box_height,
            view_box_stack: None,
        }
    }
}

impl Drop for ViewParams {
    fn drop(&mut self) {
        if let Some(ref weak_stack) = self.view_box_stack {
            let stack = weak_stack
                .upgrade()
                .expect("A ViewParams was dropped after its DrawingCtx!?");
            stack.borrow_mut().pop();
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ClipMode {
    ClipToViewport,
    ClipToVbox,
}

pub struct DrawingCtx {
    document: Rc<Document>,

    initial_affine: cairo::Matrix,

    rect: Rect,
    dpi: Dpi,

    // This is a mitigation for SVG files that try to instance a huge number of
    // elements via <use>, recursive patterns, etc.  See limits.rs for details.
    num_elements_acquired: usize,

    cr_stack: Vec<cairo::Context>,
    cr: cairo::Context,

    view_box_stack: Rc<RefCell<Vec<ViewBox>>>,

    drawsub_stack: Vec<RsvgNode>,

    acquired_nodes: AcquiredNodes,

    measuring: bool,
    testing: bool,
}

impl DrawingCtx {
    pub fn new(
        document: Rc<Document>,
        node: Option<&RsvgNode>,
        cr: &cairo::Context,
        viewport: Rect,
        dpi: Dpi,
        measuring: bool,
        testing: bool,
    ) -> DrawingCtx {
        let initial_affine = cr.get_matrix();

        // This is more or less a hack to make measuring geometries possible,
        // while the code gets refactored not to need special cases for that.

        let (rect, vbox) = if measuring {
            (
                Rect::from_size(1.0, 1.0),
                ViewBox::new(0.0, 0.0, 1.0, 1.0),
            )
        } else {
            // https://www.w3.org/TR/SVG2/coords.html#InitialCoordinateSystem
            //
            // "For the outermost svg element, the SVG user agent must
            // determine an initial viewport coordinate system and an
            // initial user coordinate system such that the two
            // coordinates systems are identical. The origin of both
            // coordinate systems must be at the origin of the SVG
            // viewport."
            //
            // "... the initial viewport coordinate system (and therefore
            // the initial user coordinate system) must have its origin at
            // the top/left of the viewport"
            let vbox = ViewBox {
                x: 0.0,
                y: 0.0,
                width: viewport.width(),
                height: viewport.height(),
            };

            (viewport, vbox)
        };

        let mut view_box_stack = Vec::new();
        view_box_stack.push(vbox);

        let acquired_nodes = AcquiredNodes::new(document.clone());

        let mut draw_ctx = DrawingCtx {
            document,
            initial_affine,
            rect,
            dpi,
            num_elements_acquired: 0,
            cr_stack: Vec::new(),
            cr: cr.clone(),
            view_box_stack: Rc::new(RefCell::new(view_box_stack)),
            drawsub_stack: Vec::new(),
            acquired_nodes,
            measuring,
            testing,
        };

        if let Some(node) = node {
            draw_ctx.add_node_and_ancestors_to_stack(node);
        }

        draw_ctx
    }

    pub fn toplevel_viewport(&self) -> Rect {
        self.rect
    }

    pub fn is_measuring(&self) -> bool {
        self.measuring
    }

    pub fn is_testing(&self) -> bool {
        self.testing
    }

    pub fn get_cairo_context(&self) -> cairo::Context {
        self.cr.clone()
    }

    pub fn empty_bbox(&self) -> BoundingBox {
        BoundingBox::new(&self.cr.get_matrix())
    }

    // FIXME: Usage of this function is more less a hack... The caller
    // manually saves and then restore the draw_ctx.cr.
    // It would be better to have an explicit push/pop for the cairo_t, or
    // pushing a temporary surface, or something that does not involve
    // monkeypatching the cr directly.
    pub fn set_cairo_context(&mut self, cr: &cairo::Context) {
        self.cr = cr.clone();
    }

    // Temporary hack while we unify surface/cr/affine creation
    pub fn push_cairo_context(&mut self, cr: cairo::Context) {
        self.cr_stack.push(self.cr.clone());
        self.cr = cr;
    }

    // Temporary hack while we unify surface/cr/affine creation
    pub fn pop_cairo_context(&mut self) {
        self.cr = self.cr_stack.pop().unwrap();
    }

    fn size_for_temporary_surface(&self) -> (i32, i32) {
        let (viewport_width, viewport_height) = (self.rect.width(), self.rect.height());

        let (scaled_width, scaled_height) = self
            .initial_affine_with_offset()
            .transform_distance(viewport_width, viewport_height);

        // We need a size in whole pixels, so use ceil() to ensure the whole viewport fits
        // into the temporary surface.
        let width = scaled_width.ceil() as i32;
        let height = scaled_height.ceil() as i32;

        (width, height)
    }

    pub fn create_surface_for_toplevel_viewport(
        &self,
    ) -> Result<cairo::ImageSurface, RenderingError> {
        let (w, h) = self.size_for_temporary_surface();

        Ok(cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?)
    }

    fn create_similar_surface_for_toplevel_viewport(
        &self,
        surface: &cairo::Surface,
    ) -> Result<cairo::Surface, RenderingError> {
        let (w, h) = self.size_for_temporary_surface();

        let surface = cairo::Surface::create_similar(surface, cairo::Content::ColorAlpha, w, h);

        // FIXME: cairo-rs should return a Result from create_similar()!
        // Since it doesn't, we need to check its status by hand...

        let status = surface.status();
        if status == cairo::Status::Success {
            Ok(surface)
        } else {
            Err(RenderingError::Cairo(status))
        }
    }

    /// Gets the viewport that was last pushed with `push_view_box()`.
    pub fn get_view_params(&self) -> ViewParams {
        let view_box_stack = self.view_box_stack.borrow();
        let last = view_box_stack.len() - 1;
        let stack_top = &view_box_stack[last];

        ViewParams {
            dpi_x: self.dpi.x(),
            dpi_y: self.dpi.y(),
            view_box_width: stack_top.width,
            view_box_height: stack_top.height,
            view_box_stack: None,
        }
    }

    /// Pushes a viewport size for normalizing `Length` values.
    ///
    /// You should pass the returned `ViewParams` to all subsequent `Length.normalize()`
    /// calls that correspond to this viewport.
    ///
    /// The viewport will stay in place, and will be the one returned by
    /// `get_view_params()`, until the returned `ViewParams` is dropped.
    pub fn push_view_box(&self, width: f64, height: f64) -> ViewParams {
        self.view_box_stack
            .borrow_mut()
            .push(ViewBox::new(0.0, 0.0, width, height));

        ViewParams {
            dpi_x: self.dpi.x(),
            dpi_y: self.dpi.y(),
            view_box_width: width,
            view_box_height: height,
            view_box_stack: Some(Rc::downgrade(&self.view_box_stack)),
        }
    }

    pub fn push_new_viewport(
        &self,
        vbox: Option<ViewBox>,
        viewport: Rect,
        preserve_aspect_ratio: AspectRatio,
        clip_mode: Option<ClipMode>,
    ) -> Option<ViewParams> {
        if let Some(ref clip) = clip_mode {
            if *clip == ClipMode::ClipToViewport {
                self.clip(viewport);
            }
        }

        preserve_aspect_ratio
            .viewport_to_viewbox_transform(vbox, viewport)
            .and_then(|matrix| {
                self.cr.transform(matrix);

                if let Some(vbox) = vbox {
                    if let Some(ref clip) = clip_mode {
                        if *clip == ClipMode::ClipToVbox {
                            self.clip(Rect::new(
                                vbox.x,
                                vbox.y,
                                vbox.x + vbox.width,
                                vbox.y + vbox.height,
                            ));
                        }
                    }

                    Some(self.push_view_box(vbox.width, vbox.height))
                } else {
                    Some(self.get_view_params())
                }
            })
    }

    // Use this function when looking up urls to other nodes, and when you expect
    // the node to be of a particular type. This function does proper recursion
    // checking and thereby avoids infinite loops.
    //
    // Nodes acquired by this function must be released in reverse
    // acquiring order.
    //
    // Specify an empty slice for `node_types` if you want a node of any type.
    //
    // Malformed SVGs, for example, may reference a marker by its IRI, but
    // the object referenced by the IRI is not a marker.
    //
    // Note that if you acquire a node, you have to release it before trying to
    // acquire it again.  If you acquire a node "#foo" and don't release it before
    // trying to acquire "foo" again, you will obtain a None the second time.
    pub fn acquire_node(
        &mut self,
        fragment: &Fragment,
        node_types: &[NodeType],
    ) -> Result<AcquiredNode, AcquireError> {
        self.num_elements_acquired += 1;

        if self.num_elements_acquired > limits::MAX_REFERENCED_ELEMENTS {
            return Err(AcquireError::MaxReferencesExceeded);
        }

        self.acquired_nodes.acquire(fragment, node_types)
    }

    pub fn acquire_node_ref(&mut self, node: &RsvgNode) -> Result<AcquiredNode, AcquireError> {
        self.acquired_nodes.push_node_ref(node)
    }

    // Returns (clip_in_user_space, clip_in_object_space), both Option<RsvgNode>
    fn get_clip_in_user_and_object_space(
        &mut self,
        clip_uri: Option<&Fragment>,
    ) -> (Option<RsvgNode>, Option<RsvgNode>) {
        clip_uri
            .and_then(|fragment| self.acquire_node(fragment, &[NodeType::ClipPath]).ok())
            .and_then(|acquired| {
                let clip_node = acquired.get().clone();

                let ClipPathUnits(units) = clip_node.borrow().get_impl::<ClipPath>().get_units();

                if units == CoordUnits::UserSpaceOnUse {
                    Some((Some(clip_node), None))
                } else {
                    assert!(units == CoordUnits::ObjectBoundingBox);
                    Some((None, Some(clip_node)))
                }
            })
            .unwrap_or((None, None))
    }

    fn clip_to_node(
        &mut self,
        clip_node: &Option<RsvgNode>,
        bbox: &BoundingBox,
    ) -> Result<(), RenderingError> {
        if let Some(node) = clip_node {
            let node_data = node.borrow();
            let clip_path = node_data.get_impl::<ClipPath>();
            clip_path.to_cairo_context(&node, self, &bbox)
        } else {
            Ok(())
        }
    }

    pub fn with_discrete_layer(
        &mut self,
        node: &RsvgNode,
        values: &ComputedValues,
        clipping: bool,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        if clipping {
            draw_fn(self)
        } else {
            self.with_saved_cr(&mut |dc| {
                let clip_uri = values.clip_path.0.get();
                let mask = values.mask.0.get();

                // The `filter` property does not apply to masks.
                let filter = if node.borrow().get_type() == NodeType::Mask {
                    None
                } else {
                    values.filter.0.get()
                };

                let UnitInterval(opacity) = values.opacity.0;

                let affine_at_start = dc.cr.get_matrix();

                let (clip_in_user_space, clip_in_object_space) =
                    dc.get_clip_in_user_and_object_space(clip_uri);

                // Here we are clipping in user space, so the bbox doesn't matter
                dc.clip_to_node(&clip_in_user_space, &dc.empty_bbox())?;

                let needs_temporary_surface = !(opacity == 1.0
                    && filter.is_none()
                    && mask.is_none()
                    && clip_in_object_space.is_none());

                if needs_temporary_surface {
                    // Compute our assortment of affines

                    let affines = CompositingAffines::new(
                        affine_at_start,
                        dc.initial_affine_with_offset(),
                        dc.cr_stack.len(),
                    );

                    // Create temporary surface and its cr

                    let cr = if filter.is_some() {
                        cairo::Context::new(&*dc.create_surface_for_toplevel_viewport()?)
                    } else {
                        cairo::Context::new(
                            &dc.create_similar_surface_for_toplevel_viewport(&dc.cr.get_target())?,
                        )
                    };

                    cr.set_matrix(affines.for_temporary_surface);

                    dc.push_cairo_context(cr);

                    // Draw!

                    let mut res = draw_fn(dc);

                    let bbox = if let Ok(ref bbox) = res {
                        *bbox
                    } else {
                        BoundingBox::new(&affines.for_temporary_surface)
                    };

                    // Filter

                    let source_surface = if let Some(filter_uri) = filter {
                        let child_surface =
                            cairo::ImageSurface::try_from(dc.cr.get_target()).unwrap();
                        let img_surface =
                            dc.run_filter(filter_uri, node, values, &child_surface, bbox)?;
                        // turn into a Surface
                        (*img_surface).clone()
                    } else {
                        dc.cr.get_target()
                    };

                    dc.pop_cairo_context();

                    // Set temporary surface as source

                    dc.cr.set_matrix(affines.compositing);
                    dc.cr.set_source_surface(&source_surface, 0.0, 0.0);

                    // Clip

                    dc.cr.set_matrix(affines.outside_temporary_surface);
                    let _: () = dc.clip_to_node(&clip_in_object_space, &bbox)?;

                    // Mask

                    if let Some(fragment) = mask {
                        if let Ok(acquired) = dc.acquire_node(fragment, &[NodeType::Mask]) {
                            let mask_node = acquired.get();

                            res = res.and_then(|bbox| {
                                mask_node
                                    .borrow()
                                    .get_impl::<Mask>()
                                    .generate_cairo_mask(&mask_node, &affines, dc, &bbox)
                                    .and_then(|mask_surf| {
                                        if let Some(surf) = mask_surf {
                                            dc.cr.set_matrix(affines.compositing);
                                            dc.cr.mask_surface(&surf, 0.0, 0.0);
                                        }
                                        Ok(())
                                    })
                                    .map(|_: ()| bbox)
                            });
                        } else {
                            rsvg_log!(
                                "element {} references nonexistent mask \"{}\"",
                                node,
                                fragment
                            );
                        }
                    } else {
                        // No mask, so composite the temporary surface

                        dc.cr.set_matrix(affines.compositing);

                        if opacity < 1.0 {
                            dc.cr.paint_with_alpha(opacity);
                        } else {
                            dc.cr.paint();
                        }
                    }

                    dc.cr.set_matrix(affine_at_start);

                    res
                } else {
                    draw_fn(dc)
                }
            })
        }
    }

    fn initial_affine_with_offset(&self) -> cairo::Matrix {
        let mut initial_with_offset = self.initial_affine;
        initial_with_offset.translate(self.rect.x0, self.rect.y0);
        initial_with_offset
    }

    /// Saves the current Cairo matrix, runs the draw_fn, and restores the matrix
    ///
    /// This is slightly cheaper than a `cr.save()` / `cr.restore()`
    /// pair, but more importantly, it does not reset the whole
    /// graphics state, i.e. it leaves a clipping path in place if it
    /// was set by the `draw_fn`.
    pub fn with_saved_matrix(
        &mut self,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        let matrix = self.cr.get_matrix();
        let res = draw_fn(self);
        self.cr.set_matrix(matrix);

        if let Ok(bbox) = res {
            let mut orig_matrix_bbox = BoundingBox::new(&matrix);
            orig_matrix_bbox.insert(&bbox);
            Ok(orig_matrix_bbox)
        } else {
            res
        }
    }

    /// Saves the current Cairo context, runs the draw_fn, and restores the context
    pub fn with_saved_cr(
        &mut self,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        self.cr.save();
        let res = draw_fn(self);
        self.cr.restore();
        res
    }

    fn run_filter(
        &mut self,
        filter_uri: &Fragment,
        node: &RsvgNode,
        values: &ComputedValues,
        child_surface: &cairo::ImageSurface,
        node_bbox: BoundingBox,
    ) -> Result<cairo::ImageSurface, RenderingError> {
        match self.acquire_node(filter_uri, &[NodeType::Filter]) {
            Ok(acquired) => {
                let filter_node = acquired.get();

                if !filter_node.borrow().is_in_error() {
                    // FIXME: deal with out of memory here
                    filters::render(&filter_node, values, child_surface, self, node_bbox)
                } else {
                    Ok(child_surface.clone())
                }
            }

            Err(_) => {
                rsvg_log!(
                    "element {} will not be rendered since its filter \"{}\" was not found",
                    node,
                    filter_uri,
                );

                // Non-existing filters must act as null filters (that is, an
                // empty surface is returned).
                Ok(cairo::ImageSurface::create(
                    cairo::Format::ARgb32,
                    child_surface.get_width(),
                    child_surface.get_height(),
                )?)
            }
        }
    }

    fn set_color(
        &self,
        color: cssparser::Color,
        opacity: UnitInterval,
        current_color: cssparser::RGBA,
    ) {
        let rgba = match color {
            cssparser::Color::RGBA(rgba) => rgba,
            cssparser::Color::CurrentColor => current_color,
        };

        let UnitInterval(o) = opacity;
        self.get_cairo_context().set_source_rgba(
            f64::from(rgba.red_f32()),
            f64::from(rgba.green_f32()),
            f64::from(rgba.blue_f32()),
            f64::from(rgba.alpha_f32()) * o,
        );
    }

    fn acquire_paint_server(&mut self, fragment: &Fragment) -> Result<AcquiredNode, AcquireError> {
        self.acquire_node(
            fragment,
            &[
                NodeType::LinearGradient,
                NodeType::RadialGradient,
                NodeType::Pattern,
            ],
        )
    }

    pub fn set_source_paint_server(
        &mut self,
        ps: &PaintServer,
        opacity: UnitInterval,
        bbox: &BoundingBox,
        current_color: cssparser::RGBA,
    ) -> Result<bool, RenderingError> {
        match *ps {
            PaintServer::Iri {
                ref iri,
                ref alternate,
            } => {
                let mut had_paint_server = false;

                match self.acquire_paint_server(iri) {
                    Ok(acquired) => {
                        let node = acquired.get();

                        had_paint_server = match node.borrow().get_type() {
                            NodeType::LinearGradient => node
                                .borrow()
                                .get_impl::<LinearGradient>()
                                .resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)?,
                            NodeType::RadialGradient => node
                                .borrow()
                                .get_impl::<RadialGradient>()
                                .resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)?,
                            NodeType::Pattern => node
                                .borrow()
                                .get_impl::<Pattern>()
                                .resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)?,
                            _ => unreachable!(),
                        }
                    }

                    Err(AcquireError::MaxReferencesExceeded) => {
                        return Err(RenderingError::InstancingLimit);
                    }

                    Err(_) => (),
                }

                if !had_paint_server && alternate.is_some() {
                    self.set_color(alternate.unwrap(), opacity, current_color);
                    had_paint_server = true;
                } else {
                    rsvg_log!(
                        "pattern \"{}\" was not found and there was no fallback alternate",
                        iri
                    );
                }

                Ok(had_paint_server)
            }

            PaintServer::SolidColor(color) => {
                self.set_color(color, opacity, current_color);
                Ok(true)
            }

            PaintServer::None => Ok(false),
        }
    }

    pub fn setup_cr_for_stroke(&self, cr: &cairo::Context, values: &ComputedValues) {
        let params = self.get_view_params();

        cr.set_line_width(values.stroke_width.0.normalize(values, &params));
        cr.set_miter_limit(values.stroke_miterlimit.0);
        cr.set_line_cap(cairo::LineCap::from(values.stroke_line_cap));
        cr.set_line_join(cairo::LineJoin::from(values.stroke_line_join));

        if let StrokeDasharray(Dasharray::Array(ref dashes)) = values.stroke_dasharray {
            let normalized_dashes: Vec<f64> = dashes
                .iter()
                .map(|l| l.normalize(values, &params))
                .collect();

            let total_length = normalized_dashes.iter().fold(0.0, |acc, &len| acc + len);

            if total_length > 0.0 {
                let offset = values.stroke_dashoffset.0.normalize(values, &params);
                cr.set_dash(&normalized_dashes, offset);
            } else {
                cr.set_dash(&[], 0.0);
            }
        }
    }

    pub fn stroke_and_fill(
        &mut self,
        cr: &cairo::Context,
        values: &ComputedValues,
    ) -> Result<BoundingBox, RenderingError> {
        cr.set_antialias(cairo::Antialias::from(values.shape_rendering));

        self.setup_cr_for_stroke(cr, values);

        // Update the bbox in the rendering context.  Below, we actually set the
        // fill/stroke patterns on the cairo_t.  That process requires the
        // rendering context to have an updated bbox; for example, for the
        // coordinate system in patterns.
        let bbox = compute_stroke_and_fill_box(cr, values);

        let current_color = values.color.0;

        let res = self
            .set_source_paint_server(&values.fill.0, values.fill_opacity.0, &bbox, current_color)
            .and_then(|had_paint_server| {
                if had_paint_server {
                    if values.stroke.0 == PaintServer::None {
                        cr.fill();
                    } else {
                        cr.fill_preserve();
                    }
                }

                Ok(())
            })
            .and_then(|_| {
                self.set_source_paint_server(
                    &values.stroke.0,
                    values.stroke_opacity.0,
                    &bbox,
                    current_color,
                )
                .and_then(|had_paint_server| {
                    if had_paint_server {
                        cr.stroke();
                    }
                    Ok(())
                })
            });

        // clear the path in case stroke == fill == None; otherwise
        // we leave it around from computing the bounding box
        cr.new_path();

        res.and_then(|_: ()| Ok(bbox))
    }

    pub fn clip(&self, rect: Rect) {
        let cr = self.get_cairo_context();
        cr.rectangle(rect.x0, rect.y0, rect.width(), rect.height());
        cr.clip();
    }

    pub fn get_snapshot(&self, surface: &cairo::ImageSurface) {
        // TODO: as far as I can tell this should not render elements past the last (topmost) one
        // with enable-background: new (because technically we shouldn't have been caching them).
        // Right now there are no enable-background checks whatsoever.
        //
        // Addendum: SVG 2 has deprecated the enable-background property, and replaced it with an
        // "isolation" property from the CSS Compositing and Blending spec.
        //
        // Deprecation:
        //   https://www.w3.org/TR/filter-effects-1/#AccessBackgroundImage
        //
        // BackgroundImage, BackgroundAlpha in the "in" attribute of filter primitives:
        //   https://www.w3.org/TR/filter-effects-1/#attr-valuedef-in-backgroundimage
        //
        // CSS Compositing and Blending, "isolation" property:
        //   https://www.w3.org/TR/compositing-1/#isolation
        let cr = cairo::Context::new(&surface);
        for (depth, draw) in self.cr_stack.iter().enumerate() {
            let affines = CompositingAffines::new(
                draw.get_matrix(),
                self.initial_affine_with_offset(),
                depth,
            );

            cr.set_matrix(affines.for_snapshot);
            cr.set_source_surface(&draw.get_target(), 0.0, 0.0);
            cr.paint();
        }
    }

    pub fn lookup_image(&self, href: &str) -> Result<SharedImageSurface, RenderingError> {
        self.document
            .lookup_image(href)
            .map_err(|_| RenderingError::InvalidHref)
    }

    pub fn draw_node_on_surface(
        &mut self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        surface: &cairo::ImageSurface,
        affine: cairo::Matrix,
        width: f64,
        height: f64,
    ) -> Result<BoundingBox, RenderingError> {
        let save_cr = self.cr.clone();
        let save_rect = self.rect;

        let cr = cairo::Context::new(surface);
        cr.set_matrix(affine);

        self.cr = cr;
        self.rect = Rect::from_size(width, height);

        let res = self.draw_node_from_stack(cascaded, node, false);

        self.cr = save_cr;
        self.rect = save_rect;

        res
    }

    pub fn draw_node_from_stack(
        &mut self,
        cascaded: &CascadedValues<'_>,
        node: &RsvgNode,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let stack_top = self.drawsub_stack.pop();

        let draw = if let Some(ref top) = stack_top {
            top == node
        } else {
            true
        };

        let values = cascaded.get();
        let res = if draw && values.is_visible() {
            node.draw(cascaded, self, clipping)
        } else {
            Ok(self.empty_bbox())
        };

        if let Some(top) = stack_top {
            self.drawsub_stack.push(top);
        }

        res
    }

    pub fn add_node_and_ancestors_to_stack(&mut self, node: &RsvgNode) {
        self.drawsub_stack.push(node.clone());
        if let Some(ref parent) = node.parent() {
            self.add_node_and_ancestors_to_stack(parent);
        }
    }
}

#[derive(Debug)]
pub struct CompositingAffines {
    pub outside_temporary_surface: cairo::Matrix,
    pub initial: cairo::Matrix,
    pub for_temporary_surface: cairo::Matrix,
    pub compositing: cairo::Matrix,
    pub for_snapshot: cairo::Matrix,
}

impl CompositingAffines {
    fn new(
        current: cairo::Matrix,
        initial: cairo::Matrix,
        cr_stack_depth: usize,
    ) -> CompositingAffines {
        let is_topmost_temporary_surface = cr_stack_depth == 0;

        let initial_inverse = initial.try_invert().unwrap();

        let outside_temporary_surface = if is_topmost_temporary_surface {
            current
        } else {
            cairo::Matrix::multiply(&current, &initial_inverse)
        };

        let (scale_x, scale_y) = initial.transform_distance(1.0, 1.0);

        let for_temporary_surface = if is_topmost_temporary_surface {
            let untransformed = cairo::Matrix::multiply(&current, &initial_inverse);
            let scale = cairo::Matrix::new(scale_x, 0.0, 0.0, scale_y, 0.0, 0.0);
            cairo::Matrix::multiply(&untransformed, &scale)
        } else {
            current
        };

        let compositing = if is_topmost_temporary_surface {
            let mut scaled = initial;
            scaled.scale(1.0 / scale_x, 1.0 / scale_y);
            scaled
        } else {
            cairo::Matrix::identity()
        };

        let for_snapshot = compositing.try_invert().unwrap();

        CompositingAffines {
            outside_temporary_surface,
            initial,
            for_temporary_surface,
            compositing,
            for_snapshot,
        }
    }
}

fn compute_stroke_and_fill_box(cr: &cairo::Context, values: &ComputedValues) -> BoundingBox {
    let affine = cr.get_matrix();

    let mut bbox = BoundingBox::new(&affine);

    // Dropping the precision of cairo's bezier subdivision, yielding 2x
    // _rendering_ time speedups, are these rather expensive operations
    // really needed here? */
    let backup_tolerance = cr.get_tolerance();
    cr.set_tolerance(1.0);

    // Bounding box for fill
    //
    // Unlike the case for stroke, for fills we always compute the bounding box.
    // In GNOME we have SVGs for symbolic icons where each icon has a bounding
    // rectangle with no fill and no stroke, and inside it there are the actual
    // paths for the icon's shape.  We need to be able to compute the bounding
    // rectangle's extents, even when it has no fill nor stroke.

    let (x0, y0, x1, y1) = cr.fill_extents();
    let fb = BoundingBox::new(&affine).with_ink_rect(Rect::new(x0, y0, x1, y1));
    bbox.insert(&fb);

    // Bounding box for stroke

    if values.stroke.0 != PaintServer::None {
        let (x0, y0, x1, y1) = cr.stroke_extents();
        let sb = BoundingBox::new(&affine).with_ink_rect(Rect::new(x0, y0, x1, y1));
        bbox.insert(&sb);
    }

    // objectBoundingBox

    let (x0, y0, x1, y1) = cr.path_extents();
    let ob = BoundingBox::new(&affine).with_rect(Rect::new(x0, y0, x1, y1));
    bbox.insert(&ob);

    // restore tolerance

    cr.set_tolerance(backup_tolerance);

    bbox
}

impl From<StrokeLinejoin> for cairo::LineJoin {
    fn from(j: StrokeLinejoin) -> cairo::LineJoin {
        match j {
            StrokeLinejoin::Miter => cairo::LineJoin::Miter,
            StrokeLinejoin::Round => cairo::LineJoin::Round,
            StrokeLinejoin::Bevel => cairo::LineJoin::Bevel,
        }
    }
}

impl From<StrokeLinecap> for cairo::LineCap {
    fn from(j: StrokeLinecap) -> cairo::LineCap {
        match j {
            StrokeLinecap::Butt => cairo::LineCap::Butt,
            StrokeLinecap::Round => cairo::LineCap::Round,
            StrokeLinecap::Square => cairo::LineCap::Square,
        }
    }
}

impl From<ClipRule> for cairo::FillRule {
    fn from(c: ClipRule) -> cairo::FillRule {
        match c {
            ClipRule::NonZero => cairo::FillRule::Winding,
            ClipRule::EvenOdd => cairo::FillRule::EvenOdd,
        }
    }
}

impl From<FillRule> for cairo::FillRule {
    fn from(f: FillRule) -> cairo::FillRule {
        match f {
            FillRule::NonZero => cairo::FillRule::Winding,
            FillRule::EvenOdd => cairo::FillRule::EvenOdd,
        }
    }
}

impl From<ShapeRendering> for cairo::Antialias {
    fn from(sr: ShapeRendering) -> cairo::Antialias {
        match sr {
            ShapeRendering::Auto | ShapeRendering::GeometricPrecision => cairo::Antialias::Default,
            ShapeRendering::OptimizeSpeed | ShapeRendering::CrispEdges => cairo::Antialias::None,
        }
    }
}

pub struct AcquiredNode {
    stack: Option<Rc<RefCell<NodeStack>>>,
    node: RsvgNode,
}

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        if let Some(ref stack) = self.stack {
            let mut stack = stack.borrow_mut();
            let last = stack.pop().unwrap();
            assert!(last == self.node);
        }
    }
}

impl AcquiredNode {
    pub fn get(&self) -> &RsvgNode {
        &self.node
    }
}

struct AcquiredNodes {
    document: Rc<Document>,
    node_stack: Rc<RefCell<NodeStack>>,
}

impl AcquiredNodes {
    fn new(document: Rc<Document>) -> AcquiredNodes {
        AcquiredNodes {
            document,
            node_stack: Rc::new(RefCell::new(NodeStack::new())),
        }
    }

    fn lookup_node(
        &self,
        fragment: &Fragment,
        node_types: &[NodeType],
    ) -> Result<RsvgNode, AcquireError> {
        let node = self.document.lookup(fragment).map_err(|_| {
            // FIXME: callers shouldn't have to know that get_node() can initiate a file load.
            // Maybe we should have the following stages:
            //   - load main SVG XML
            //
            //   - load secondary SVG XML and other files like images;
            //     all document::Resources and document::Images loaded
            //
            //   - Now that all files are loaded, resolve URL references
            AcquireError::LinkNotFound(fragment.clone())
        })?;

        if node_types.is_empty() {
            Ok(node)
        } else {
            let node_type = node.borrow().get_type();
            if node_types.iter().find(|&&t| t == node_type).is_some() {
                Ok(node)
            } else {
                Err(AcquireError::InvalidLinkType(fragment.clone()))
            }
        }
    }

    fn acquire(
        &self,
        fragment: &Fragment,
        node_types: &[NodeType],
    ) -> Result<AcquiredNode, AcquireError> {
        let node = self.lookup_node(fragment, node_types)?;

        if node_is_accessed_by_reference(&node) {
            self.push_node_ref(&node)
        } else {
            Ok(AcquiredNode {
                stack: None,
                node: node.clone(),
            })
        }
    }

    fn push_node_ref(&self, node: &RsvgNode) -> Result<AcquiredNode, AcquireError> {
        if self.node_stack.borrow().contains(&node) {
            Err(AcquireError::CircularReference(node.clone()))
        } else {
            self.node_stack.borrow_mut().push(&node);
            Ok(AcquiredNode {
                stack: Some(self.node_stack.clone()),
                node: node.clone(),
            })
        }
    }
}

// Returns whether a node of a particular type is only accessed by reference
// from other nodes' atributes.  The node could in turn cause other nodes
// to get referenced, potentially causing reference cycles.
fn node_is_accessed_by_reference(node: &RsvgNode) -> bool {
    use NodeType::*;

    match node.borrow().get_type() {
        ClipPath | Filter | LinearGradient | Marker | Mask | Pattern | RadialGradient => true,

        _ => false,
    }
}

/// Keeps a stack of nodes and can check if a certain node is contained in the stack
///
/// Sometimes parts of the code cannot plainly use the implicit stack of acquired
/// nodes as maintained by DrawingCtx::acquire_node(), and they must keep their
/// own stack of nodes to test for reference cycles.  NodeStack can be used to do that.
pub struct NodeStack(Vec<RsvgNode>);

impl NodeStack {
    pub fn new() -> NodeStack {
        NodeStack(Vec::new())
    }

    pub fn push(&mut self, node: &RsvgNode) {
        self.0.push(node.clone());
    }

    pub fn pop(&mut self) -> Option<RsvgNode> {
        self.0.pop()
    }

    pub fn contains(&self, node: &RsvgNode) -> bool {
        self.0.iter().find(|n| **n == *node).is_some()
    }
}
