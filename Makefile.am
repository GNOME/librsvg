if ENABLE_GTK_DOC
RSVG_DOC = doc
endif

SUBDIRS = . gdk-pixbuf-loader librsvg-c/tests-c $(RSVG_DOC) win32

NULL =

lib_LTLIBRARIES = librsvg-@RSVG_API_MAJOR_VERSION@.la
noinst_LTLIBRARIES = librsvg_c_api.la

# Some systems, reportedly OpenBSD and Mac OS X, refuse
# to create libraries without any object files. Compiling
# this file, and adding its object file to the library,
# will prevent the library from being empty.
BUILT_SOURCES = _rsvg_dummy.c
_rsvg_dummy.c:
	echo "static int __attribute__((unused)) __rsvg_dummy;" > _rsvg_dummy.c

# Automake wants to have a C sources list for LTLIBRARIES.
# See LIBRSVG_SRC below for the real list of Rust sources.
librsvg_c_api_la_SOURCES = _rsvg_dummy.c

CLEANFILES = $(BUILT_SOURCES)

include rsvg-c-srcs.mk

librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES =	\
	$(librsvg_public_headers)		\
	$(NULL)

LIBRSVG_SRC =						\
	Cargo.toml					\
	librsvg-c/Cargo.toml				\
	librsvg-c/build.rs				\
	librsvg-c/src/dpi.rs				\
	librsvg-c/src/handle.rs				\
	librsvg-c/src/lib.rs				\
	librsvg-c/src/messages.rs			\
	librsvg-c/src/pixbuf_utils.rs			\
	librsvg-c/src/sizing.rs				\
	rsvg/Cargo.toml					\
	rsvg/build.rs					\
	rsvg/src/accept_language.rs			\
	rsvg/src/angle.rs				\
	rsvg/src/api.rs					\
	rsvg/src/aspect_ratio.rs			\
	rsvg/src/bbox.rs				\
	rsvg/src/color.rs				\
	rsvg/src/cond.rs				\
	rsvg/src/coord_units.rs				\
	rsvg/src/css.rs					\
	rsvg/src/dasharray.rs				\
	rsvg/src/document.rs				\
	rsvg/src/dpi.rs					\
	rsvg/src/drawing_ctx.rs				\
	rsvg/src/element.rs				\
	rsvg/src/error.rs				\
	rsvg/src/filter_func.rs				\
	rsvg/src/filter.rs				\
	rsvg/src/filters/blend.rs			\
	rsvg/src/filters/bounds.rs			\
	rsvg/src/filters/color_matrix.rs		\
	rsvg/src/filters/component_transfer.rs		\
	rsvg/src/filters/composite.rs			\
	rsvg/src/filters/context.rs			\
	rsvg/src/filters/convolve_matrix.rs		\
	rsvg/src/filters/displacement_map.rs		\
	rsvg/src/filters/drop_shadow.rs			\
	rsvg/src/filters/error.rs			\
	rsvg/src/filters/flood.rs			\
	rsvg/src/filters/gaussian_blur.rs		\
	rsvg/src/filters/image.rs			\
	rsvg/src/filters/lighting.rs			\
	rsvg/src/filters/merge.rs			\
	rsvg/src/filters/mod.rs				\
	rsvg/src/filters/morphology.rs			\
	rsvg/src/filters/offset.rs			\
	rsvg/src/filters/tile.rs			\
	rsvg/src/filters/turbulence.rs			\
	rsvg/src/float_eq_cairo.rs			\
	rsvg/src/font_props.rs				\
	rsvg/src/gradient.rs				\
	rsvg/src/handle.rs				\
	rsvg/src/href.rs				\
	rsvg/src/image.rs				\
	rsvg/src/io.rs					\
	rsvg/src/iri.rs					\
	rsvg/src/layout.rs				\
	rsvg/src/length.rs				\
	rsvg/src/lib.rs					\
	rsvg/src/limits.rs				\
	rsvg/src/log.rs					\
	rsvg/src/marker.rs				\
	rsvg/src/node.rs				\
	rsvg/src/paint_server.rs			\
	rsvg/src/parsers.rs				\
	rsvg/src/path_builder.rs			\
	rsvg/src/path_parser.rs				\
	rsvg/src/pattern.rs				\
	rsvg/src/properties.rs				\
	rsvg/src/property_defs.rs			\
	rsvg/src/property_macros.rs			\
	rsvg/src/rect.rs				\
	rsvg/src/session.rs				\
	rsvg/src/shapes.rs				\
	rsvg/src/space.rs				\
	rsvg/src/structure.rs				\
	rsvg/src/style.rs				\
	rsvg/src/surface_utils/iterators.rs		\
	rsvg/src/surface_utils/mod.rs			\
	rsvg/src/surface_utils/shared_surface.rs	\
	rsvg/src/surface_utils/srgb.rs			\
	rsvg/src/text.rs				\
	rsvg/src/transform.rs				\
	rsvg/src/ua.css					\
	rsvg/src/unit_interval.rs			\
	rsvg/src/url_resolver.rs			\
	rsvg/src/util.rs				\
	rsvg/src/viewbox.rs				\
	rsvg/src/ua.css					\
	rsvg/src/xml/attributes.rs			\
	rsvg/src/xml/mod.rs				\
	rsvg/src/xml/xml2_load.rs			\
	rsvg/src/xml/xml2.rs				\
	$(NULL)

