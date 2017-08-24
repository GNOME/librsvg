extern crate cairo;
extern crate gtk;
extern crate rsvg;

use std::env;

use gtk::prelude::*;
use gtk::DrawingArea;

use cairo::Context;
use rsvg::Handle;
use rsvg::HandleExt;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Need single input svg.");
        return;
    }

    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    let handle = Handle::new_from_file(&args[1]).unwrap();
    let svg_dimensions = handle.get_dimensions();

    drawable(500, 500, move |drawing_area, cr| {
        let (da_width, da_height) = (drawing_area.get_allocated_width(), drawing_area.get_allocated_height());
        let (svg_width, svg_height) = (svg_dimensions.width, svg_dimensions.height);
        let (scale_x, scale_y) = (da_width as f64 / svg_width as f64, da_height as f64 / svg_height as f64);
        let scale = if scale_x < scale_y { scale_x } else { scale_y };

        println!("window_size: {}, {}; svg_size: {}, {}; scale: {}", da_width, da_height, svg_width, svg_height, scale);
        cr.scale(scale, scale);

        cr.paint_with_alpha(0.0);
        handle.render_cairo(&cr);

        Inhibit(false)
    });

    gtk::main();
}

pub fn drawable<F>(width: i32, height: i32, draw_fn: F)
    where F: Fn(&DrawingArea, &Context) -> Inhibit + 'static {
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    let drawing_area = Box::new(DrawingArea::new)();

    drawing_area.connect_draw(draw_fn);

    window.set_default_size(width, height);

    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });
    window.add(&drawing_area);
    window.show_all();
}