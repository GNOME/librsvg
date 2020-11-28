SUBDIRS = . gdk-pixbuf-loader tests tools doc win32 po

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
# See LIBRSVG_INTERNALS_SRC below for the real list of Rust
# sources.
librsvg_c_api_la_SOURCES = _rsvg_dummy.c

bin_PROGRAMS = rsvg-convert

include rsvg-c-srcs.mk

librsvg_@RSVG_API_MAJOR_VERSION@_la_SOURCES =	\
	$(librsvg_c_srcs)			\
	$(NULL)

LIBRSVG_INTERNALS_SRC =						\
	Cargo.toml						\
	rsvg_internals/Cargo.toml				\
	rsvg_internals/build.rs					\
	rsvg_internals/src/allowed_url.rs			\
	rsvg_internals/src/angle.rs				\
	rsvg_internals/src/aspect_ratio.rs			\
	rsvg_internals/src/attributes.rs			\
	rsvg_internals/src/bbox.rs				\
	rsvg_internals/src/color.rs				\
	rsvg_internals/src/cond.rs				\
	rsvg_internals/src/coord_units.rs			\
	rsvg_internals/src/css.rs				\
	rsvg_internals/src/dasharray.rs				\
	rsvg_internals/src/document.rs				\
	rsvg_internals/src/dpi.rs				\
	rsvg_internals/src/drawing_ctx.rs			\
	rsvg_internals/src/element.rs				\
	rsvg_internals/src/error.rs				\
	rsvg_internals/src/filter.rs				\
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
	rsvg_internals/src/filters/lighting.rs			\
	rsvg_internals/src/filters/merge.rs			\
	rsvg_internals/src/filters/morphology.rs		\
	rsvg_internals/src/filters/mod.rs			\
	rsvg_internals/src/filters/offset.rs			\
	rsvg_internals/src/filters/tile.rs			\
	rsvg_internals/src/filters/turbulence.rs		\
	rsvg_internals/src/float_eq_cairo.rs			\
	rsvg_internals/src/font_props.rs			\
	rsvg_internals/src/gradient.rs				\
	rsvg_internals/src/handle.rs				\
	rsvg_internals/src/href.rs				\
	rsvg_internals/src/image.rs				\
	rsvg_internals/src/io.rs				\
	rsvg_internals/src/iri.rs				\
	rsvg_internals/src/length.rs				\
	rsvg_internals/src/lib.rs				\
	rsvg_internals/src/limits.rs				\
	rsvg_internals/src/log.rs				\
	rsvg_internals/src/marker.rs				\
	rsvg_internals/src/node.rs				\
	rsvg_internals/src/number_list.rs			\
	rsvg_internals/src/paint_server.rs			\
	rsvg_internals/src/parsers.rs				\
	rsvg_internals/src/path_builder.rs			\
	rsvg_internals/src/path_parser.rs			\
	rsvg_internals/src/pattern.rs				\
	rsvg_internals/src/properties.rs			\
	rsvg_internals/src/property_defs.rs			\
	rsvg_internals/src/property_macros.rs			\
	rsvg_internals/src/rect.rs				\
	rsvg_internals/src/shapes.rs				\
	rsvg_internals/src/surface_utils/iterators.rs		\
	rsvg_internals/src/surface_utils/mod.rs			\
	rsvg_internals/src/surface_utils/shared_surface.rs	\
	rsvg_internals/src/surface_utils/srgb.rs		\
	rsvg_internals/src/space.rs				\
	rsvg_internals/src/structure.rs				\
	rsvg_internals/src/style.rs				\
	rsvg_internals/src/text.rs				\
	rsvg_internals/src/transform.rs				\
	rsvg_internals/src/unit_interval.rs			\
	rsvg_internals/src/util.rs				\
	rsvg_internals/src/viewbox.rs				\
	rsvg_internals/src/xml.rs				\
	rsvg_internals/src/xml2.rs				\
	rsvg_internals/src/xml2_load.rs				\
	$(NULL)

