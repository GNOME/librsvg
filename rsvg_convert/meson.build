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

rsvg_convert = custom_target(
  'rsvg-convert',
  build_by_default: true,
  output: 'rsvg-convert@0@'.format(ext_exe),
  console: true,
  install: true,
  install_dir: get_option('prefix') / get_option('bindir'),
  depends: rust_artifacts,
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
  ]
)
