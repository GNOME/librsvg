gdkpixbuf_loader = library('pixbufloader-svg', 'io-svg.c',
                           c_args : ['-DGDK_PIXBUF_ENABLE_BACKEND',
                                     '-DG_LOG_DOMAIN="libpixbufloader-svg"'],
                           dependencies : librsvg_dep)

executable('rsvg-loader', 'test.c',
           link_depends : gdkpixbuf_loader,
           dependencies : librsvg_dep,
           install : false)
