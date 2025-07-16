pixbuf_loader = shared_library('pixbufloader-svg', 'io-svg.c',
  dependencies: librsvg_dep,
  include_directories: librsvg_inc_priv,
  c_args: ['-DGDK_PIXBUF_ENABLE_BACKEND', '-DG_LOG_DOMAIN="libpixbufloader-svg"'],
  install: true,
  install_dir : pixbuf_loader_install_dir,
)