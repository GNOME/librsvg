use cairo;
use cairo::{MatrixTrait, PatternTrait};
use std::cell::{Cell, RefCell};

use allowed_url::Href;
use aspect_ratio::AspectRatio;
use attributes::Attribute;
use bbox::BoundingBox;
use drawing_ctx::DrawingCtx;
use error::{NodeError, RenderingError};
use float_eq_cairo::ApproxEqCairo;
use length::*;
use node::*;
use parsers::{ParseError, ParseValue};
use property_bag::PropertyBag;

pub struct NodeImage {
    aspect: Cell<AspectRatio>,
    x: Cell<Length>,
    y: Cell<Length>,
    w: Cell<Length>,
    h: Cell<Length>,
    href: RefCell<Option<Href>>,
}

impl NodeImage {
    pub fn new() -> NodeImage {
        NodeImage {
            aspect: Cell::new(AspectRatio::default()),
            x: Cell::new(Length::default()),
            y: Cell::new(Length::default()),
            w: Cell::new(Length::default()),
            h: Cell::new(Length::default()),
            href: RefCell::new(None),
        }
    }
}

impl NodeTrait for NodeImage {
    fn set_atts(&self, node: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        // SVG element has overflow:hidden
        // https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
        node.set_overflow_hidden();

        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(attr.parse(value, LengthDir::Horizontal)?),
                Attribute::Y => self.y.set(attr.parse(value, LengthDir::Vertical)?),
                Attribute::Width => self.w.set(attr.parse_and_validate(
                    value,
                    LengthDir::Horizontal,
                    Length::check_nonnegative,
                )?),
                Attribute::Height => self.h.set(attr.parse_and_validate(
                    value,
                    LengthDir::Vertical,
                    Length::check_nonnegative,
                )?),

                Attribute::PreserveAspectRatio => self.aspect.set(attr.parse(value, ())?),

                // "path" is used by some older Adobe Illustrator versions
                Attribute::XlinkHref | Attribute::Path => {
                    let href = Href::parse(value).map_err(|_| {
                        NodeError::parse_error(attr, ParseError::new("could not parse href"))
                    })?;

                    *self.href.borrow_mut() = Some(href);
                }

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let values = cascaded.get();
        let params = draw_ctx.get_view_params();

        let x = self.x.get().normalize(values, &params);
        let y = self.y.get().normalize(values, &params);
        let w = self.w.get().normalize(values, &params);
        let h = self.h.get().normalize(values, &params);

        if w.approx_eq_cairo(&0.0) || h.approx_eq_cairo(&0.0) {
            return Ok(());
        }

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            let surface = if let Some(Href::PlainUrl(ref url)) = *self.href.borrow() {
                dc.lookup_image(&url)?
            } else {
                return Ok(());
            };

            let aspect = self.aspect.get();

            if !values.is_overflow() && aspect.is_slice() {
                dc.clip(x, y, w, h);
            }

            let width = surface.width();
            let height = surface.height();
            if clipping || width == 0 || height == 0 {
                return Ok(());
            }

            // The bounding box for <image> is decided by the values of x, y, w, h and not by
            // the final computed image bounds.
            let bbox = BoundingBox::new(&dc.get_cairo_context().get_matrix()).with_rect(Some(
                cairo::Rectangle {
                    x,
                    y,
                    width: w,
                    height: h,
                },
            ));

            let width = f64::from(width);
            let height = f64::from(height);

            let (x, y, w, h) = aspect.compute(width, height, x, y, w, h);

            let cr = dc.get_cairo_context();

            cr.save();

            dc.set_affine_on_cr(&cr);
            cr.scale(w / width, h / height);
            let x = x * width / w;
            let y = y * height / h;

            // We need to set extend appropriately, so can't use cr.set_source_surface().
            //
            // If extend is left at its default value (None), then bilinear scaling uses
            // transparency outside of the image producing incorrect results.
            // For example, in svg1.1/filters-blend-01-b.svgthere's a completely
            // opaque 100×1 image of a gradient scaled to 100×98 which ends up
            // transparent almost everywhere without this fix (which it shouldn't).
            let ptn = surface.to_cairo_pattern();
            let mut matrix = cairo::Matrix::identity();
            matrix.translate(-x, -y);
            ptn.set_matrix(matrix);
            ptn.set_extend(cairo::Extend::Pad);
            cr.set_source(&ptn);

            // Clip is needed due to extend being set to pad.
            cr.rectangle(x, y, width, height);
            cr.clip();

            cr.paint();

            cr.restore();

            dc.insert_bbox(&bbox);
            Ok(())
        })
    }
}
