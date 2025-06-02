includes = files(
  'rsvg-gtk.h'
)

sources = files(
  'paintable.c'
)

libgtk = library('librsvg-gtk',
                 sources: sources,
                 dependencies: [ gtk_dep, librsvg_dep])

install_headers(includes,
                subdir: '@0@/librsvg'.format(librsvg_pc))