RUST_EXTRA =					\
	Cargo.lock				\
	example.svg				\
	benches/box_blur.rs			\
	benches/composite.rs			\
	benches/lighting.rs			\
	benches/path_parser.rs			\
	benches/pixbuf_from_surface.rs		\
	benches/pixel_iterators.rs		\
	benches/pixel_ops.rs			\
	benches/srgb.rs				\
	benches/surface_from_pixbuf.rs		\
	$(NULL)

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

LIBRSVG_BUILD_DIR=@abs_top_builddir@
CARGO_TARGET_DIR=$(LIBRSVG_BUILD_DIR)/target

RUST_LIB=$(CARGO_TARGET_DIR)/$(RUST_TARGET_SUBDIR)/librsvg.a

check-local:
	cd $(srcdir) &&						\
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR)			\
	LIBRSVG_BUILD_DIR=$(LIBRSVG_BUILD_DIR)			\
	TESTS_OUTPUT_DIR=$(LIBRSVG_BUILD_DIR)/tests/output	\
	$(CARGO) --locked test $(CARGO_VERBOSE) $(CARGO_TARGET_ARGS) $(CARGO_RELEASE_ARGS) -- --include-ignored -Z unstable-options

clean-local:
	cd $(top_srcdir) &&			\
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR)	\
	$(CARGO) clean

distcheck-hook:
	if test -z "${DESTDIR}";						\
	then									\
		echo "Please specify DESTDIR=/tmp/foo so distcheck will work";	\
		exit 1;								\
	fi

librsvg_c_api.la: $(librsvg_c_api_la_OBJECTS) $(LIBRSVG_SRC)
	+cd $(top_srcdir) &&							\
	PKG_CONFIG_ALLOW_CROSS=1						\
	PKG_CONFIG='$(PKG_CONFIG)'						\
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR)					\
	$(CARGO) --locked build $(CARGO_VERBOSE) $(CARGO_TARGET_ARGS) $(CARGO_RELEASE_ARGS) --lib \
	&& cd $(LIBRSVG_BUILD_DIR) && $(LINK) $< && mv $(RUST_LIB) .libs/librsvg_c_api.a

librsvg_@RSVG_API_MAJOR_VERSION@_la_CPPFLAGS = $(AM_CPPFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_CFLAGS = $(AM_CFLAGS)

librsvg_@RSVG_API_MAJOR_VERSION@_la_LDFLAGS = \
	$(BSYMBOLIC_LDFLAG) \
	-version-info @RSVG_LT_VERSION_INFO@ \
	-export-dynamic \
	-no-undefined \
	-export-symbols-regex "^rsvg_.*" \
	$(AM_LDFLAGS)

if OS_DARWIN
librsvg_@RSVG_API_MAJOR_VERSION@_la_LDFLAGS += -framework Foundation
endif

librsvg_@RSVG_API_MAJOR_VERSION@_la_LIBADD = \
	librsvg_c_api.la	\
	$(LIBRSVG_LIBS) 	\
	$(LIBM)			\
	$(DLOPEN_LIBS)

librsvgincdir = $(includedir)/librsvg-$(RSVG_API_VERSION)/librsvg
librsvginc_HEADERS = $(librsvg_public_headers)

# Use SCRIPTS instead of PROGRAMS since we build it on our own
bin_SCRIPTS = rsvg-convert$(EXEEXT)

CLEANFILES += $(bin_SCRIPTS)

RSVG_CONVERT_BIN=$(CARGO_TARGET_DIR)/$(RUST_TARGET_SUBDIR)/rsvg-convert$(EXEEXT)

RSVG_CONVERT_SRC = src/bin/rsvg-convert.rs

$(RSVG_CONVERT_BIN): $(RSVG_CONVERT_SRC) | librsvg_c_api.la
	+cd $(top_srcdir) &&							\
	PKG_CONFIG_ALLOW_CROSS=1						\
	PKG_CONFIG='$(PKG_CONFIG)'						\
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR)					\
	$(CARGO) --locked build $(CARGO_VERBOSE) $(CARGO_TARGET_ARGS) $(CARGO_RELEASE_ARGS) --bin rsvg-convert

