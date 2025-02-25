use cairo;
use pango;
use pango::prelude::*;
use pangocairo;

use std::fs::File;

fn main() {
    // Create a font map, in this case the default one which uses the system's fonts.
    let font_map = pangocairo::FontMap::default();

    // Create a pango context.  We set the resolution to 72 dots-per-inch so that the sizes we specify
    // later will be pixel sizes, instead of point sizes (Pango's API takes points).
    let pango_context = font_map.create_context();
    pangocairo::functions::context_set_resolution(&pango_context, 72.0);

    // Create a Cairo image surface for our output
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 800, 400).unwrap();
    let cr = cairo::Context::new(&surface).unwrap();
    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    cr.paint().unwrap();

    let layout = pango::Layout::new(&pango_context);
    fill_layout(&layout);

    // We will iterate the layout by hand
    // line
    //   runs
    //     clusters

    let mut iter = layout.iter();
    while let Some(_line) = iter.line_readonly() {
        while let Some(run) = iter.run_readonly() {
            let item = run.item();
            let glyph_string = run.glyph_string();

            println!(
                "item.offfset: {}, item.char_offset: {}",
                item.offset(),
                item.char_offset(),
            );

            println!("glyph_string num_glyphs: {}", glyph_string.num_glyphs(),);

            for glyph_info in glyph_string.glyph_info() {
                let geometry = glyph_info.geometry();

                println!(
                    "    glyph {} width={} x_offset={} y_offset={}",
                    glyph_info.glyph(),
                    geometry.width(),
                    geometry.x_offset(),
                    geometry.y_offset(),
                );
            }

            for glyph_info in glyph_string.glyph_info() {
                let geometry = glyph_info.geometry();

                println!(
                    "    glyph {} width={} x_offset={} y_offset={}",
                    glyph_info.glyph(),
                    geometry.width(),
                    geometry.x_offset(),
                    geometry.y_offset(),
                );
            }

            if !iter.next_run() {
                break;
            }
        }

        if !iter.next_line() {
            break;
        }
    }

    // Set the paint color to black; that's what pangocairo will use.
    cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
    cr.move_to(100.0, 100.0);
    pangocairo::functions::show_layout(&cr, &layout);

    // Write a PNG file from the surface
    let mut output = File::create("example_3_glyphs.png").unwrap();
    surface.write_to_png(&mut output).unwrap();
}

fn fill_layout(layout: &pango::Layout) {
    let text = "Hello böld world in itálics!";
    layout.set_text(&text);

    // Create an attribute list and an array of attributes to put in it
    let attr_list = pango::AttrList::new();

    // Font description and style; this spans the whole string
    let mut font_desc = pango::FontDescription::new();
    font_desc.set_family("Sans");
    font_desc.set_size(40 * pango::SCALE); // sizes in Pango are scaled by this factor
    let mut attr = pango::AttrFontDesc::new(&font_desc).upcast();
    attr.set_start_index(0);
    attr.set_end_index(text.len() as u32); // in bytes
    attr_list.insert(attr);

    // Make the word bold; note how we use byte offsets
    let mut font_desc = pango::FontDescription::new();
    font_desc.set_weight(pango::Weight::Bold);
    let mut attr = pango::AttrFontDesc::new(&font_desc).upcast();
    attr.set_start_index(text.find("böld").unwrap() as u32);
    attr.set_end_index((text.find("böld").unwrap() + "böld".len()) as u32);
    attr_list.insert(attr);

    // Make the two words italics; same as the previous one
    let mut font_desc = pango::FontDescription::new();
    font_desc.set_style(pango::Style::Italic);
    let mut attr = pango::AttrFontDesc::new(&font_desc).upcast();
    attr.set_start_index(text.find("in itálics").unwrap() as u32);
    attr.set_end_index((text.find("in itálics").unwrap() + "in itálics".len()) as u32);
    attr_list.insert(attr);

    // Also make "in italics" red
    let mut attr = pango::AttrColor::new_foreground(0xffff, 0x0000, 0x0000);
    attr.set_start_index(text.find("in itálics").unwrap() as u32);
    attr.set_end_index((text.find("in itálics").unwrap() + "in itálics".len()) as u32);
    attr_list.insert(attr);

    // Finally, set the attribute list on the layout
    layout.set_attributes(Some(&attr_list));
}
