use libc;
use pango::{self, ContextExt, LayoutExt};

use drawing_ctx::{self, RsvgDrawingCtx};
use space::xml_space_normalize;
use state::{self, RsvgState, UnicodeBidi};
use util::utf8_cstr;

extern "C" {
    fn _rsvg_css_accumulate_baseline_shift(
        state: *mut RsvgState,
        draw_ctx: *const RsvgDrawingCtx,
    ) -> libc::c_double;
}

// FIXME: should the pango crate provide this like PANGO_GRAVITY_IS_VERTICAL() /
// PANGO_GRAVITY_IS_IMPROPER()?
pub fn gravity_is_vertical(gravity: pango::Gravity) -> bool {
    match gravity {
        pango::Gravity::East | pango::Gravity::West => true,
        _ => false,
    }
}

fn to_pango_units(v: f64) -> i32 {
    (v * f64::from(pango::SCALE)) as i32
}

fn create_pango_layout(draw_ctx: *const RsvgDrawingCtx, text: &str) -> pango::Layout {
    let state = drawing_ctx::get_current_state(draw_ctx);
    let pango_context = drawing_ctx::get_pango_context(draw_ctx);

    if let Some(lang) = state::get_language(state) {
        let pango_lang = pango::Language::from_string(&lang);
        pango_context.set_language(&pango_lang);
    }

    let unicode_bidi = state::get_unicode_bidi(state);
    match unicode_bidi {
        UnicodeBidi::Override | UnicodeBidi::Embed => {
            pango_context.set_base_dir(state::get_text_dir(state));
        }

        _ => (),
    }

    let gravity = state::get_text_gravity(state);
    if gravity_is_vertical(gravity) {
        pango_context.set_base_gravity(gravity);
    }

    let mut font_desc = pango_context.get_font_description().unwrap();

    if let Some(font_family) = state::get_font_family(state) {
        font_desc.set_family(&font_family);
    }

    font_desc.set_style(state::get_font_style(state));
    font_desc.set_variant(state::get_font_variant(state));
    font_desc.set_weight(state::get_font_weight(state));
    font_desc.set_stretch(state::get_font_stretch(state));

    let (_, dpi_y) = drawing_ctx::get_dpi(draw_ctx);
    font_desc.set_size(to_pango_units(
        drawing_ctx::get_normalized_font_size(draw_ctx) / dpi_y * 72.0,
    ));

    let layout = pango::Layout::new(&pango_context);
    layout.set_font_description(&font_desc);

    let attr_list = pango::AttrList::new();

    attr_list.insert(
        pango::Attribute::new_letter_spacing(to_pango_units(
            state::get_letter_spacing(state).normalize(draw_ctx),
        )).unwrap(),
    );

    if let Some(font_decor) = state::get_font_decor(state) {
        if font_decor.underline {
            attr_list.insert(pango::Attribute::new_underline(pango::Underline::Single).unwrap());
        }

        if font_decor.strike {
            attr_list.insert(pango::Attribute::new_strikethrough(true).unwrap());
        }
    }

    layout.set_attributes(&attr_list);

    layout.set_alignment(match state::get_text_dir(state) {
        pango::Direction::Ltr => pango::Alignment::Left,
        _ => pango::Alignment::Right,
    });

    let t = xml_space_normalize(state::get_xml_space(state), text);
    layout.set_text(&t);

    layout
}

fn measure_text(draw_ctx: *const RsvgDrawingCtx, text: &str) -> f64 {
    let layout = create_pango_layout(draw_ctx, text);
    let (width, _) = layout.get_size();

    f64::from(width) / f64::from(pango::SCALE)
}

fn render_text(draw_ctx: *const RsvgDrawingCtx, text: &str, x: &mut f64, y: &mut f64) {
    let state = drawing_ctx::get_current_state(draw_ctx);

    let layout = create_pango_layout(draw_ctx, text);
    let (width, _) = layout.get_size();
    let mut offset = f64::from(layout.get_baseline()) / f64::from(pango::SCALE);

    unsafe {
        offset += _rsvg_css_accumulate_baseline_shift(state, draw_ctx);
    }

    let gravity = state::get_text_gravity(state);
    if gravity_is_vertical(gravity) {
        drawing_ctx::render_pango_layout(draw_ctx, &layout, *x + offset, *y);
        *y += f64::from(width) / f64::from(pango::SCALE);
    } else {
        drawing_ctx::render_pango_layout(draw_ctx, &layout, *x, *y - offset);
        *x += f64::from(width) / f64::from(pango::SCALE);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_text_measure(
    draw_ctx: *const RsvgDrawingCtx,
    text: *const libc::c_char,
) -> libc::c_double {
    assert!(!text.is_null());
    let s = unsafe { utf8_cstr(text) };

    measure_text(draw_ctx, s)
}

#[no_mangle]
pub extern "C" fn rsvg_text_render(
    draw_ctx: *const RsvgDrawingCtx,
    text: *const libc::c_char,
    raw_x: *mut libc::c_double,
    raw_y: *mut libc::c_double,
) {
    assert!(!text.is_null());
    assert!(!raw_x.is_null());
    assert!(!raw_y.is_null());
    let s = unsafe { utf8_cstr(text) };
    let x: &mut f64 = unsafe { &mut *raw_x };
    let y: &mut f64 = unsafe { &mut *raw_y };

    render_text(draw_ctx, s, x, y)
}
