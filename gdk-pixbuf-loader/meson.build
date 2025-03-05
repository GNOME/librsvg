pixbufloader_sources = files(
  'Cargo.toml',
  'src/lib.rs',
)

prefix = get_option('prefix')

pixbufloader = custom_target(
  'pixbufloader-svg',
  build_by_default: true,
  output: ['@0@pixbufloader_svg.@1@'.format(cargo_dylib_prefix, ext_dynamic)],
  console: true,
  install: true,
  install_dir: pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_moduledir', pkgconfig_define: ['prefix', prefix]),
  depends: librsvg_rust_dep,
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

if meson.can_run_host_binaries()
  gdk_pixbuf_query_loaders = find_program(pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_query_loaders', default_value: 'gdk-pixbuf-query-loaders'))
endif

pixbufloader_svg_install_args = [
  '--gdk-pixbuf-moduledir',
  pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_moduledir', pkgconfig_define: ['prefix', prefix]),
  pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_query_loaders', pkgconfig_define: ['prefix', prefix]),
  pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_cache_file', pkgconfig_define: ['prefix', prefix])
]

# Tell people to run gdk-pixbuf-query-loaders manually for cross builds
if not meson.can_run_host_binaries()
  pixbufloader_svg_install_args += '--show-cross-message'
endif

meson.add_install_script(
  find_program('meson_install.py', native: true),
  pixbufloader_svg_install_args
)

if build_tests
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
    depends: pixbufloader
  )
endif
