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
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 400, 400).unwrap();
    let cr = cairo::Context::new(&surface).unwrap();
    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    cr.paint().unwrap();

    // Note how this uses all the defaults in Pango.  The text will probably
    // be set in a sans-serif font, 10 pixels tall or so.
    let layout = pango::Layout::new(&pango_context);
    layout.set_text("Hello world!");

    // Set the paint color to black; that's what pangocairo will use.
    cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
    cr.move_to(100.0, 100.0);
    pangocairo::functions::show_layout(&cr, &layout);

    // Write a PNG file from the surface
    let mut output = File::create("example_1_trivial.png").unwrap();
    surface.write_to_png(&mut output).unwrap();
}
