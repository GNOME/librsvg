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

    // Note how this uses all the defaults in Pango.  The text will probably
    // be set in a sans-serif font, 10 pixels tall or so.
    let layout = pango::Layout::new(&pango_context);

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

    // Set the paint color to black; that's what pangocairo will use.
    cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
    cr.move_to(100.0, 100.0);
    pangocairo::functions::show_layout(&cr, &layout);

    // Write a PNG file from the surface
    let mut output = File::create("example_2_attributes.png").unwrap();
    surface.write_to_png(&mut output).unwrap();
}
