project('librsvg',
        'c',
        version: '2.57.91',
        meson_version: '>= 1.2.0',
        default_options: [
          'pkgconfig.relocatable=true',
        ]
)

librsvg_api_version = '2.0'
api_split = librsvg_api_version.split('.')
librsvg_api_major = api_split[0]
librsvg_api_minor = api_split[1]

librsvg_pc = 'librsvg-@0@'.format(librsvg_api_version)
rsvg_ver = 'rsvg-@0@'.format(librsvg_api_major)

system = build_machine.system()
cc = meson.get_compiler('c')

# MSRV - Minimum Supported Rust Version
# If you change this, please update compiling.rst
cargo = find_program('cargo', version:'>= 1.70.0', native: true)
cargo_wrapper = find_program('meson/cargo_wrapper.py', native: true)
cargo_c = find_program('cargo-cbuild', version:'>= 0.9.19', native: true)
rustc = find_program('rustc', version:'>= 1.70.0', native: true)
makedef = find_program('meson/makedef.py', native: true)

py = import('python')

python = py.find_installation()

# Required versions for dependencies - Please update compiling.rst with version numbers if these change
cairo_required         = '>= 1.17.0'
ft2_cmake_required     = '>= 2.8.0'   # Actual FreeType version required
freetype2_required     = '>= 20.0.14' # Corresponds to ft2_cmake_required
gdk_pixbuf_required    = '>= 2.20'
gio_required           = '>= 2.24.0'
glib_required          = '>= 2.50.0'
harfbuzz_required      = '>= 2.0.0'
libxml_required        = '>= 2.9.0'
pango_required         = '>= 1.50.0'

gidocgen_required      = '>= 2021.1'
introspection_required = '>= 1.39.0'
vapigen_required       = '>= 0.17.1.26'
# FIXME: add subprojects for the deps above

cairo_dep      = dependency('cairo',
                            version: cairo_required,
                            fallback: ['cairo', 'libcairo_dep'])
cairogobj_dep  = dependency('cairo-gobject',
                            version: cairo_required,
                            fallback: ['cairo', 'libcairogobject_dep'])
cairo_png_dep  = dependency('cairo-png',
                            version: cairo_required,
                            fallback: ['cairo']) # FIXME: cairo has no _dep for this one
freetype2_dep  = dependency('freetype2',
                            version: freetype2_required,
                            method: 'pkg-config',
                            required: false)

# Try again using CMake, as Windows builds of FreeType might not
# come with pkg-config files; sadly, pkg-config and CMake use
# different version schemes for FreeType
if not freetype2_dep.found()
  freetype2_dep  = dependency('FreeType',
                              version: ft2_cmake_required,
                              method: 'cmake',
                              fallback: ['freetype2', 'freetype_dep'])
endif
pixbuf_dep     = dependency('gdk-pixbuf-2.0',
                            version: gdk_pixbuf_required,
                            fallback: ['gdk-pixbuf', 'gdkpixbuf_dep'],
                            default_options: ['png=enabled', 'jpeg=enabled', 'builtin_loaders=png,jpeg', 'man=false'])
gio_dep        = dependency('gio-2.0',
                            version: gio_required,
                            fallback: ['glib', 'libgio_dep'])
glib_dep       = dependency('glib-2.0',
                            version: glib_required,
                            fallback: ['glib', 'libglib_dep'])
harfbuzz_dep   = dependency('harfbuzz',
                            version: harfbuzz_required,
                            fallback: ['harfbuzz', 'libharfbuzz_dep'])
libxml_dep     = dependency(['libxml-2.0', 'LibXml2'],
                            version: libxml_required)
pango_dep      = dependency('pango',
                            version: pango_required,
                            fallback: ['pango', 'libpango_dep'])
pangocairo_dep = dependency('pangocairo',
                            version: pango_required,
                            fallback: ['pango', 'libpangocairo_dep'])
pangoft2_dep   = dependency('pangoft2',
                            version: pango_required,
                            required: system not in ['windows', 'darwin', 'ios'],
                            fallback: ['pango', 'libpangoft2_dep'])
gmodule_dep    = dependency('gmodule-2.0', version: glib_required,
                            fallback : ['glib', 'libgmodule_dep'])
rst2man        = find_program('rst2man', 'rst2man.py',
                              required: get_option('docs'),
                              disabler: true,
                              native: true)
gidocgen       = find_program('gi-docgen', version: gidocgen_required,
                            required: false,
                            disabler: true,
                            native: true)
gi_dep         = dependency('gobject-introspection-1.0',
                            version: introspection_required,
                            required: get_option('introspection'))
vapigen_dep    = dependency('vapigen',
                            version: vapigen_required,
                            required: get_option('vala'))

# Extra env to pass to cargo
extra_env = {}

# If FreeType and/or libxml2 is/are found by CMake instead of
# pkg-config, we must tell Cargo the libraries explicitly and
# disable pkg-config for these deps
if freetype2_dep.type_name() == 'cmake' or libxml_dep.type_name() == 'cmake'
  fs = import('fs')
endif
if freetype2_dep.type_name() == 'cmake'
  ft2_lib = freetype2_dep.get_variable('FREETYPE_LIBRARIES')
  extra_env += {'SYSTEM_DEPS_FREETYPE2_NO_PKG_CONFIG': '1',
                'SYSTEM_DEPS_FREETYPE2_SEARCH_NATIVE': fs.parent(ft2_lib),
                'SYSTEM_DEPS_FREETYPE2_LIB': fs.stem(ft2_lib)}
