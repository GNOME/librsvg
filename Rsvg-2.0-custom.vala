namespace Rsvg {
	public class Handle : GLib.Object {
		[Version (deprecated = true, deprecated_since = "2.36", replacement = "")]
		public unowned string get_desc ();
		[Version (deprecated = true, deprecated_since = "2.13.90", replacement = "GLib.Object.unref")]
		public void free ();
		[Version (deprecated = true, deprecated_since = "2.13.90", replacement = "render_cairo")]
		public void set_size_callback (owned Rsvg.SizeFunc size_func);
		[Version (deprecated = true, deprecated_since = "2.36", replacement = "")]
		public unowned string get_title ();
		[Version (deprecated = true, deprecated_since = "2.36", replacement = "")]
		public unowned string get_metadata ();
	}

	namespace Version {
		[CCode (cname = "LIBRSVG_CHECK_VERSION")]
		public static bool check (int major, int minor, int micro);
	}

	[Version (deprecated = true, deprecated_since = "2.13.90", replacement = "render_cairo")]
	public delegate void SizeFunc (ref int width, ref int height);

	[Version (deprecated = true, deprecated_since = "2.36", replacement = "")]
	public static void init ();
	[Version (deprecated = true, deprecated_since = "2.35.0", replacement = "")]
	public static Gdk.Pixbuf pixbuf_from_file (string file_name) throws GLib.Error;
	[Version (deprecated = true, deprecated_since = "2.35.0", replacement = "")]
	public static Gdk.Pixbuf pixbuf_from_file_at_max_size (string file_name, int max_width, int max_height) throws GLib.Error;
	[Version (deprecated = true, deprecated_since = "2.35.0", replacement = "")]
	public static Gdk.Pixbuf pixbuf_from_file_at_size (string file_name, int width, int height) throws GLib.Error;
	[Version (deprecated = true, deprecated_since = "2.35.0", replacement = "")]
	public static Gdk.Pixbuf pixbuf_from_file_at_zoom (string file_name, double x_zoom, double y_zoom) throws GLib.Error;
	[Version (deprecated = true, deprecated_since = "2.35.0", replacement = "")]
	public static Gdk.Pixbuf pixbuf_from_file_at_zoom_with_max (string file_name, double x_zoom, double y_zoom, int max_width, int max_height) throws GLib.Error;
	[Version (deprecated = true, deprecated_since = "2.35.0", replacement = "")]
	public static void term ();
}
