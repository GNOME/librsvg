extern crate libc;
extern crate cairo;
extern crate cairo_sys;
extern crate glib;

use self::glib::translate::*;
use self::cairo::Pattern;

use length::*;

use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;

use bbox::*;

use self::cairo::MatrixTrait;

#[derive(Copy, Clone)]
pub struct ColorStop {
    pub offset: f64,
    pub rgba:   u32
}

/* Any of the attributes in gradient elements may be omitted.  In turn, the missing
 * ones can be inherited from the gradient referenced by its "fallback" IRI.  We
 * represent these possibly-missing attributes as Option<foo>.
 */
pub struct GradientCommon {
    pub obj_bbox: Option<bool>,
    pub affine:   Option<cairo::Matrix>,
    pub spread:   Option<cairo::enums::Extend>,
    pub fallback: Option<String>,
    pub stops:    Option<Vec<ColorStop>>
}

#[derive(Copy, Clone)]
pub enum GradientVariant {
    Linear {
        x1: Option<RsvgLength>,
        y1: Option<RsvgLength>,
        x2: Option<RsvgLength>,
        y2: Option<RsvgLength>
    },

    Radial {
        cx: Option<RsvgLength>,
        cy: Option<RsvgLength>,
        r:  Option<RsvgLength>,
        fx: Option<RsvgLength>,
        fy: Option<RsvgLength>,
    }
}

pub struct Gradient {
    pub common: GradientCommon,
    pub variant: GradientVariant
}

impl GradientCommon {
    fn new (obj_bbox: Option<bool>,
            affine:   Option<cairo::Matrix>,
            spread:   Option<cairo::enums::Extend>,
            fallback: Option<String>,
            stops:    Option<Vec<ColorStop>>) -> GradientCommon {
        GradientCommon {
            obj_bbox: obj_bbox,
            affine:   affine,
            spread:   spread,
            fallback: fallback,
            stops:    stops
        }
    }

    fn clone_stops (&self) -> Option<Vec<ColorStop>> {
        if let Some (ref stops) = self.stops {
            Some (stops.clone ())
        } else {
            None
        }
    }

    fn is_resolved (&self) -> bool {
        self.obj_bbox.is_some() &&
            self.affine.is_some () &&
            self.spread.is_some () &&
            self.stops.is_some ()
    }

    fn resolve_from_defaults (&mut self) {
        /* These are per the spec */

        if self.obj_bbox.is_none () { self.obj_bbox = Some (true); }
        if self.affine.is_none ()   { self.affine   = Some (cairo::Matrix::identity ()); }
        if self.spread.is_none ()   { self.spread   = Some (cairo::enums::Extend::Pad); }

        self.fallback = None;

        if self.stops.is_none ()    { self.stops    = Some (Vec::<ColorStop>::new ()); } // empty array of color stops
    }

    fn resolve_from_fallback (&mut self, fallback: &GradientCommon) {
        if self.obj_bbox.is_none () { self.obj_bbox = fallback.obj_bbox; }
        if self.affine.is_none ()   { self.affine   = fallback.affine; }
        if self.spread.is_none ()   { self.spread   = fallback.spread; }
        if self.stops.is_none ()    { self.stops    = fallback.clone_stops (); }

        if self.fallback.is_none () {
            self.fallback = clone_fallback_name (&fallback.fallback);
        }
    }

    fn add_color_stop (&mut self, mut offset: f64, rgba: u32) {
        if self.stops.is_none () {
            self.stops = Some (Vec::<ColorStop>::new ());
        }

        if let Some (ref mut stops) = self.stops {
            let mut last_offset: f64 = 0.0;

            if stops.len () > 0 {
                last_offset = stops[stops.len () - 1].offset;
            }

            if last_offset > offset {
                offset = last_offset;
            }

            stops.push (ColorStop { offset: offset,
                                    rgba:   rgba });
        } else {
            unreachable! ();
        }
    }
}

fn clone_fallback_name (fallback: &Option<String>) -> Option<String> {
    if let Some (ref fallback_name) = *fallback {
        Some (fallback_name.clone ())
    } else {
        None
    }
}

