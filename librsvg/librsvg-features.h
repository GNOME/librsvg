#if !defined (__RSVG_RSVG_H_INSIDE__) && !defined (RSVG_COMPILATION)
#warning "Including <librsvg/librsvg-features.h> directly is deprecated."
#endif

#ifndef LIBRSVG_FEATURES_H
#define LIBRSVG_FEATURES_H

#define LIBRSVG_CHECK_VERSION(major,minor,micro) \
  (LIBRSVG_MAJOR_VERSION > (major) || \
   (LIBRSVG_MAJOR_VERSION == (major) && LIBRSVG_MINOR_VERSION > (minor)) || \
   (LIBRSVG_MAJOR_VERSION == (major) && LIBRSVG_MINOR_VERSION == (minor) && LIBRSVG_MICRO_VERSION >= (micro)))

#ifndef __GI_SCANNER__
#define LIBRSVG_HAVE_SVGZ  (TRUE)
#define LIBRSVG_HAVE_CSS   (TRUE)

#define LIBRSVG_CHECK_FEATURE(FEATURE) (defined(LIBRSVG_HAVE_##FEATURE) && LIBRSVG_HAVE_##FEATURE)
#endif

#ifndef __GTK_DOC_IGNORE__

/*
 * On Windows builds, we need to decorate variables that are exposed in the public API
 * so that they can be properly exported and linked to, for DLL builds
 */
#ifndef RSVG_VAR
# ifdef G_PLATFORM_WIN32
#  ifndef RSVG_STATIC
#   ifdef RSVG_COMPILATION
#    define RSVG_VAR extern __declspec (dllexport)
#   else /* RSVG_COMPILATION */
#    define RSVG_VAR extern __declspec (dllimport)
#   endif /* !RSVG_COMPILATION */
#  else /* !RSVG_STATIC */
#   define RSVG_VAR extern
#  endif /* RSVG_STATIC */
# else /* G_PLATFORM_WIN32 */
#  define RSVG_VAR extern
# endif /* !G_PLATFORM_WIN32 */
#endif

#endif /* __GTK_DOC_IGNORE__ */


RSVG_VAR const guint librsvg_major_version;
RSVG_VAR const guint librsvg_minor_version;
RSVG_VAR const guint librsvg_micro_version;
RSVG_VAR const char librsvg_version[];

#endif
