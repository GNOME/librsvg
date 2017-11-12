test_env = [
  'G_TEST_SRCDIR=' + meson.current_source_dir(),
  'G_TEST_BUILDDIR=' + meson.current_build_dir(),
  'G_DEBUG=gc-friendly',
  'MALLOC_CHECK_=2',
]

test_common_src = 'test-utils.c'

test_progs = ['loading', 'rsvg-test', 'crash', 'render-crash', 'dimensions']
# styles

foreach t : test_progs
  test_src = t + '.c'
  e = executable(t, test_src, test_common_src,
                 dependencies : librsvg_dep,
                 install : false)
  test(t, e, env : test_env)
endforeach
