library_sources = files(
  'Cargo.toml',
  'build.rs',
  'src/accept_language.rs',
  'src/angle.rs',
  'src/api.rs',
  'src/aspect_ratio.rs',
  'src/bbox.rs',
  'src/color.rs',
  'src/cond.rs',
  'src/coord_units.rs',
  'src/css.rs',
  'src/dasharray.rs',
  'src/document.rs',
  'src/dpi.rs',
  'src/drawing_ctx.rs',
  'src/element.rs',
  'src/error.rs',
  'src/filter_func.rs',
  'src/filter.rs',
  'src/filters/blend.rs',
  'src/filters/bounds.rs',
  'src/filters/color_matrix.rs',
  'src/filters/component_transfer.rs',
  'src/filters/composite.rs',
  'src/filters/context.rs',
  'src/filters/convolve_matrix.rs',
  'src/filters/displacement_map.rs',
  'src/filters/error.rs',
  'src/filters/flood.rs',
  'src/filters/gaussian_blur.rs',
  'src/filters/image.rs',
  'src/filters/lighting.rs',
  'src/filters/merge.rs',
  'src/filters/mod.rs',
  'src/filters/morphology.rs',
  'src/filters/offset.rs',
  'src/filters/tile.rs',
  'src/filters/turbulence.rs',
  'src/float_eq_cairo.rs',
  'src/font_props.rs',
  'src/gradient.rs',
  'src/href.rs',
  'src/image.rs',
  'src/io.rs',
  'src/iri.rs',
  'src/layout.rs',
  'src/length.rs',
  'src/lib.rs',
  'src/limits.rs',
  'src/log.rs',
  'src/marker.rs',
  'src/node.rs',
  'src/paint_server.rs',
  'src/parsers.rs',
  'src/path_builder.rs',
  'src/path_parser.rs',
  'src/pattern.rs',
  'src/properties.rs',
  'src/property_defs.rs',
  'src/property_macros.rs',
  'src/rect.rs',
  'src/shapes.rs',
  'src/space.rs',
  'src/structure.rs',
  'src/style.rs',
  'src/surface_utils/iterators.rs',
  'src/surface_utils/mod.rs',
  'src/surface_utils/shared_surface.rs',
  'src/surface_utils/srgb.rs',
  'src/text.rs',
  'src/transform.rs',
  'src/ua.css',
  'src/unit_interval.rs',
  'src/url_resolver.rs',
  'src/util.rs',
  'src/viewbox.rs',
  'src/ua.css',
  'src/xml/attributes.rs',
  'src/xml/mod.rs',
  'src/xml/xml2_load.rs',
  'src/xml/xml2.rs',
)

rust_artifacts = custom_target(
  'librsvg-2',
  build_by_default: true,
  output: '@0@rsvg_@1@.@2@'.format(lib_prefix, librsvg_api_major, ext_static),
  console: true,
  install: false,
  depend_files: library_sources + librsvg_c_sources,
  env: extra_env,
  command: [
    cargo_wrapper,
    cargo_wrapper_args,
    '--command=cbuild',
    '--current-build-dir', '@OUTDIR@',
    '--current-source-dir', meson.current_source_dir(),
    '--packages', 'librsvg-c',
    '--extensions', ext_static,
  ]
)

librsvg_c_lib = rust_artifacts[0]

makedef_args = [
  makedef,
  '--regex',
  '^rsvg_.',
]

if system in ['darwin', 'ios']
  makedef_args += ['--os', 'darwin']
elif system in ['windows', 'cygwin']
  makedef_args += ['--os', 'win']
else
  makedef_args += ['--os', 'linux']
endif

if cc.symbols_have_underscore_prefix()
  makedef_args += ['--prefix', '_']
else
  makedef_args += ['--prefix', '']
endif

makedef_tool_args = []

if cc.get_define('_MSC_VER') != ''
  dumpbin = find_program('dumpbin', native: true)
  makedef_tool_args += ['--dumpbin', dumpbin]
else
  nm = find_program('nm', native: true)
  makedef_tool_args += ['--nm', nm]
endif

librsvg_def = configure_file(
  command: [makedef_args, '--list', '@INPUT@'],
  input: files(
    '../win32/librsvg.symbols'
  ),
  output: '@0@.def'.format(librsvg_pc),
  capture: true,
)

librsvg_ver = custom_target(
  '@0@.ver'.format(librsvg_pc),
  command: [makedef_args, makedef_tool_args, '@INPUT@'],
  input: librsvg_c_lib,
  output: '@0@.ver'.format(librsvg_pc),
  capture: true,
)

version_script = meson.current_build_dir() / '@0@.def'.format(librsvg_pc)
if host_machine.system() in ['darwin', 'ios']
  vflag = '-Wl,-exported_symbols_list,@0@'.format(version_script)
else
  vflag = '-Wl,--version-script,@0@'.format(version_script)
endif

