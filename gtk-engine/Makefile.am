INCLUDES = $(GTK_CFLAGS)

enginedir = $(libdir)/gtk-2.0/$(GTK_VERSION)/engines

engine_LTLIBRARIES = libsvg.la

libsvg_la_SOURCES = 		\
	pixbuf-draw.c		\
	pixbuf-main.c		\
	pixbuf-render.c		\
	pixbuf-rc-style.c	\
	pixbuf-rc-style.h	\
	pixbuf-style.h		\
	pixbuf.h

libsvg_la_LDFLAGS =  -avoid-version -module
libsvg_la_LIBADD = $(GTK_LIBS)

dist-hook:
	cp -pr examples $(distdir);					\
        find $(distdir)/examples -name 'CVS' -print | xargs rm -rf 
