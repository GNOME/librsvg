rsvg_test_lib = static_library('rsvg-test-common', 'test-utils.c',
  dependencies: librsvg_dep,
  install: false
)

rsvg_tests = [
  { 'name': 'loading', 'sources': ['loading.c'] },
  { 'name': 'rsvg-test', 'sources': ['rsvg-test.c'] },
  { 'name': 'crash', 'sources': ['crash.c'] },
  {
    'name': 'styles',
    'sources': ['styles.c'],
    'dependencies': [
      # needed due to rsvg-private.h include
      libxml_dep.partial_dependency(compile_args: true, includes: true),
      pangocairo_dep.partial_dependency(compile_args: true, includes: true)
    ]
  },
  { 'name': 'render-crash', 'sources': ['render-crash.c'] },
  { 'name': 'dimensions', 'sources': ['dimensions.c'] },
  {
    'name': 'errors',
    'sources': ['errors.c'],
    # 515 billion-laughs needs more time on slow cpus
    'timeout': 90
  },
]

foreach decl : rsvg_tests
  test(
    decl['name'],
    executable(decl['name'], decl['sources'],
      include_directories: librsvg_inc_priv,
      link_with: rsvg_test_lib,
      dependencies: [
        librsvg_dep,
        decl.get('dependencies', [])
      ]
    ),
    protocol: 'tap',
    timeout: decl.get('timeout', 30),
    env: [
      'G_TEST_SRCDIR=@0@'.format(meson.current_source_dir()),
      'G_TEST_BUILDDIR=@0@'.format(meson.current_build_dir()),
    ]
  )
endforeach