LIBRSVG_CRATE_SRC = 						\
	librsvg_crate/Cargo.toml				\
	librsvg_crate/example.svg				\
	librsvg_crate/examples/proportional.rs			\
	librsvg_crate/src/lib.rs				\
	librsvg_crate/tests/api.rs				\
	librsvg_crate/tests/bugs.rs				\
	librsvg_crate/tests/intrinsic_dimensions.rs		\
	librsvg_crate/tests/primitives.rs			\
	librsvg_crate/tests/utils/mod.rs			\
	librsvg_crate/tests/utils/compare_surfaces.rs		\
	$(NULL)

LIBRSVG_C_API_SRC =						\
	librsvg/Cargo.toml					\
	librsvg/c_api.rs					\
	librsvg/color_utils.rs					\
	librsvg/dpi.rs						\
	librsvg/lib.rs						\
	librsvg/messages.rs					\
	librsvg/pixbuf_utils.rs					\
	librsvg/benches/pixbuf_from_surface.rs			\
	$(NULL)

RUST_EXTRA = 						\
	Cargo.lock					\
	cargo-vendor-config				\
	rsvg_internals/benches/box_blur.rs		\
	rsvg_internals/benches/composite.rs		\
	rsvg_internals/benches/lighting.rs		\
	rsvg_internals/benches/path_parser.rs           \
	rsvg_internals/benches/pixel_iterators.rs	\
	rsvg_internals/benches/srgb.rs			\
	rsvg_internals/src/ua.css			\
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

if LIB_NAME_WORKAROUND
# Workaround for https://github.com/rust-lang/rust/issues/69904
# can be removed when our minimum Rust version is at least 1.44.
RUST_LIB=$(CARGO_TARGET_DIR)/$(RUST_TARGET_SUBDIR)/rsvg_c_api.lib
else
RUST_LIB=$(CARGO_TARGET_DIR)/$(RUST_TARGET_SUBDIR)/librsvg_c_api.a
endif

check-local:
	cd $(srcdir) &&					\
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR)		\
	LIBRSVG_BUILD_DIR=$(LIBRSVG_BUILD_DIR)		\
	OUT_DIR=$(LIBRSVG_BUILD_DIR)/tests/output	\
	$(CARGO) --locked test $(CARGO_VERBOSE) $(CARGO_TARGET_ARGS) $(CARGO_RELEASE_ARGS)

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

librsvg_c_api.la: $(librsvg_c_api_la_OBJECTS) $(LIBRSVG_INTERNALS_SRC) $(LIBRSVG_C_API_SRC)
	+cd $(top_srcdir)/librsvg &&						\
	PKG_CONFIG_ALLOW_CROSS=1						\
	PKG_CONFIG='$(PKG_CONFIG)'						\
	CARGO_TARGET_DIR=$(CARGO_TARGET_DIR)					\
	$(CARGO) --locked build $(CARGO_VERBOSE) $(CARGO_TARGET_ARGS) $(CARGO_RELEASE_ARGS) \
	&& cd $(LIBRSVG_BUILD_DIR) && $(LINK) $< && mv $(RUST_LIB) .libs/librsvg_c_api.a

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

if OS_DARWIN
librsvg_@RSVG_API_MAJOR_VERSION@_la_LDFLAGS += -framework Foundation
endif

librsvg_@RSVG_API_MAJOR_VERSION@_la_LIBADD = \
	librsvg_c_api.la \
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

dist_doc_DATA =				\
	COMPILING.md			\
	CONTRIBUTING.md			\
	README.md			\
	code-of-conduct.md

EXTRA_DIST =				\
	$(LIBRSVG_INTERNALS_SRC)	\
	$(LIBRSVG_C_API_SRC)		\
	$(RUST_EXTRA)			\
	$(LIBRSVG_CRATE_SRC)		\
	librsvg.doap			\
	AUTHORS				\
	NEWS				\
	COPYING.LIB 			\
	librsvg-zip.in 			\
	gtk-doc.make			\
	Rsvg-2.0-custom.vala		\
	Rsvg-2.0.metadata		\
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

CLEANFILES = $(BUILT_SOURCES) $(nodist_gir_DATA) $(nodist_typelibs_DATA)

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
