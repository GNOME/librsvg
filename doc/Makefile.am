## Process this file with automake to produce Makefile.in

# The name of the module.
DOC_MODULE=rsvg

# The top-level SGML file.
DOC_MAIN_SGML_FILE=rsvg-docs.sgml

# The directory containing the source code. Relative to $(srcdir)
DOC_SOURCE_DIR=..

# Extra options to pass to gtkdoc-scangobj
SCANGOBJ_OPTIONS= --nogtkinit --type-init-func="g_type_init ()" 

# Extra options to supply to gtkdoc-scan
SCAN_OPTIONS=--deprecated-guards=RSVG_DISABLE_DEPRECATED

# Extra options to supply to gtkdoc-mkdb
MKDB_OPTIONS= --sgml-mode --output-format=xml

# Extra options to supply to gtkdoc-fixref
FIXXREF_OPTIONS=

# Used for dependencies
HFILE_GLOB=$(top_srcdir)/*.h
CFILE_GLOB=$(top_srcdir)/*.c

# Header files to ignore when scanning
IGNORE_HFILES= \
	config.h		\
	librsvg-features.h	\
	rsvg-bpath-util.h	\
	rsvg-css.h		\
	rsvg-defs.h		\
	rsvg-paint-server.h	\
	rsvg-path.h		\
	rsvg-private.h		\
	rsvg-shapes.h		\
	rsvg-styles.h		\
	rsvg-text.h		\
	svg-rc-style.h		\
	svg-style.h		\
	svg.h

# Images to copy into HTML directory
HTML_IMAGES=

EXTRA_HFILES=

# Extra SGML files that are included by $(DOC_MAIN_SGML_FILE)
content_files=

# Other files to distribute
extra_files=

# CFLAGS and LDFLAGS for compiling scan program. Only needed
# if $(DOC_MODULE).types is non-empty.
GTKDOC_CFLAGS = -I$(top_srcdir) $(LIBRSVG_CFLAGS)

GTKDOC_LIBS = $(top_builddir)/librsvg-2.la $(LIBRSVG_LIBS)

if GTK_DOC_INSTALLED
include $(top_srcdir)/gtk-doc.make
CLEANFILES += rsvg-scan.*
.PHONY : dist-hook-local
endif
