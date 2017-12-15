use libc;
use glib::translate::*;
use pango::{self, ContextExt, LayoutExt};
use pango_sys;

use drawing_ctx::{self, RsvgDrawingCtx};
use state::{self, UnicodeBidi};

// FIXME: should the pango crate provide this like PANGO_GRAVITY_IS_VERTICAL() / PANGO_GRAVITY_IS_IMPROPER()?
fn gravity_is_vertical(gravity: pango::Gravity) -> bool {
    match gravity {
        pango::Gravity::East | pango::Gravity::West => true,
        _ => false
    }
}

fn to_pango_units(v: f64) -> i32 {
    (v * pango::SCALE as f64) as i32
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
        },

        _ => ()
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
    font_desc.set_size(to_pango_units(drawing_ctx::get_normalized_font_size(draw_ctx) / dpi_y * 72.0));

    let layout = pango::Layout::new(&pango_context);
    layout.set_font_description(&font_desc);

    let attr_list = pango::AttrList::new();

    attr_list.insert(
        pango::Attribute::new_letter_spacing(
            to_pango_units(state::get_letter_spacing(state).normalize(draw_ctx))
        ).unwrap());

    if let Some(font_decor) = state::get_font_decor(state) {
        if font_decor.underline {
            attr_list.insert(pango::Attribute::new_underline(pango::Underline::Single)
                             .unwrap());
        }

        if font_decor.strike {
            attr_list.insert(pango::Attribute::new_strikethrough(true)
                             .unwrap());
        }
    }

    layout.set_attributes(&attr_list);

    layout.set_alignment(
        match state::get_text_dir(state) {
            pango::Direction::Ltr => pango::Alignment::Left,
            _                     => pango::Alignment::Right,
        }
    );

    layout.set_text(text);

    layout
}

#[no_mangle]
pub extern fn rsvg_text_create_layout(draw_ctx: *const RsvgDrawingCtx,
                                      text: *const libc::c_char) -> *const pango_sys::PangoLayout {
    assert!(!text.is_null());
    let s = unsafe { String::from_glib_none(text) };
    let layout = create_pango_layout(draw_ctx, &s);

    layout.to_glib_full()
}
