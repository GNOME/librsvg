use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::aspect_ratio::AspectRatio;
use crate::document::{AcquiredNodes, NodeId};
use crate::drawing_ctx::DrawingCtx;
use crate::element::{set_attribute, ElementTrait};
use crate::href::{is_href, set_href};
use crate::node::{CascadedValues, Node};
use crate::parsers::ParseValue;
use crate::properties::ComputedValues;
use crate::rect::Rect;
use crate::rsvg_log;
use crate::session::Session;
use crate::surface_utils::shared_surface::{Interpolation, SharedImageSurface};
use crate::viewbox::ViewBox;
use crate::xml::Attributes;

use super::bounds::{Bounds, BoundsBuilder};
use super::context::{FilterContext, FilterOutput};
use super::{
    FilterEffect, FilterError, FilterResolveError, Primitive, PrimitiveParams, ResolvedPrimitive,
};

/// The `feImage` filter primitive.
#[derive(Default)]
pub struct FeImage {
    base: Primitive,
    params: ImageParams,
}

#[derive(Clone, Default)]
struct ImageParams {
    aspect: AspectRatio,
    href: Option<String>,
}

/// Resolved `feImage` primitive for rendering.
pub struct Image {
    aspect: AspectRatio,
    source: Source,
    feimage_values: Box<ComputedValues>,
}

/// What a feImage references for rendering.
enum Source {
    /// Nothing is referenced; ignore the filter.
    None,

    /// Reference to a node.
    Node(Node, String),

    /// Reference to an external image.  This is just a URL.
    ExternalImage(String),
}

impl Image {
    /// Renders the filter if the source is an existing node.
    fn render_node(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
        bounds: Rect,
        referenced_node: &Node,
    ) -> Result<SharedImageSurface, FilterError> {
        // https://www.w3.org/TR/filter-effects/#feImageElement
        //
        // The filters spec says, "... otherwise [rendering a referenced object], the
        // referenced resource is rendered according to the behavior of the use element."
        // I think this means that we use the same cascading mode as <use>, i.e. the
        // referenced object inherits its properties from the feImage element.
        let cascaded =
            CascadedValues::new_from_values(referenced_node, &self.feimage_values, None, None);

        let interpolation = Interpolation::from(self.feimage_values.image_rendering());

        let image = draw_ctx.draw_node_to_surface(
            referenced_node,
            acquired_nodes,
            &cascaded,
            ctx.paffine(),
            ctx.source_graphic().width(),
            ctx.source_graphic().height(),
        )?;

        let surface = ctx
            .source_graphic()
            .paint_image(bounds, &image, None, interpolation)?;

        Ok(surface)
    }

    /// Renders the filter if the source is an external image.
    fn render_external_image(
        &self,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        _draw_ctx: &DrawingCtx,
        bounds: &Bounds,
        url: &str,
    ) -> Result<SharedImageSurface, FilterError> {
        // FIXME: translate the error better here
        let image = acquired_nodes
            .lookup_image(url)
            .map_err(|_| FilterError::InvalidInput)?;

        let rect = self.aspect.compute(
            &ViewBox::from(Rect::from_size(
                f64::from(image.width()),
                f64::from(image.height()),
            )),
            &bounds.unclipped,
        );

        let interpolation = Interpolation::from(self.feimage_values.image_rendering());

        let surface =
            ctx.source_graphic()
                .paint_image(bounds.clipped, &image, Some(rect), interpolation)?;

        Ok(surface)
    }
}

impl ElementTrait for FeImage {
    fn set_attributes(&mut self, attrs: &Attributes, session: &Session) {
        self.base.parse_no_inputs(attrs, session);

        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "preserveAspectRatio") => {
                    set_attribute(&mut self.params.aspect, attr.parse(value), session);
                }

                // "path" is used by some older Adobe Illustrator versions
                ref a if is_href(a) || *a == expanded_name!("", "path") => {
                    set_href(a, &mut self.params.href, Some(value.to_string()));
                }

                _ => (),
            }
        }
    }
}

impl Image {
    pub fn render(
        &self,
        bounds_builder: BoundsBuilder,
        ctx: &FilterContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<FilterOutput, FilterError> {
        let bounds = bounds_builder.compute(ctx);

        let surface = match &self.source {
            Source::None => return Err(FilterError::InvalidInput),

            Source::Node(node, ref name) => {
                if let Ok(acquired) = acquired_nodes.acquire_ref(node) {
                    rsvg_log!(draw_ctx.session(), "(feImage \"{}\"", name);
                    let res = self.render_node(
                        ctx,
                        acquired_nodes,
                        draw_ctx,
                        bounds.clipped,
                        acquired.get(),
                    );
                    rsvg_log!(draw_ctx.session(), ")");
                    res?
                } else {
                    return Err(FilterError::InvalidInput);
                }
            }

            Source::ExternalImage(ref href) => {
                self.render_external_image(ctx, acquired_nodes, draw_ctx, &bounds, href)?
            }
        };

        Ok(FilterOutput {
            surface,
            bounds: bounds.clipped.into(),
        })
    }
}

impl FilterEffect for FeImage {
    fn resolve(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        node: &Node,
    ) -> Result<Vec<ResolvedPrimitive>, FilterResolveError> {
        let cascaded = CascadedValues::new_from_node(node);
        let feimage_values = cascaded.get().clone();

        let source = match self.params.href {
            None => Source::None,

            Some(ref s) => {
                if let Ok(node_id) = NodeId::parse(s) {
                    acquired_nodes
                        .acquire(&node_id)
                        .map(|acquired| Source::Node(acquired.get().clone(), s.clone()))
                        .unwrap_or(Source::None)
                } else {
                    Source::ExternalImage(s.to_string())
                }
            }
        };

        Ok(vec![ResolvedPrimitive {
            primitive: self.base.clone(),
            params: PrimitiveParams::Image(Image {
                aspect: self.params.aspect,
                source,
                feimage_values: Box::new(feimage_values),
            }),
        }])
    }
}
