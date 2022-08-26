if ENABLE_GTK_DOC
RSVG_DOC = doc
endif

SUBDIRS = . gdk-pixbuf-loader tests $(RSVG_DOC) win32

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

LIBRSVG_SRC =					\
	Cargo.toml				\
	build.rs				\
	src/accept_language.rs			\
	src/angle.rs				\
	src/api.rs				\
	src/aspect_ratio.rs			\
	src/bbox.rs				\
	src/c_api/dpi.rs			\
	src/c_api/handle.rs			\
	src/c_api/messages.rs			\
	src/c_api/mod.rs			\
	src/c_api/pixbuf_utils.rs		\
	src/c_api/sizing.rs			\
	src/color.rs				\
	src/cond.rs				\
	src/coord_units.rs			\
	src/css.rs				\
	src/dasharray.rs			\
	src/document.rs				\
	src/dpi.rs				\
	src/drawing_ctx.rs			\
	src/element.rs				\
	src/error.rs				\
	src/filter_func.rs			\
	src/filter.rs				\
	src/filters/blend.rs			\
	src/filters/bounds.rs			\
	src/filters/color_matrix.rs		\
	src/filters/component_transfer.rs	\
	src/filters/composite.rs		\
	src/filters/context.rs			\
	src/filters/convolve_matrix.rs		\
	src/filters/displacement_map.rs		\
	src/filters/error.rs			\
	src/filters/flood.rs			\
	src/filters/gaussian_blur.rs		\
	src/filters/image.rs			\
	src/filters/lighting.rs			\
	src/filters/merge.rs			\
	src/filters/mod.rs			\
	src/filters/morphology.rs		\
	src/filters/offset.rs			\
	src/filters/tile.rs			\
	src/filters/turbulence.rs		\
	src/float_eq_cairo.rs			\
	src/font_props.rs			\
	src/gradient.rs				\
	src/handle.rs				\
	src/href.rs				\
	src/image.rs				\
	src/io.rs				\
	src/iri.rs				\
	src/layout.rs				\
	src/length.rs				\
	src/lib.rs				\
	src/limits.rs				\
	src/log.rs				\
	src/marker.rs				\
	src/node.rs				\
	src/paint_server.rs			\
	src/parsers.rs				\
	src/path_builder.rs			\
	src/path_parser.rs			\
	src/pattern.rs				\
	src/properties.rs			\
	src/property_defs.rs			\
	src/property_macros.rs			\
	src/rect.rs				\
	src/shapes.rs				\
	src/space.rs				\
	src/structure.rs			\
	src/style.rs				\
	src/surface_utils/iterators.rs		\
	src/surface_utils/mod.rs		\
	src/surface_utils/shared_surface.rs	\
	src/surface_utils/srgb.rs		\
	src/text.rs				\
	src/transform.rs			\
	src/ua.css				\
	src/unit_interval.rs			\
	src/url_resolver.rs			\
	src/util.rs				\
	src/viewbox.rs				\
	src/ua.css				\
	src/xml/attributes.rs			\
	src/xml/mod.rs				\
	src/xml/xml2_load.rs			\
	src/xml/xml2.rs				\
	$(NULL)

RUST_EXTRA =					\
	Cargo.lock				\
	cargo-vendor-config			\
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

RUST_LIB=$(CARGO_TARGET_DIR)/$(RUST_TARGET_SUBDIR)/liblibrsvg.a

check-local:
	cd $(srcdir) &&					\
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR)		\
	LIBRSVG_BUILD_DIR=$(LIBRSVG_BUILD_DIR)		\
	OUT_DIR=$(LIBRSVG_BUILD_DIR)/tests/output	\
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

dist-hook:
	(cd $(distdir) &&							\
	$(CARGO) vendor -q &&							\
	mkdir .cargo &&								\
	cp cargo-vendor-config .cargo/config)

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

$(RSVG_CONVERT_BIN): $(RSVG_CONVERT_SRC)
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
	COMPILING.md			\
	CONTRIBUTING.md			\
	README.md			\
	code-of-conduct.md

EXTRA_DIST =				\
	$(LIBRSVG_SRC)			\
	$(RUST_EXTRA)			\
	$(RSVG_CONVERT_SRC)		\
	AUTHORS				\
	COPYING.LIB 			\
	NEWS				\
	SECURITY.md			\
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
