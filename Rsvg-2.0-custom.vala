namespace Rsvg {
	namespace Version {
		[CCode (cname = "LIBRSVG_CHECK_VERSION")]
		public static bool check (int major, int minor, int micro);
	}
}
