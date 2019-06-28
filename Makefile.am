SUBDIRS = . gdk-pixbuf-loader tests tools doc win32 po

NULL =
BUILT_SOURCES =

lib_LTLIBRARIES = librsvg-@RSVG_API_MAJOR_VERSION@.la

bin_PROGRAMS = rsvg-convert
if HAVE_GTK_3
bin_PROGRAMS += rsvg-view-3
endif

include rsvg-c-srcs.mk

librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES =	\
	$(librsvg_c_srcs)			\
	$(NULL)

RUST_SRC =							\
	Cargo.toml						\
	rsvg_internals/Cargo.toml				\
	rsvg_internals/build.rs					\
	rsvg_internals/src/allowed_url.rs			\
	rsvg_internals/src/angle.rs				\
	rsvg_internals/src/aspect_ratio.rs			\
	rsvg_internals/src/bbox.rs				\
	rsvg_internals/src/c_api.rs				\
	rsvg_internals/src/clip_path.rs				\
	rsvg_internals/src/color.rs				\
	rsvg_internals/src/cond.rs				\
	rsvg_internals/src/coord_units.rs			\
	rsvg_internals/src/create_node.rs			\
	rsvg_internals/src/croco.rs				\
	rsvg_internals/src/css.rs				\
	rsvg_internals/src/dpi.rs				\
	rsvg_internals/src/drawing_ctx.rs			\
	rsvg_internals/src/filters/bounds.rs			\
	rsvg_internals/src/filters/blend.rs			\
	rsvg_internals/src/filters/color_matrix.rs		\
	rsvg_internals/src/filters/component_transfer.rs	\
	rsvg_internals/src/filters/composite.rs			\
	rsvg_internals/src/filters/convolve_matrix.rs		\
	rsvg_internals/src/filters/context.rs			\
	rsvg_internals/src/filters/displacement_map.rs		\
	rsvg_internals/src/filters/error.rs			\
	rsvg_internals/src/filters/flood.rs			\
	rsvg_internals/src/filters/gaussian_blur.rs		\
	rsvg_internals/src/filters/image.rs			\
	rsvg_internals/src/filters/light/light_source.rs	\
	rsvg_internals/src/filters/light/lighting.rs		\
	rsvg_internals/src/filters/light/mod.rs			\
	rsvg_internals/src/filters/merge.rs			\
	rsvg_internals/src/filters/morphology.rs		\
	rsvg_internals/src/filters/mod.rs			\
	rsvg_internals/src/filters/node.rs			\
	rsvg_internals/src/filters/input.rs			\
	rsvg_internals/src/filters/offset.rs			\
	rsvg_internals/src/filters/tile.rs			\
	rsvg_internals/src/filters/turbulence.rs		\
	rsvg_internals/src/error.rs				\
	rsvg_internals/src/float_eq_cairo.rs			\
	rsvg_internals/src/font_props.rs			\
	rsvg_internals/src/gradient.rs				\
	rsvg_internals/src/handle.rs				\
	rsvg_internals/src/image.rs				\
	rsvg_internals/src/io.rs				\
	rsvg_internals/src/iri.rs				\
	rsvg_internals/src/length.rs				\
	rsvg_internals/src/lib.rs				\
	rsvg_internals/src/log.rs				\
	rsvg_internals/src/link.rs				\
	rsvg_internals/src/marker.rs				\
	rsvg_internals/src/mask.rs				\
	rsvg_internals/src/node.rs				\
	rsvg_internals/src/number_list.rs			\
	rsvg_internals/src/paint_server.rs			\
	rsvg_internals/src/parsers.rs				\
	rsvg_internals/src/path_builder.rs			\
	rsvg_internals/src/path_parser.rs			\
	rsvg_internals/src/pattern.rs				\
	rsvg_internals/src/pixbuf_utils.rs			\
	rsvg_internals/src/properties.rs			\
	rsvg_internals/src/property_bag.rs			\
	rsvg_internals/src/property_defs.rs			\
	rsvg_internals/src/property_macros.rs			\
	rsvg_internals/src/rect.rs				\
	rsvg_internals/src/shapes.rs				\
	rsvg_internals/src/surface_utils/iterators.rs		\
	rsvg_internals/src/surface_utils/mod.rs			\
	rsvg_internals/src/surface_utils/shared_surface.rs	\
	rsvg_internals/src/space.rs				\
	rsvg_internals/src/srgb.rs				\
	rsvg_internals/src/structure.rs				\
	rsvg_internals/src/style.rs				\
	rsvg_internals/src/svg.rs				\
	rsvg_internals/src/text.rs				\
	rsvg_internals/src/transform.rs				\
	rsvg_internals/src/tree_utils/mod.rs			\
	rsvg_internals/src/unit_interval.rs			\
	rsvg_internals/src/util.rs				\
	rsvg_internals/src/viewbox.rs				\
	rsvg_internals/src/xml.rs				\
	rsvg_internals/src/xml2.rs				\
	rsvg_internals/src/xml2_load.rs

