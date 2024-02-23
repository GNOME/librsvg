api_test_dependencies = [
  cairo_dep,
  freetype2_dep,
  glib_dep,
  gio_dep,
  harfbuzz_dep,
  pango_dep,
  pangocairo_dep,
  pixbuf_dep,
]

api_test_sources = files(
  'api.c',
  'test-utils.c',
  'test-utils.h',
)

if build_tests.allowed()
  api_test = executable(
    'api',
    api_test_sources,
    c_args: [
      '-DTEST_DATA_DIR=@0@'.format(meson.current_source_dir()),
      '-DTEST_SRC_DIR=@0@'.format(meson.current_build_dir()),
      '-DTOP_SRC_DIR=@0@'.format(meson.project_source_root()),
    ],
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
endif
