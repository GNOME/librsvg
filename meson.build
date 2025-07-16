project('librsvg', 'c',
        version: '2.40.23',
        meson_version: '>= 1.1',
        default_options: [
          'c_std=gnu11', 'warning_level=1', 'werror=false'
        ]
)

if not ['shared', 'static'].contains(get_option('default_library'))
  error('Unsupported default_library value: @0@'.format(get_option('default_library')))
endif

gnome = import('gnome')
cc = meson.get_compiler('c')

ver_split = meson.project_version().split('.')
librsvg_api_version = '2.0'
api_split = librsvg_api_version.split('.')
librsvg_api_major = api_split[0]
librsvg_api_minor = api_split[1]

rsvg_ver = 'rsvg-@0@'.format(librsvg_api_major)

glib_required          = '>= 2.38.0'
libxml_required        = '>= 2.9.0'
cairo_required         = '>= 1.2.0'
pango_required         = '>= 1.38.0'
gdk_pixbuf_required    = '>= 2.20'
croco_required         = '>= 0.6.1'

glib_dep       = dependency('glib-2.0',
                            version: glib_required)
gio_dep        = dependency('gio-2.0',
                            version: glib_required)
libxml_dep     = dependency(['libxml-2.0', 'LibXml2'],
                            version: libxml_required,
                            default_options: ['python=false'])
cairo_dep      = dependency('cairo',
                            version: cairo_required)
cairo_png_dep  = dependency('cairo-png',
                            version: cairo_required)
pangocairo_dep = dependency('pangocairo',
                            version: pango_required)
pangoft2_dep   = dependency('pangoft2',
                            version: pango_required,
                            required: false)
gdk_pixbuf_dep = dependency('gdk-pixbuf-2.0',
                            version: gdk_pixbuf_required)
croco_dep      = dependency('libcroco-0.6',
                            version: croco_required,
                            default_options: ['default_library=static'])

libm           = cc.find_library('m', required: false)

librsvg_includedir = join_paths(
  get_option('includedir'),
  'librsvg-@0@'.format(librsvg_api_version),
  'librsvg'
)

config = configuration_data()
config.set('HAVE_PANGOFT2', pangoft2_dep.found())
config.set('HAVE_STRINGS_H', cc.has_header('strings.h'))
config.set('HAVE_STRTOK_R', cc.has_function('strtok_r', prefix: '#include <string.h>'))
config.set('HAVE_FLOAT_H', cc.has_header('float.h'))
config.set_quoted('VERSION', meson.project_version())
configure_file(output: 'config.h', configuration: config)

features = configuration_data()
features.set('LIBRSVG_MAJOR_VERSION', ver_split[0])
features.set('LIBRSVG_MINOR_VERSION', ver_split[1])
features.set('LIBRSVG_MICRO_VERSION', ver_split[2])
features.set('PACKAGE_VERSION', meson.project_version())
configure_file(input: 'librsvg-features.h.in',
               output: 'librsvg-features.h',
               configuration: features,
               install: true,
               install_dir: librsvg_includedir)

librsvg_inc_priv = include_directories('librsvg')

librsvg_headers = [
  'librsvg/rsvg.h',
  'rsvg-cairo.h'
]

install_headers(librsvg_headers,
  install_dir : librsvg_includedir)

librsvg_src = [
  'librsvg-features.c',
  'rsvg-css.c',
  'rsvg-defs.c',
  'rsvg-image.c',
  'rsvg-io.c',
  'rsvg-paint-server.c' ,
  'rsvg-path.c',
  'rsvg-base-file-util.c' ,
  'rsvg-filter.c',
  'rsvg-marker.c',
  'rsvg-mask.c',
  'rsvg-shapes.c',
  'rsvg-structure.c',
  'rsvg-styles.c',
  'rsvg-text.c',
  'rsvg-cond.c',
  'rsvg-base.c',
  'rsvg-cairo-draw.c',
  'rsvg-cairo-render.c',
  'rsvg-cairo-clip.c',
  'rsvg.c',
  'rsvg-gobject.c',
  'rsvg-file-util.c',
  'rsvg-size-callback.c',
  'rsvg-xml.c',
]