impl GradientVariant {
    fn is_resolved (&self) -> bool {
        match *self {
            GradientVariant::Linear { x1, y1, x2, y2 } => {
                x1.is_some () &&
                    y1.is_some () &&
                    x2.is_some () &&
                    y2.is_some ()
            },

            GradientVariant::Radial { cx, cy, r, fx, fy } => {
                cx.is_some () &&
                    cy.is_some () &&
                    r.is_some () &&
                    fx.is_some () &&
                    fy.is_some ()
            }
        }
    }

    fn resolve_from_defaults (&mut self) {
        /* These are per the spec */

        match *self {
            GradientVariant::Linear { ref mut x1, ref mut y1, ref mut x2, ref mut y2 } => {
                if x1.is_none () { *x1 = Some (RsvgLength::parse ("0%", LengthDir::Horizontal)); }
                if y1.is_none () { *y1 = Some (RsvgLength::parse ("0%", LengthDir::Vertical)); }
                if x2.is_none () { *x2 = Some (RsvgLength::parse ("100%", LengthDir::Horizontal)); }
                if y2.is_none () { *y2 = Some (RsvgLength::parse ("0%", LengthDir::Vertical)); }
            },

            GradientVariant::Radial { ref mut cx, ref mut cy, ref mut r, ref mut fx, ref mut fy } => {
                if cx.is_none () { *cx = Some (RsvgLength::parse ("50%", LengthDir::Horizontal)); }
                if cy.is_none () { *cy = Some (RsvgLength::parse ("50%", LengthDir::Vertical)); }
                if r.is_none ()  { *r  = Some (RsvgLength::parse ("50%", LengthDir::Both)); }

                /* fx and fy fall back to the presentational value of cx and cy */
                if fx.is_none () { *fx = *cx }
                if fy.is_none () { *fy = *cy }
            }
        }
    }

    fn resolve_from_fallback (&mut self, fallback: &GradientVariant) {
        match *self {
            GradientVariant::Linear { ref mut x1, ref mut y1, ref mut x2, ref mut y2 } => {
                if let &GradientVariant::Linear { x1: x1f, y1: y1f, x2: x2f, y2: y2f } = fallback {
                    if x1.is_none () { *x1 = x1f; }
                    if y1.is_none () { *y1 = y1f; }
                    if x2.is_none () { *x2 = x2f; }
                    if y2.is_none () { *y2 = y2f; }
                }
            },

            GradientVariant::Radial { ref mut cx, ref mut cy, ref mut r, ref mut fx, ref mut fy } => {
                if let &GradientVariant::Radial { cx: cxf, cy: cyf, r: rf, fx: fxf, fy: fyf } = fallback {
                    if cx.is_none () { *cx = cxf; }
                    if cy.is_none () { *cy = cyf; }
                    if r.is_none ()  { *r  = rf;  }
                    if fx.is_none () { *fx = fxf; }
                    if fy.is_none () { *fy = fyf; }
                }
            }
        }
    }
}

impl Gradient {
    fn new (common: GradientCommon, variant: GradientVariant) -> Gradient {
        Gradient {
            common: common,
            variant: variant
        }
    }

    fn is_resolved (&self) -> bool {
        self.common.is_resolved () && self.variant.is_resolved ()
    }

    fn resolve_from_defaults (&mut self) {
        self.common.resolve_from_defaults ();
        self.variant.resolve_from_defaults ();
    }

    fn resolve_from_fallback (&mut self, fallback: &Gradient) {
        self.common.resolve_from_fallback (&fallback.common);
        self.variant.resolve_from_fallback (&fallback.variant);
    }

    fn add_color_stop (&mut self, offset: f64, rgba: u32) {
        self.common.add_color_stop (offset, rgba);
    }