rsvg-convert$(EXEEXT): $(RSVG_CONVERT_BIN)
	cd $(LIBRSVG_BUILD_DIR) && mv $(RSVG_CONVERT_BIN) rsvg-convert$(EXEEXT)

rsvg-convert.1: rsvg-convert.rst
if HAVE_RST2MAN
	$(RST2MAN) $(top_srcdir)/rsvg-convert.rst rsvg-convert.1
else
	@echo "========================================"
	@echo "You need rst2man installed to make dist"
	@echo "========================================"
	@false
endif

if HAVE_RST2MAN
man1_MANS = rsvg-convert.1

CLEANFILES += rsvg-convert.1
endif

dist_doc_DATA =				\
	README.md			\
	code-of-conduct.md

EXTRA_DIST =				\
	$(LIBRSVG_SRC)			\
	$(RUST_EXTRA)			\
	$(RSVG_CONVERT_SRC)		\
	AUTHORS				\
	COPYING.LIB 			\
	NEWS				\
	Rsvg-2.0-custom.vala		\
	Rsvg-2.0.metadata		\
	glib-tap.mk			\
	librsvg-zip.in 			\
	librsvg.doap			\
	rsvg-convert.rst		\
	tap-driver.sh			\
	tap-test			\
	$(NULL)

pkgconfigdir = $(libdir)/pkgconfig
pkgconfig_DATA = librsvg-$(RSVG_API_VERSION).pc

if HAVE_INTROSPECTION
-include $(INTROSPECTION_MAKEFILE)

INTROSPECTION_GIRS = Rsvg-@RSVG_API_VERSION@.gir
INTROSPECTION_SCANNER_ARGS = --add-include-path=$(srcdir) --symbol-prefix=rsvg --symbol-prefix=librsvg --warn-all
INTROSPECTION_COMPILER_ARGS = --includedir=$(srcdir)

Rsvg-@RSVG_API_VERSION@.gir: librsvg-@RSVG_API_MAJOR_VERSION@.la

Rsvg_@RSVG_API_VERSION_U@_gir_NAMESPACE = Rsvg
Rsvg_@RSVG_API_VERSION_U@_gir_EXPORT_PACKAGES = librsvg-$(RSVG_API_VERSION)
Rsvg_@RSVG_API_VERSION_U@_gir_FILES = $(librsvginc_HEADERS)
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
Rsvg_@RSVG_API_VERSION_U@_gir_CFLAGS = -I$(top_srcdir)/include -I$(top_builddir) -I$(top_builddir)/include -DRSVG_COMPILATION
Rsvg_@RSVG_API_VERSION_U@_gir_LIBS = librsvg-@RSVG_API_MAJOR_VERSION@.la
Rsvg_@RSVG_API_VERSION_U@_gir_SCANNERFLAGS = --c-include="librsvg/rsvg.h"

girdir = $(datadir)/gir-1.0
nodist_gir_DATA = $(INTROSPECTION_GIRS)

typelibsdir = $(libdir)/girepository-1.0
nodist_typelibs_DATA = $(INTROSPECTION_GIRS:.gir=.typelib)

CLEANFILES += $(nodist_gir_DATA) $(nodist_typelibs_DATA)

if ENABLE_VAPIGEN
include $(VAPIGEN_MAKEFILE)

librsvg-$(RSVG_API_VERSION).vapi: Rsvg-$(RSVG_API_VERSION).gir

VAPIGEN_VAPIS = librsvg-$(RSVG_API_VERSION).vapi

librsvg_@RSVG_API_VERSION_U@_vapi_DEPS = gio-2.0 cairo gdk-pixbuf-2.0
librsvg_@RSVG_API_VERSION_U@_vapi_METADATADIRS = $(srcdir)
librsvg_@RSVG_API_VERSION_U@_vapi_FILES = Rsvg-$(RSVG_API_VERSION).gir $(srcdir)/Rsvg-$(RSVG_API_VERSION)-custom.vala

vapidir = $(datadir)/vala/vapi
vapi_DATA = $(VAPIGEN_VAPIS)

CLEANFILES += $(VAPIGEN_VAPIS)

endif # ENABLE_VAPIGEN

endif # HAVE_INTROSPECTION