enums = gnome.mkenums_simple(
  'librsvg-enum-types',
  sources: librsvg_headers,
  install_header: true,
  install_dir: librsvg_includedir
)

librsvg_def_file = disabler() 
if cc.get_argument_syntax() == 'msvc' and get_option('default_library') == 'shared'
  # can't use the cc.preprocessor() for VS 2008 or earlier
  if cc.version().split('.')[0].to_int() >= 16
    librsvg_symbols = cc.preprocess('rsvg.symbols', output: 'rsvg.def.pre')
  else
    librsvg_symbols = custom_target(
      'rsvg.def.pre',
      capture: true,
      input: 'rsvg.symbols',
      output: 'rsvg.def.pre',
      command: cc.cmd_array() + ['-EP', '@INPUT@']
    )
  endif
  librsvg_def_file = custom_target(
    'rsvg.def',
    depends: librsvg_symbols,
    input: librsvg_symbols,
    output: 'rsvg.def',
    command: ['format-symbols.py', '@INPUT@', '@OUTPUT@']
  )
endif

rsvg_lib = library(rsvg_ver, librsvg_src, enums,
  c_args: [
    '-DRSVG_COMPILATION',
    '-DG_LOG_DOMAIN="librsvg"',
    '-DSRCDIR="@0@"'.format(meson.project_source_root()),
    '-DRSVG_DISABLE_DEPRECATION_WARNINGS',
  ],
  include_directories: librsvg_inc_priv,
  dependencies: [
    gdk_pixbuf_dep,
    glib_dep,
    gio_dep,
    libxml_dep,
    pangocairo_dep,
    pangoft2_dep,
    cairo_dep,
    cairo_png_dep,
    croco_dep,
    libm
  ],
  install: true,
  soversion: ver_split[0],
  vs_module_defs: librsvg_def_file,
)

librsvg_ld_args = []
if get_option('default_library') == 'static' and cc.has_link_argument('-Wl,--exclude-libs=ALL')
  librsvg_ld_args += '-Wl,--exclude-libs=@0@'.format(rsvg_lib.full_path())
endif

librsvg_dep = declare_dependency(
  # guard against race conditions where dependents will start compiling
  # before mkenums finished
  sources: [enums[1]],
  link_with: rsvg_lib,
  link_args: librsvg_ld_args,
  include_directories: include_directories('.'),
  dependencies: [
    glib_dep,
    gio_dep,
    gdk_pixbuf_dep,
    cairo_dep
  ]
)

if get_option('tests').allowed()
  subdir('tests')
endif

if get_option('rsvg-convert').allowed()
  executable('rsvg-convert', 'rsvg-convert.c', 'rsvg-size-callback.c',
    include_directories: librsvg_inc_priv,
    dependencies: [
      librsvg_dep,
      dependency('gio-unix-2.0', version: glib_required, required: false),
      dependency('gio-windows-2.0', version: glib_required, required: false),
      libm
    ],
    install: true
  )
endif

gtk3_required = '>= 3.10.0'
gtk3_dep = dependency('gtk+-3.0', version: gtk3_required, required: get_option('rsvg-view-3'))
if gtk3_dep.found()
  executable('rsvg-view-3', 'test-display.c',
    # These are GTK3 deprecations, so we can ignore them; they're going to stay
    c_args: [cc.get_id() == 'msvc' ? '-wd4996' : '-Wno-deprecated-declarations'],
    include_directories: librsvg_inc_priv,
    dependencies: [
      librsvg_dep,
      # needed due to rsvg-private.h include
      libxml_dep.partial_dependency(compile_args: true, includes: true),
      pangocairo_dep.partial_dependency(compile_args: true, includes: true),
      gtk3_dep,
      libm
    ],
    install: true
  )
endif