    fn add_color_stops_to_pattern (&self,
                                   pattern:  &mut cairo::Gradient,
                                   opacity:  u8) {
        let stops = self.common.stops.as_ref ().unwrap ();

        for stop in stops {
            let rgba = stop.rgba;
            pattern.add_color_stop_rgba (stop.offset,
                                         ((rgba >> 24) & 0xff) as f64 / 255.0,
                                         ((rgba >> 16) & 0xff) as f64 / 255.0,
                                         ((rgba >> 8) & 0xff) as f64 / 255.0,
                                         (((rgba >> 0) & 0xff) * opacity as u32) as f64 / 255.0 / 255.0);
        }
    }
}

trait FallbackSource {
    fn get_fallback (&self, name: &str) -> Option<Gradient>;
}

fn resolve_gradient (gradient: &Gradient, fallback_source: &FallbackSource) -> Gradient {
    let mut result = Gradient::new (GradientCommon::new (gradient.common.obj_bbox,
                                                         gradient.common.affine,
                                                         gradient.common.spread,
                                                         clone_fallback_name (&gradient.common.fallback),
                                                         gradient.common.clone_stops ()),
                                    gradient.variant);

    while !result.is_resolved () {
        let mut opt_fallback: Option<Gradient> = None;

        if let Some (ref fallback_name) = result.common.fallback {
            opt_fallback = fallback_source.get_fallback (&**fallback_name);
        }

        if let Some (fallback_gradient) = opt_fallback {
            result.resolve_from_fallback (&fallback_gradient);
        } else {
            result.resolve_from_defaults ();
            break;
        }
    }

    result
}

fn set_common_on_pattern (gradient: &Gradient,
                          draw_ctx: &mut RsvgDrawingCtx,
                          pattern:  &mut cairo::LinearGradient,
                          bbox:     &RsvgBbox,
                          opacity:  u8)
{
    let cr = drawing_ctx::get_cairo_context (draw_ctx);

    let mut affine = gradient.common.affine.unwrap ();

    let obj_bbox = gradient.common.obj_bbox.unwrap ();

    if obj_bbox {
        let bbox_matrix = cairo::Matrix::new (bbox.rect.width, 0.0,
                                              0.0, bbox.rect.height,
                                              bbox.rect.x, bbox.rect.y);
        affine = cairo::Matrix::multiply (&affine, &bbox_matrix);
    }

    affine.invert ();
    pattern.set_matrix (affine);
    pattern.set_extend (gradient.common.spread.unwrap ());

    gradient.add_color_stops_to_pattern (pattern, opacity);

    cr.set_source (pattern);
}

fn set_linear_gradient_on_pattern (gradient: &Gradient,
                                   draw_ctx: &mut RsvgDrawingCtx,
                                   bbox:     &RsvgBbox,
                                   opacity:  u8)
{
    if let GradientVariant::Linear { x1, y1, x2, y2 } = gradient.variant {
        let obj_bbox = gradient.common.obj_bbox.unwrap ();

        if obj_bbox {
            drawing_ctx::push_view_box (draw_ctx, 1.0, 1.0);
        }

        let mut pattern = cairo::LinearGradient::new (x1.as_ref ().unwrap ().normalize (draw_ctx),
                                                      y1.as_ref ().unwrap ().normalize (draw_ctx),
                                                      x2.as_ref ().unwrap ().normalize (draw_ctx),
                                                      y2.as_ref ().unwrap ().normalize (draw_ctx));

        if obj_bbox {
            drawing_ctx::pop_view_box (draw_ctx);
        }

        set_common_on_pattern (gradient, draw_ctx, &mut pattern, bbox, opacity);
    } else {
        unreachable! ();
    }
}

fn set_radial_gradient_on_pattern (gradient: &Gradient,
                                   draw_ctx: &mut RsvgDrawingCtx,
                                   bbox:     &RsvgBbox,
                                   opacity:  u8) {
    unimplemented! ();
}

fn set_pattern_on_draw_context (gradient: &Gradient,
                                draw_ctx: &mut RsvgDrawingCtx,
                                bbox:     &RsvgBbox,
                                opacity:  u8) {
    assert! (gradient.is_resolved ());

    match gradient.variant {
        GradientVariant::Linear { .. } => {
            set_linear_gradient_on_pattern (gradient, draw_ctx, bbox, opacity);
        }

        GradientVariant::Radial { .. } => {
            set_radial_gradient_on_pattern (gradient, draw_ctx, bbox, opacity);
        }
    }


}


