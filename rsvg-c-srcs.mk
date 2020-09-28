headers =			\
	librsvg/rsvg.h		\
	librsvg/rsvg-cairo.h	\
	librsvg/rsvg-features.h

extra_inc_headers =		\
	librsvg/rsvg-version.h

librsvg_c_srcs =		\
	librsvg/rsvg-cairo.h	\
	librsvg/rsvg-css.h 	\
	librsvg/rsvg-features.h \
	librsvg/rsvg-version.h	\
	librsvg/rsvg.h		\
	$(NULL)

rsvg_convert_srcs = rsvg-convert.c
