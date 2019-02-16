use std::env;
extern crate cc;

fn main() {
    let mut cc = cc::Build::new();
    cc.include("..");

    // Expose internals
    cc.define("RSVG_COMPILATION", Some("1"));
    cc.define("RSVG_DISABLE_DEPRECATION_WARNINGS", Some("1"));

    cc.define("DG_LOG_DOMAIN", Some("\"librsvg\""));
    cc.define("GETTEXT_PACKAGE", Some("\"librsvg\""));
    cc.define("LIBRSVG_MAJOR_VERSION", Some(env::var("CARGO_PKG_VERSION_MAJOR").unwrap().as_str()));
    cc.define("LIBRSVG_MINOR_VERSION", Some(env::var("CARGO_PKG_VERSION_MINOR").unwrap().as_str()));
    cc.define("LIBRSVG_MICRO_VERSION", Some(env::var("CARGO_PKG_VERSION_PATCH").unwrap().as_str()));
    cc.define("LIBRSVG_VERSION", Some(format!("\"{}\"", env::var("CARGO_PKG_VERSION").unwrap()).as_str()));
    cc.define("VERSION", Some(format!("\"{}\"", env::var("CARGO_PKG_VERSION").unwrap()).as_str()));

    for var_name in ["DEP_GLIB_INCLUDE", "DEP_CAIRO_INCLUDE", "DEP_PANGO_INCLUDE", "DEP_GDK_PIXBUF_INCLUDE"].iter() {
        let var = env::var_os(var_name).expect(var_name);
        for inc in env::split_paths(&var) {
            cc.include(inc);
        }
    }
    cc.file("librsvg-features.c");
    cc.file("rsvg-base.c");
    cc.file("rsvg-handle.c");
    cc.file("rsvg-pixbuf.c");
    cc.compile("rsvg_legacy");
}
