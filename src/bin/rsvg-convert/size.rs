#[derive(Debug)]
pub struct Dpi {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Scale {
    pub x: f64,
    pub y: f64,
}

impl Scale {
    #[allow(clippy::float_cmp)]
    pub fn is_identity(&self) -> bool {
        self.x == 1.0 && self.y == 1.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

impl Size {
    pub fn new(w: f64, h: f64) -> Self {
        Self { w, h }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ResizeStrategy {
    Scale(Scale),
    Fit(u32, u32),
    FitWidth(u32),
    FitHeight(u32),
    FitLargestScale(Scale, Option<u32>, Option<u32>),
}

impl ResizeStrategy {
    pub fn apply(self, input: Size, keep_aspect_ratio: bool) -> Result<Size, ()> {
        if input.w == 0.0 && input.h == 0.0 {
            return Err(());
        }

        let output = match self {
            ResizeStrategy::Scale(s) => Size {
                w: input.w * s.x,
                h: input.h * s.y,
            },
            ResizeStrategy::Fit(w, h) => Size {
                w: f64::from(w),
                h: f64::from(h),
            },
            ResizeStrategy::FitWidth(w) => Size {
                w: f64::from(w),
                h: input.h * f64::from(w) / input.w,
            },
            ResizeStrategy::FitHeight(h) => Size {
                w: input.w * f64::from(h) / input.h,
                h: f64::from(h),
            },
            ResizeStrategy::FitLargestScale(s, w, h) => {
                let scaled_input_w = input.w * s.x;
                let scaled_input_h = input.h * s.y;

                let f = match (w.map(f64::from), h.map(f64::from)) {
                    (Some(w), Some(h)) if w < scaled_input_w || h < scaled_input_h => {
                        let sx = w / scaled_input_w;
                        let sy = h / scaled_input_h;
                        if sx > sy {
                            sy
                        } else {
                            sx
                        }
                    }
                    (Some(w), None) if w < scaled_input_w => w / scaled_input_w,
                    (None, Some(h)) if h < scaled_input_h => h / scaled_input_h,
                    _ => 1.0,
                };

                Size {
                    w: input.w * f * s.x,
                    h: input.h * f * s.y,
                }
            }
        };

        if !keep_aspect_ratio {
            return Ok(output);
        }

        if output.w < output.h {
            Ok(Size {
                w: output.w,
                h: input.h * (output.w / input.w),
            })
        } else {
            Ok(Size {
                w: input.w * (output.h / input.h),
                h: output.h,
            })
        }
    }
}
