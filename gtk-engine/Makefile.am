INCLUDES = $(GTK_CFLAGS) $(LIBRSVG_CFLAGS)

enginedir = $(libdir)/gtk-2.0/$(GTK_VERSION)/engines

engine_LTLIBRARIES = libsvg.la

libsvg_la_SOURCES = 		\
	svg-draw.c		\
	svg-main.c		\
	svg-render.c		\
	svg-rc-style.c		\
	svg-rc-style.h		\
	svg-style.h		\
	svg.h

libsvg_la_LDFLAGS =  -avoid-version -module
libsvg_la_LIBADD = $(GTK_LIBS) $(LIBRSVG_LIBS)

dist-hook:
	cp -pr examples $(distdir);					\
        find $(distdir)/examples -name 'CVS' -print | xargs rm -rf 
