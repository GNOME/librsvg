project('librsvg', 'c', 'rust',
  version : '2.41.2',
  meson_version : '>=0.44')

rsvg_api_major = '2'
rsvg_api_minor = '0'
rsvg_api_version = rsvg_api_major + '.' + rsvg_api_minor

rsvg_versions = meson.project_version().split('.')

cc = meson.get_compiler('c')
host_system = host_machine.system()

datadir_abs = join_paths(get_option('prefix'), get_option('datadir'))
librsvg_inc_subdir = join_paths('librsvg-' + rsvg_api_version, 'librsvg')
librsvg_incdir = join_paths(get_option('includedir'), librsvg_inc_subdir)

# For generating config.h
cdata = configuration_data()
cdata.set_quoted('VERSION', rsvg_api_version)

glib_req = '>=2.24'
cairo_req = '>=1.2'
pango_req = '>=1.38'

glib_dep = [dependency('glib-2.0', version : glib_req),
            dependency('gio-2.0', version : glib_req)]
gthread_dep = dependency('gthread-2.0', version : glib_req)

if host_system != 'windows'
  gio_platform_dep = dependency('gio-unix-2.0', version : glib_req)
else
  gio_platform_dep = dependency('gio-windows-2.0', version : glib_req)
endif

cairo_dep = [dependency('cairo', version : cairo_req),
             dependency('cairo-png', version : cairo_req)]

pango_dep = [dependency('pangocairo', version : pango_req)]

pangoft2_dep = dependency('pangoft2', version : pango_req,
                          # Optional on Windows
                          required : host_system != 'windows')
cdata.set('HAVE_PANGOFT2', pangoft2_dep.found())

libxml2_dep = dependency('libxml-2.0', version : '>=2.9')
# XXX: should this be behind an option?
gtk3_dep = dependency('gtk+-3.0', version : '>=3.10', required : false)
pixbuf_dep = dependency('gdk-pixbuf-2.0', version : '>=2.20')
croco_dep = dependency('libcroco-0.6', version : '>=0.6.1')
libm = cc.find_library('m', required : false)
libdl = cc.find_library('dl', required : false)
thread_dep = dependency('threads')

librsvg_deps = [glib_dep, cairo_dep, pango_dep, pangoft2_dep, pixbuf_dep,
                libxml2_dep, croco_dep, thread_dep, libdl, libm]

cdata.set('HAVE_STRTOK_R', cc.has_function('strtok_r'))
cdata.set('HAVE_STRINGS_H', cc.has_header('strings.h'))

librsvg_link_args = []
if cc.has_argument('-Wl,-Bsymbolic-functions')
  librsvg_link_args += ['-Wl,-Bsymbolic-functions']
endif

# XXX: debug rust code
#
# TODO: introspection
# TODO: vala bindings
#
# TODO: LC_MESSAGES
cdata.set('HAVE_LC_MESSAGES', false)
configure_file(output : 'config.h',
               configuration : cdata)

# For generating librsvg.pc
pcdata = configuration_data()
pcdata.set('prefix', get_option('prefix'))
pcdata.set('exec_prefix', get_option('prefix'))
pcdata.set('libdir', join_paths('${prefix}', get_option('libdir')))
pcdata.set('includedir', join_paths('${prefix}', get_option('includedir')))
pcdata.set('VERSION', meson.project_version())
pcdata.set('RSVG_API_MAJOR_VERSION', rsvg_api_major)
pcdata.set('RSVG_API_VERSION', rsvg_api_version)
configure_file(input : 'librsvg.pc.in',
               output : 'librsvg.pc',
               configuration : pcdata,
               install_dir : join_paths(get_option('prefix'),
                                        get_option('libdir'),
                                        'pkgconfig'))

# Install headers
librsvg_headers = ['rsvg.h', 'rsvg-cairo.h']
install_headers(librsvg_headers, subdir : librsvg_inc_subdir)

# For generating and installing librsvg-features.h
ftdata = configuration_data()
ftdata.set('LIBRSVG_MAJOR_VERSION', rsvg_versions[0])
ftdata.set('LIBRSVG_MINOR_VERSION', rsvg_versions[1])
ftdata.set('LIBRSVG_MICRO_VERSION', rsvg_versions[2])
ftdata.set('PACKAGE_VERSION', meson.project_version())
configure_file(input : 'librsvg-features.h.in',
               output : 'librsvg-features.h',
               configuration : ftdata,
               install_dir : librsvg_incdir)

# Build the rust static library
subdir('rust')

# Generate and install enumtypes
gnome = import('gnome')
librsvg_enums = gnome.mkenums_simple('librsvg-enum-types',
                                     sources : librsvg_headers,
                                     install_dir : librsvg_incdir)

# Build C code
librsvg_sources = [
  'librsvg-features.c',
  'rsvg-css.c',
  'rsvg-defs.c',
  'rsvg-io.c',
  'rsvg-paint-server.c',
  'rsvg-base-file-util.c',
  'rsvg-filter.c',
  'rsvg-mask.c',
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
  librsvg_enums,
]
librsvg = library('rsvg-' + rsvg_api_major, librsvg_sources,
                  version : meson.project_version(),
                  dependencies : [librsvg_deps, librsvg_internals],
                  # FIXME: symbol visibility
                  link_args : librsvg_link_args,
                  c_args : ['-DG_LOG_DOMAIN="@0@"'.format(meson.project_name()),
                            '-DLIBRSVG_DATADIR="@0@"'.format(datadir_abs),
                            '-DSRCDIR="@0@"'.format(meson.source_root()),
                            '-DRSVG_DISABLE_DEPRECATION_WARNINGS',
                            '-DRSVG_COMPILATION'])
# For usage as a subproject
librsvg_dep = declare_dependency(link_with : librsvg,
                                 dependencies : librsvg_deps,
                                 sources : librsvg_enums[1],
                                 include_directories : include_directories('.'))

install_man('rsvg-convert.1')

executable('rsvg-convert', 'rsvg-convert.c', 'rsvg-size-callback.c',
           dependencies : [librsvg_dep, gio_platform_dep],
           c_args : ['-DLIBRSVG_DATADIR="@0@"'.format(datadir_abs)],
           install : true)

if gtk3_dep.found()
  executable('rsvg-view-3', 'rsvg-view.c',
             dependencies : [librsvg_dep, gthread_dep, gtk3_dep],
             c_args : ['-DLIBRSVG_DATADIR="@0@"'.format(datadir_abs)],
             gui_app : true,
             install : true)
endif

subdir('tests')
if get_option('pixbuf-loader')
  subdir('gdk-pixbuf-loader')
endif
if get_option('tools')
  subdir('tools')
endif
# TODO: gtk-doc
#subdir('doc')
