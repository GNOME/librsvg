headers =			\
	librsvg/rsvg.h		\
	librsvg/rsvg-cairo.h

extra_inc_headers =	\
	librsvg/librsvg-features.h

librsvg_c_srcs =	\
	librsvg/librsvg-features.c 		\
	librsvg/librsvg-features.h 		\
	librsvg/rsvg-base.c			\
	librsvg/rsvg-cairo.h			\
	librsvg/rsvg-css.h 			\
	librsvg/rsvg-handle.c			\
	librsvg/rsvg-pixbuf.c			\
	librsvg/rsvg.h				\
	$(NULL)

rsvg_convert_srcs = rsvg-convert.c