endif
if libxml_dep.type_name() == 'cmake'
  libxml_lib = libxml_dep.get_variable('LIBXML2_LIBRARIES')
  extra_env += {'SYSTEM_DEPS_LIBXML2_NO_PKG_CONFIG': '1',
                'SYSTEM_DEPS_LIBXML2_SEARCH_NATIVE': fs.parent(libxml_lib),
                'SYSTEM_DEPS_LIBXML2_LIB': fs.stem(libxml_lib)}
endif

build_gir = get_option('introspection').require(not meson.is_cross_build() and gi_dep.found())
build_vala = get_option('vala').require(not meson.is_cross_build() and vapigen_dep.found())

build_gdk_pixbuf = get_option('pixbuf-loader').require(pixbuf_dep.found())

build_docs = get_option('docs')

build_tests = get_option('tests').require(pixbuf_dep.found(), error_message: 'Tests require gdk-pixbuf to be available')

gnome = import('gnome')

foundation_dep = dependency('appleframeworks', modules: 'Foundation',
                            required: system in ['darwin', 'ios'])
m_dep = cc.find_library('m', required: false)

# ignore the system dependencies for GIR
library_dependencies_sole = [
  cairo_dep,
  cairogobj_dep,
  cairo_png_dep,
  freetype2_dep,
  pixbuf_dep,
  gio_dep,
  glib_dep,
  harfbuzz_dep,
  libxml_dep,
  pangocairo_dep,
  pangoft2_dep,
  gmodule_dep,
]

library_dependencies = library_dependencies_sole

if system == 'windows'
  rustc_native_libs_harness = find_program('meson/query-rustc-native-libs.py', native: true, required: get_option('default_library') != 'shared')

  native_libs = run_command(
    [
      rustc_native_libs_harness,
      rustc,
    ],
    capture: true,
    check: true
  )

  foreach i: native_libs.stdout().split()
    if i != 'msvcrt'
      library_dependencies += cc.find_library(i, required: get_option('default_library') != 'shared')
    endif
  endforeach
endif

library_dependencies += [m_dep, foundation_dep]

cargo_toml = meson.project_source_root() / 'Cargo.toml'

if get_option('default_library') == 'static'
  extra_env += {
    # Tell the pkg-config crate to think of all libraries as static
    'PKG_CONFIG_ALL_STATIC': '1',
    # Tell the system-deps crate to process linker flag for static deps
    'SYSTEM_DEPS_LINK': 'static',
    # Allow cross-compilation
    'PKG_CONFIG_ALLOW_CROSS': '1'
  }
endif

pkg_config = find_program('pkgconfig', 'pkg-config', 'pkgconf', required: true)
extra_env += {'PKG_CONFIG': pkg_config.full_path()}

pkg_config_path = get_option('pkg_config_path')
if pkg_config_path.length() > 0
  pathsep = ':'
  if system == 'windows'
    pathsep = ';'
  endif
  extra_env += {'PKG_CONFIG_PATH': pathsep.join(pkg_config_path)}
endif

# used in librsvg/meson.build.  This is so that '#include <librsvg/rsvg.h>` will pick
# up exactly that file from the source tree.
includeinc = include_directories('include')

# Set the suffixes up
if system == 'windows'
  lib_prefix = ''
  ext_dynamic = 'dll'
  ext_import = 'dll.lib'
  ext_static = 'lib'
  ext_exe = '.exe'
elif system == 'darwin'
  lib_prefix = 'lib'
  ext_dynamic = 'dylib'
  ext_static = 'a'
  ext_exe = ''
else
  lib_prefix = 'lib'
  ext_dynamic = 'so'
  ext_static = 'a'
  ext_exe = ''
endif

cargo_wrapper_args = [
  '--cargo', cargo.full_path(),
  '--manifest-path', cargo_toml,
  '--project-build-root', meson.project_build_root(),
  '--prefix', get_option('prefix'),
  '--libdir', get_option('libdir'),
]

# Ideally, cc.get_argument_syntax() == 'msvc' should
# cover this, but 'clang' can mean GCC or MSVC-style
# depending on the env and/or flags on Windows
is_msvc_style = cc.get_define('_MSC_VER') != ''

if not get_option('debug') or \
   (is_msvc_style and get_option('buildtype') == 'debugoptimized')
  cargo_wrapper_args += ['--release']
endif

target = get_option('triplet')

if target != ''
  cargo_wrapper_args += ['--target', target]
endif

subdir('include')
subdir('librsvg-c')
subdir('rsvg')
if build_tests.allowed()
  subdir('librsvg-c/tests-c')
endif
subdir('rsvg_convert')
if build_gdk_pixbuf.allowed()
  subdir('gdk-pixbuf-loader')
endif
if build_docs.allowed()
  subdir('doc')
endif

# The Vala generator must reside in the main folder since Meson isn't yet
# clever enough to take a list of files.
# See https://github.com/mesonbuild/meson/blob/b290a8254122f53d6477b7ba24366d81cfc5e99c/mesonbuild/modules/gnome.py#L2150-L2152
if build_gir.allowed() and build_vala.allowed()
  vala = gnome.generate_vapi(
    'librsvg-@0@'.format(librsvg_api_version),
    packages: vala_includes,
    metadata_dirs: [meson.current_source_dir()],
    sources: ['Rsvg-@0@-custom.vala'.format(librsvg_api_version), rsvg_gir[0]],
    install: true,
  )
endif