LIBRSVG_CRATE_SOURCES = 					\
	librsvg_crate/Cargo.toml				\
	librsvg_crate/build.rs					\
	librsvg_crate/examples/proportional.rs			\
	librsvg_crate/src/lib.rs				\
	librsvg_crate/tests/api.rs				\
	librsvg_crate/tests/intrinsic_dimensions.rs		\
	librsvg_crate/tests/render_to_viewport.rs		\
	librsvg_crate/tests/primitives.rs			\
	librsvg_crate/tests/utils/mod.rs			\
	librsvg_crate/tests/utils/compare_surfaces.rs

RUST_EXTRA = 						\
	Cargo.lock					\
	cargo-vendor-config				\
	rsvg_internals/benches/box_blur.rs		\
	rsvg_internals/benches/composite.rs		\
	rsvg_internals/benches/lighting.rs		\
	rsvg_internals/benches/pixel_iterators.rs	\
	rsvg_internals/benches/srgb.rs

if DEBUG_RELEASE
CARGO_RELEASE_ARGS=
else
CARGO_RELEASE_ARGS=--release
endif

if CROSS_COMPILING
CARGO_TARGET_ARGS=--target=$(RUST_TARGET)
endif

CARGO_VERBOSE = $(cargo_verbose_$(V))
cargo_verbose_ = $(cargo_verbose_$(AM_DEFAULT_VERBOSITY))
cargo_verbose_0 =
cargo_verbose_1 = --verbose

RUST_LIB=@abs_top_builddir@/target/@RUST_TARGET_SUBDIR@/librsvg_internals.a
CARGO_TARGET_DIR=@abs_top_builddir@/target

check-local:
	cd $(srcdir) && \
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR) $(CARGO) --locked test $(CARGO_VERBOSE) $(CARGO_TARGET_ARGS) $(CARGO_RELEASE_ARGS)

clean-local:
	cd $(top_srcdir) && \
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR) $(CARGO) clean

distcheck-hook:
	if test -z "${DESTDIR}";						\
	then									\
		echo "Please specify DESTDIR=/tmp/foo so distcheck will work";	\
		exit 1;								\
	fi

dist-hook:
	(cd $(distdir) &&							\
	$(CARGO) vendor -q &&							\
	mkdir .cargo &&								\
	cp cargo-vendor-config .cargo/config)

$(RUST_LIB): $(RUST_SRC)
	+cd $(top_srcdir)/rsvg_internals &&					\
	PKG_CONFIG_ALLOW_CROSS=1						\
	PKG_CONFIG='$(PKG_CONFIG)'						\
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR)					\
	$(CARGO) --locked build $(CARGO_VERBOSE) $(CARGO_TARGET_ARGS) $(CARGO_RELEASE_ARGS) --features "c-library"

librsvg_@RSVG_API_MAJOR_VERSION@_la_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-I$(top_builddir)/librsvg 		\
	-DG_LOG_DOMAIN=\"librsvg\" 		\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	-DSRCDIR=\""${abs_top_srcdir}"\"	\
	-DRSVG_DISABLE_DEPRECATION_WARNINGS	\
	$(AM_CPPFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_CFLAGS = \
	-DRSVG_COMPILATION	\
	$(LIBRSVG_CFLAGS) 	\
	$(AM_CFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_LDFLAGS = \
	$(BSYMBOLIC_LDFLAG) \
	-version-info @RSVG_LT_VERSION_INFO@ \
	-export-dynamic \
	-no-undefined \
	-export-symbols-regex "^rsvg_.*" \
	$(AM_LDFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_LIBADD = \
	$(RUST_LIB)		\
	$(LIBRSVG_LIBS) 	\
	$(LIBM)			\
	$(DLOPEN_LIBS)

librsvgincdir = $(includedir)/librsvg-$(RSVG_API_VERSION)/librsvg
librsvginc_HEADERS =			\
	$(headers)			\
	$(extra_inc_headers)

dist_man_MANS = rsvg-convert.1

rsvg_convert_SOURCES =			\
	$(rsvg_convert_srcs)		\
	$(NULL)

rsvg_convert_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir) 			\
	-I$(top_builddir)/librsvg 		\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	$(AM_CPPFLAGS)

rsvg_convert_CFLAGS =\
	$(RSVG_CONVERT_CFLAGS) \
	$(AM_CFLAGS)

rsvg_convert_LDFLAGS = $(AM_LDFLAGS)

rsvg_convert_LDADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la	\
	$(RSVG_CONVERT_LIBS) \
	$(LIBM)

rsvg_view_3_SOURCES =			\
	$(rsvg_view_srcs)

rsvg_view_3_CPPFLAGS = \
	-I$(top_srcdir) 			\
	-I$(top_builddir)/librsvg 		\
	-DLIBRSVG_DATADIR="\"$(datadir)\""	\
	$(AM_CPPFLAGS)

