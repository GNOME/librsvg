pixbufloader_sources = files(
  'Cargo.toml',
	'src/lib.rs',
)

library_output = ['@0@pixbufloader_svg.@1@'.format(lib_prefix, ext_dynamic)]

pixbufloader = custom_target(
  'pixbufloader-svg',
  build_by_default: true,
  output: library_output,
  console: true,
  install: true,
  install_dir: pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_moduledir'),
  depend_files: pixbufloader_sources,
  env: extra_env,
  command: [
    cargo_wrapper,
    cargo_wrapper_args,
    '--command=build',
    '--current-source-dir', meson.current_source_dir(),
    '--packages', 'pixbufloader-svg',
    '--extensions', ext_dynamic
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

if not meson.is_cross_build()
  meson.add_install_script(
    find_program('meson_install.py', native: true),
    '--gdk-pixbuf-moduledir',
    pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_moduledir'),
    pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_query_loaders'),
    pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_cache_file')
  )
endif
