librsvg_tools_main = static_library('rsvg_tools_main', 'rsvg-tools-main.c',
                                    dependencies : librsvg_dep)

executable('test-performance', 'test-performance.c',
           link_with : librsvg_tools_main,
           dependencies : librsvg_dep)

executable('rsvg-dimensions', 'rsvg-dimensions.c',
           link_with : librsvg_tools_main,
           dependencies : librsvg_dep)