rsvg_view_3_CFLAGS =\
	$(GTK3_CFLAGS)		\
	$(LIBRSVG_CFLAGS) 	\
	$(AM_CFLAGS)

rsvg_view_3_LDFLAGS =

rsvg_view_3_LDADD = \
	$(top_builddir)/librsvg-@RSVG_API_MAJOR_VERSION@.la \
	$(GTK3_LIBS)		\
	$(LIBRSVG_LIBS)		\
	$(GTHREAD_LIBS)		\
	$(LIBM)

if OS_WIN32
rsvg_view_3_LDFLAGS += -mwindows
endif # OS_WIN32

dist_doc_DATA =				\
	COMPILING.md			\
	CONTRIBUTING.md			\
	README.md			\
	code-of-conduct.md

EXTRA_DIST =				\
	$(RUST_SRC)			\
	$(RUST_EXTRA)			\
	$(LIBRSVG_CRATE_SOURCES)	\
	librsvg.doap			\
	AUTHORS				\
	NEWS				\
	COPYING.LIB 			\
	librsvg-zip.in 			\
	gtk-doc.make			\
	Rsvg-2.0-custom.vala		\
	Rsvg-2.0.metadata		\
	config.h.win32.in		\
	config.h.win32			\
	rsvg.symbols			\
	glib-tap.mk			\
	tap-driver.sh			\
	tap-test

## Put `exec' in the name because this should be installed by
## `install-exec', not `install-data'.

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = librsvg-$(RSVG_API_VERSION).pc

DISTCHECK_CONFIGURE_FLAGS = --enable-gtk-doc

if HAVE_INTROSPECTION
-include $(INTROSPECTION_MAKEFILE)

INTROSPECTION_GIRS = Rsvg-@RSVG_API_VERSION@.gir
INTROSPECTION_SCANNER_ARGS = --add-include-path=$(srcdir) --symbol-prefix=rsvg --symbol-prefix=librsvg
INTROSPECTION_COMPILER_ARGS = --includedir=$(srcdir)

Rsvg-@RSVG_API_VERSION@.gir: librsvg-@RSVG_API_MAJOR_VERSION@.la

Rsvg_@RSVG_API_VERSION_U@_gir_NAMESPACE = Rsvg
Rsvg_@RSVG_API_VERSION_U@_gir_EXPORT_PACKAGES = librsvg-$(RSVG_API_VERSION)
Rsvg_@RSVG_API_VERSION_U@_gir_FILES = \
	$(librsvginc_HEADERS) \
	$(filter-out %.h,$(librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES)) \
	$(NULL)
Rsvg_@RSVG_API_VERSION_U@_gir_INCLUDES = \
	GLib-2.0 \
	GObject-2.0 \
	Gio-2.0 \
	cairo-1.0 \
	GdkPixbuf-2.0 \
	$(NULL)
Rsvg_@RSVG_API_VERSION_U@_gir_PACKAGES = \
	pango \
	libxml-2.0 \
	$(NULL)
Rsvg_@RSVG_API_VERSION_U@_gir_CFLAGS = -I$(top_builddir) -I$(top_builddir)/librsvg -DRSVG_COMPILATION
Rsvg_@RSVG_API_VERSION_U@_gir_LIBS = librsvg-@RSVG_API_MAJOR_VERSION@.la
Rsvg_@RSVG_API_VERSION_U@_gir_SCANNERFLAGS = --c-include="librsvg/rsvg.h"

girdir = $(datadir)/gir-1.0
nodist_gir_DATA = $(INTROSPECTION_GIRS)

typelibsdir = $(libdir)/girepository-1.0
nodist_typelibs_DATA = $(INTROSPECTION_GIRS:.gir=.typelib)

CLEANFILES = $(nodist_gir_DATA) $(nodist_typelibs_DATA)

if ENABLE_VAPIGEN
include $(VAPIGEN_MAKEFILE)

librsvg-$(RSVG_API_VERSION).vapi: Rsvg-$(RSVG_API_VERSION).gir

VAPIGEN_VAPIS = librsvg-$(RSVG_API_VERSION).vapi

librsvg_@RSVG_API_VERSION_U@_vapi_DEPS = gio-2.0 cairo
librsvg_@RSVG_API_VERSION_U@_vapi_METADATADIRS = $(srcdir)
librsvg_@RSVG_API_VERSION_U@_vapi_FILES = Rsvg-$(RSVG_API_VERSION).gir $(srcdir)/Rsvg-$(RSVG_API_VERSION)-custom.vala

vapidir = $(datadir)/vala/vapi
vapi_DATA = $(VAPIGEN_VAPIS)

CLEANFILES += $(VAPIGEN_VAPIS)

endif # ENABLE_VAPIGEN

endif # HAVE_INTROSPECTION
