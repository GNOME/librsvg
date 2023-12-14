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

fn geometry_for_layer(
    session: &Session,
    document: &Document,
    node: Node,
    viewport: Rect,
    user_language: &UserLanguage,
    dpi: Dpi,
    is_testing: bool,
) -> Result<(Rect, Rect), InternalRenderingError> {
    let root = document.root();

    let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
    let cr = cairo::Context::new(&target)?;

    let bbox = draw_tree(
        session.clone(),
        DrawingMode::LimitToStack { node, root },
        &cr,
        viewport,
        user_language,
        dpi,
        SvgNesting::Standalone,
        true,
        is_testing,
        &mut AcquiredNodes::new(document),
    )?;

    let ink_rect = bbox.ink_rect.unwrap_or_default();
    let logical_rect = bbox.rect.unwrap_or_default();

    Ok((ink_rect, logical_rect))
}

pub fn get_geometry_for_layer(
    session: &Session,
    document: &Document,
    node: Node,
    viewport: &cairo::Rectangle,
    user_language: &UserLanguage,
    dpi: Dpi,
    is_testing: bool,
) -> Result<(cairo::Rectangle, cairo::Rectangle), InternalRenderingError> {
    let viewport = Rect::from(*viewport);

    let (ink_rect, logical_rect) = geometry_for_layer(
        session,
        document,
        node,
        viewport,
        user_language,
        dpi,
        is_testing,
    )?;

    Ok((
        cairo::Rectangle::from(ink_rect),
        cairo::Rectangle::from(logical_rect),
    ))
}

fn get_bbox_for_element(
    session: &Session,
    document: &Document,
    node: &Node,
    user_language: &UserLanguage,
    dpi: Dpi,
    is_testing: bool,
) -> Result<BoundingBox, InternalRenderingError> {
    let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
    let cr = cairo::Context::new(&target)?;

    let node = node.clone();

    draw_tree(
        session.clone(),
        DrawingMode::OnlyNode(node),
        &cr,
        unit_rectangle(),
        user_language,
        dpi,
        SvgNesting::Standalone,
        true,
        is_testing,
        &mut AcquiredNodes::new(document),
    )
}

/// Returns (ink_rect, logical_rect)
pub fn get_geometry_for_element(
    session: &Session,
    document: &Document,
    node: Node,
    user_language: &UserLanguage,
    dpi: Dpi,
    is_testing: bool,
) -> Result<(cairo::Rectangle, cairo::Rectangle), InternalRenderingError> {
    let bbox = get_bbox_for_element(session, document, &node, user_language, dpi, is_testing)?;

    let ink_rect = bbox.ink_rect.unwrap_or_default();
    let logical_rect = bbox.rect.unwrap_or_default();

    // Translate so ink_rect is always at offset (0, 0)
    let ofs = (-ink_rect.x0, -ink_rect.y0);

    Ok((
        cairo::Rectangle::from(ink_rect.translate(ofs)),
        cairo::Rectangle::from(logical_rect.translate(ofs)),
    ))
}

pub fn render_element(
    session: &Session,
    document: &Document,
    cr: &cairo::Context,
    node: Node,
    element_viewport: &cairo::Rectangle,
    user_language: &UserLanguage,
    dpi: Dpi,
    is_testing: bool,
) -> Result<(), InternalRenderingError> {
    cr.status()?;

    let bbox = get_bbox_for_element(session, document, &node, user_language, dpi, is_testing)?;

    if bbox.ink_rect.is_none() || bbox.rect.is_none() {
        // Nothing to draw
        return Ok(());
    }

    let ink_r = bbox.ink_rect.unwrap_or_default();

    if ink_r.is_empty() {
        return Ok(());
    }

    // Render, transforming so element is at the new viewport's origin

    with_saved_cr(cr, || {
        let factor = (element_viewport.width() / ink_r.width())
            .min(element_viewport.height() / ink_r.height());

        cr.translate(element_viewport.x(), element_viewport.y());
        cr.scale(factor, factor);
        cr.translate(-ink_r.x0, -ink_r.y0);

        draw_tree(
            session.clone(),
            DrawingMode::OnlyNode(node),
            cr,
            unit_rectangle(),
            user_language,
            dpi,
            SvgNesting::Standalone,
            false,
            is_testing,
            &mut AcquiredNodes::new(document),
        )
        .map(|_bbox| ())
    })
}

fn unit_rectangle() -> Rect {
    Rect::from_size(1.0, 1.0)
}
