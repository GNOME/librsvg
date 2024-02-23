pixbufloader_sources = files(
  'Cargo.toml',
	'src/lib.rs',
)

pixbufloader_svg_def = configure_file(
  command: [makedef_args, '--list', '@INPUT@'],
  input: files(
    'gdk-pixbuf-loader.symbols'
  ),
  output: 'gdk-pixbuf-loader.def',
  capture: true,
)

version_script = meson.current_build_dir() / 'gdk-pixbuf-loader.def'
if host_machine.system() in ['darwin', 'ios']
  vflag = '-Wl,-exported_symbols_list,@0@'.format(version_script)
else
  vflag = '-Wl,--version-script,@0@'.format(version_script)
endif

pixbufloader_link_args = cc.get_supported_link_arguments([vflag]) + strip_link_args

pixbufloader_svg = shared_library(
  'pixbufloader-svg',
  files(
    'io-svg.c'
  ),
  c_args: ['-DGDK_PIXBUF_ENABLE_BACKEND'],
  link_args: pixbufloader_link_args,
  dependencies: librsvg_dep,
  vs_module_defs: pixbufloader_svg_def,
  install: true,
  install_dir: pixbuf_dep.get_variable(pkgconfig: 'gdk_pixbuf_moduledir'),
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
