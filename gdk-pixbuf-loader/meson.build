pixbufloader_sources = files(
  'Cargo.toml',
  'src/lib.rs',
)

pixbufloader = custom_target(
  'pixbufloader-svg',
  build_by_default: true,
  output: ['@0@pixbufloader_svg.@1@'.format(lib_prefix, ext_dynamic)],
  console: true,
  install: true,
  install_dir: pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_moduledir'),
  depend_files: pixbufloader_sources,
  env: extra_env,
  command: [
    cargo_wrapper,
    cargo_wrapper_args,
    '--command=build',
    '--current-build-dir', '@OUTDIR@',
    '--current-source-dir', meson.current_source_dir(),
    '--packages', 'pixbufloader-svg',
    '--extension', ext_dynamic
  ]
)

pixbuf_thumbnailer = configure_file(
  input: 'librsvg.thumbnailer.in',
  output: 'librsvg.thumbnailer',
  configuration: {
    'bindir': get_option('prefix') / get_option('bindir')
  },
  install: true,
  install_dir: get_option('datadir') / 'thumbnailers'
)

gdk_pixbuf_query_loaders = find_program(pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_query_loaders', default_value: 'gdk-pixbuf-query-loaders'))

pixbufloader_svg_install_args = [
  '--gdk-pixbuf-moduledir',
  pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_moduledir'),
  pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_query_loaders'),
  pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_cache_file')
]

# Tell people to run gdk-pixbuf-query-loaders manually for cross builds
if meson.is_cross_build()
  pixbufloader_svg_install_args += '--show-cross-message'
endif

meson.add_install_script(
  find_program('meson_install.py', native: true),
  pixbufloader_svg_install_args
)

if build_tests.allowed()
  test(
    'Rust tests (pixbufloader-svg)',
    cargo_wrapper,
    timeout: -1, # no timeout
    args: [
      cargo_wrapper_args,
      '--current-build-dir', meson.current_build_dir(),
      '--command=test',
      '--current-source-dir', meson.current_source_dir(),
      '--packages', 'pixbufloader-svg',
    ],
    env: extra_env,
    depends: librsvg_lib
  )
endif