/* All the arguments are pointers because they are in fact optional in
 * SVG.  We turn the arguments into Option<foo>: NULL into None, and
 * anything else into a Some().
 */
#[no_mangle]
pub unsafe extern fn gradient_linear_new (x1: *const RsvgLength,
                                          y1: *const RsvgLength,
                                          x2: *const RsvgLength,
                                          y2: *const RsvgLength,
                                          obj_bbox: *const bool,
                                          affine: *const cairo::Matrix,
                                          spread: *const cairo::enums::Extend,
                                          fallback_name: *const libc::c_char) -> *mut Gradient {
    let my_obj_bbox      = { if obj_bbox.is_null ()      { None } else { Some (*obj_bbox) } };
    let my_affine        = { if affine.is_null ()        { None } else { Some (*affine) } };
    let my_spread        = { if spread.is_null ()        { None } else { Some (*spread) } };
    let my_fallback_name = { if fallback_name.is_null () { None } else { Some (String::from_glib_none (fallback_name)) } };

    let my_x1 = { if x1.is_null () { None } else { Some (*x1) } };
    let my_y1 = { if y1.is_null () { None } else { Some (*y1) } };
    let my_x2 = { if x2.is_null () { None } else { Some (*x2) } };
    let my_y2 = { if y2.is_null () { None } else { Some (*y2) } };

    let gradient = Gradient::new (GradientCommon::new (my_obj_bbox, my_affine, my_spread, my_fallback_name, None),
                                  GradientVariant::Linear { x1: my_x1,
                                                            y1: my_y1,
                                                            x2: my_x2,
                                                            y2: my_y2 });

    let boxed_gradient = Box::new (gradient);

    Box::into_raw (boxed_gradient)
}

#[no_mangle]
pub unsafe extern fn gradient_radial_new (cx: *const RsvgLength,
                                          cy: *const RsvgLength,
                                          r:  *const RsvgLength,
                                          fx: *const RsvgLength,
                                          fy: *const RsvgLength,
                                          obj_bbox: *const bool,
                                          affine: *const cairo::Matrix,
                                          spread: *const cairo::enums::Extend,
                                          fallback_name: *const libc::c_char) -> *mut Gradient {
    let my_obj_bbox      = { if obj_bbox.is_null ()      { None } else { Some (*obj_bbox) } };
    let my_affine        = { if affine.is_null ()        { None } else { Some (*affine) } };
    let my_spread        = { if spread.is_null ()        { None } else { Some (*spread) } };
    let my_fallback_name = { if fallback_name.is_null () { None } else { Some (String::from_glib_none (fallback_name)) } };

    let my_cx = { if cx.is_null () { None } else { Some (*cx) } };
    let my_cy = { if cy.is_null () { None } else { Some (*cy) } };
    let my_r  = { if r.is_null  () { None } else { Some (*r)  } };
    let my_fx = { if fx.is_null () { None } else { Some (*fx) } };
    let my_fy = { if fy.is_null () { None } else { Some (*fy) } };

    let gradient = Gradient::new (GradientCommon::new (my_obj_bbox, my_affine, my_spread, my_fallback_name, None),
                                  GradientVariant::Radial { cx: my_cx,
                                                            cy: my_cy,
                                                            r:  my_r,
                                                            fx: my_fx,
                                                            fy: my_fy });

    let boxed_gradient = Box::new (gradient);

    Box::into_raw (boxed_gradient)
}

#[no_mangle]
pub unsafe extern fn gradient_destroy (raw_gradient: *mut Gradient) {
    assert! (!raw_gradient.is_null ());

    let _ = Box::from_raw (raw_gradient);
}

#[no_mangle]
pub extern fn gradient_add_color_stop (raw_gradient: *mut Gradient,
                                       offset:       f64,
                                       rgba:         u32) {
    assert! (!raw_gradient.is_null ());

    let gradient: &mut Gradient = unsafe { &mut (*raw_gradient) };

    gradient.add_color_stop (offset, rgba);
}
