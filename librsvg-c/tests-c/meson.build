api_test_sources = files(
  'api.c',
  'test-utils.c',
  'test-utils.h',
)

test_c_args = [
  '-DTEST_DATA_DIR=@0@'.format(meson.current_source_dir()),
  '-DTEST_SRC_DIR=@0@'.format(meson.current_build_dir()),
  '-DTOP_SRC_DIR=@0@'.format(meson.project_source_root()),
]

if pixbuf_dep.found()
  test_c_args += ['-DHAVE_PIXBUF']
endif

api_test = executable(
  'api',
  api_test_sources,
  c_args: test_c_args,
  dependencies: librsvg_dep,
  gnu_symbol_visibility: 'hidden',
  install: false,
)

test(
  'C API tests',
  api_test,

  # the following incantation is for glib's gtestutils
  timeout: -1,
  args: [ '--tap', 'k' ],
  env: [
    'G_TEST_SRCDIR=@0@'.format(meson.current_source_dir()),
    'G_TEST_BUILDDIR=@0@'.format(meson.current_build_dir()),
  ]
)
