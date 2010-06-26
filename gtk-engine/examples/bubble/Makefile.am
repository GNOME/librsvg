gtk2_themedir = $(datadir)/themes/bubble/gtk-2.0
gtk3_themedir = $(datadir)/themes/bubble/gtk-3.0

theme_files = \
	gtkrc		\
	blue.svg	\
	green.svg	\
	orange.svg	\
	purple.svg	\
	red.svg

if HAVE_GTK_2
gtk2_theme_DATA = $(theme_files)
endif
if HAVE_GTK_3
gtk3_theme_DATA = $(theme_files)
endif

EXTRA_DIST = \
	$(theme_files)	\
	README
