includes = files(
    'rsvg.h',
    'rsvg-cairo.h',
    'rsvg-features.h'
)

header_version = meson.project_version().split('.')

header = configure_file(
    input: 'rsvg-version.h.in',
    output: 'rsvg-version.h',
    configuration: {
        'LIBRSVG_MAJOR_VERSION': header_version[0],
        'LIBRSVG_MINOR_VERSION': header_version[1],
        'LIBRSVG_MICRO_VERSION': header_version[2],
        'PACKAGE_VERSION': meson.project_version(),
    }
)

includes += [header]
install_headers(
    includes,
    subdir: '@0@/librsvg'.format(librsvg_pc),
)