# This is not strictly needed, but since we are telling Cargo to build a staticlib, it puts in
# all of Rust's standard library and code from dependencies even when it is not needed.
# With the following, we shrink the final .so size; see issue #965.
#
# Amyspark: this is still needed after switching to cargo-cbuild, since
# GIR requires a shared library under MSVC, and Meson will only accept it
# if the library is a Meson generated target.
# No further flags are needed under MSVC, because LINK will already strip
# the library by default.
strip_link_args = cc.get_supported_link_arguments(
  '-Wl,-dead_strip',
  '-Wl,--gc-sections'
)

link_args = cc.get_supported_link_arguments([vflag]) + strip_link_args

# Some systems, reportedly OpenBSD and macOS, refuse
# to create libraries without any object files. Compiling
# this file, and adding its object file to the library,
# will prevent the library from being empty.
if cc.has_function_attribute('unused')
  rsvg_dummy = configure_file(
    command: [
      python,
      '-c',
      'print("static int __attribute__((unused)) __rsvg_dummy;")'
    ],
    capture: true,
    output: '_rsvg_dummy.c'
  )
else
  rsvg_dummy = configure_file(
    command: [
      python,
      '-c',
      'print("static int __rsvg_dummy; int dummy(void) { return __rsvg_dummy; }")'
    ],
    capture: true,
    output: '_rsvg_dummy.c'
  )
endif

librsvg_lib = shared_library(
  rsvg_ver,
  rsvg_dummy,
  link_whole: librsvg_c_lib,
  link_args: link_args,
  link_depends: librsvg_ver,
  dependencies: library_dependencies,
  include_directories: [includeinc],
  vs_module_defs: librsvg_ver, # librsvg_def
  install: true,
  version: meson.project_version() # it's not exact as m4
)

# This is the main dependency to use for tests; it means "link to the library we just built"
librsvg_dep = declare_dependency(
  sources: includes + header,
  include_directories: [includeinc],
  dependencies: library_dependencies,
  link_with: librsvg_lib,
)

pkg = import('pkgconfig')

librsvg_c_pc = pkg.generate(
  name : 'librsvg',
  filebase: librsvg_pc,
  description : 'library that renders svg files',
  libraries : librsvg_lib,
  subdirs: librsvg_pc,
)

if build_gir.allowed()
  env = environment()
  if system in ['darwin', 'ios']
    var = 'DYLD_LIBRARY_PATH'
  elif system in ['windows', 'cygwin']
    var = 'PATH'
  else
    var = 'LD_LIBRARY_PATH'
  endif

  current_library_path = run_command(
    [
      python,
      '-c',
      'import os; print(os.environ["@0@"]) if "@0@" in os.environ else ""'.format(var)
    ],
    capture: true,
    check: true,
  )
  env.set(var, current_library_path.stdout().strip())
  if system == 'windows'
    env.prepend('PATH', meson.current_build_dir() / 'rsvg')
    foreach i: library_dependencies_sole
      x = i.get_variable(pkgconfig: 'bindir', default_value: '')
      if x != ''
        env.prepend('PATH', x)
      endif
    endforeach
  endif

  gir_args = [
    '-DRSVG_COMPILATION'
  ]

  gir_includes = [
    'GLib-2.0',
    'GObject-2.0',
    'Gio-2.0',
    'cairo-1.0',
  ]

  vala_includes = [
    'gio-2.0',
    'cairo',
  ]

  if build_gdk_pixbuf.allowed()
    gir_includes += [
      'GdkPixbuf-2.0',
    ]

    vala_includes += [
      'gdk-pixbuf-2.0',
    ]
  endif

  rsvg_gir = gnome.generate_gir(
    librsvg_lib,
    namespace: 'Rsvg',
    nsversion: '2.0',
    symbol_prefix: [ 'rsvg', 'librsvg' ],
    dependencies: library_dependencies,
    includes: gir_includes,
    env: env,
    export_packages: librsvg_pc,
    header: ['librsvg/rsvg.h'],
    install: true,
    sources: includes,
    extra_args: gir_args,
    fatal_warnings: get_option('werror'),
  )
endif

if build_tests.allowed()
  library_test_sources = files(
    'tests/api.rs',
    'tests/bugs.rs',
    'tests/errors.rs',
    'tests/filters.rs',
    'tests/geometries.rs',
    'tests/intrinsic_dimensions.rs',
    'tests/loading_crash.rs',
    'tests/loading_disallowed.rs',
    'tests/primitive_geometries.rs',
    'tests/primitives.rs',
    'tests/reference.rs',
    'tests/render_crash.rs',
    'tests/shapes.rs',
    'tests/text.rs',
    'src/test_utils/compare_surfaces.rs',
    'src/test_utils/mod.rs',
    'src/test_utils/reference_utils.rs',
  )

  # HACK: Meson tests don't take OptionString nor ExternalProgram
  c_w_a = []

  test(
    'Rust tests',
    cargo_wrapper,
    timeout: -1, # no timeout
    args: [
      cargo_wrapper_args,
      '--current-build-dir', meson.current_build_dir(),
      '--command=test',
      '--current-source-dir', meson.current_source_dir(),
    ],
    env: extra_env,
    depends: librsvg_lib
  )
endif
