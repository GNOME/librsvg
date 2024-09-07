rsvg_convert_files = files(
  'Cargo.toml',
  'src/main.rs',
  'tests/internal_predicates/file.rs',
  'tests/internal_predicates/mod.rs',
  'tests/internal_predicates/pdf.rs',
  'tests/internal_predicates/png.rs',
  'tests/internal_predicates/svg.rs',
  'tests/rsvg_convert.rs',
)

rsvg_convert_output = ['rsvg-convert@0@'.format(ext_exe)]

if get_option('debug') and is_msvc_style
  rsvg_convert_output += ['rsvg-convert.pdb']
endif

rsvg_convert = custom_target(
  'rsvg-convert',
  build_by_default: true,
  output: rsvg_convert_output,
  console: true,
  install: true,
  install_dir: get_option('prefix') / get_option('bindir'),
  depends: librsvg_rust_dep,
  depend_files: rsvg_convert_files,
  env: extra_env,
  command: [
    cargo_wrapper,
    cargo_wrapper_args,
    '--command=build',
    '--current-build-dir', '@OUTDIR@',
    '--current-source-dir', meson.current_source_dir(),
    '--packages', 'rsvg_convert',
    '--bin', 'rsvg-convert'
  ] + avif_feature_args,
)

if build_tests
  test(
    'Rust tests (rsvg_convert)',
    cargo_wrapper,
    timeout: -1, # no timeout
    args: [
      cargo_wrapper_args,
      '--current-build-dir', meson.current_build_dir(),
      '--command=test',
      '--current-source-dir', meson.current_source_dir(),
      '--packages', 'rsvg_convert',
    ] + avif_feature_args,
    env: extra_env,
    depends: rsvg_convert
  )
endif
