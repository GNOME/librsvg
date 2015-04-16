/* rsvg-compat.h: miscellaneous compatibility functions to deal with deprecations in the platform */

#ifndef RSVG_COMPAT_H
#define RSVG_COMPAT_H

/* g_type_init() was deprecated in glib 2.36.0 */
#if !GLIB_CHECK_VERSION (2, 36, 0)
#  define RSVG_G_TYPE_INIT g_type_init ()
#else
#  define RSVG_G_TYPE_INIT {}
#endif

#endif /* RSVG_COMPAT_H */
