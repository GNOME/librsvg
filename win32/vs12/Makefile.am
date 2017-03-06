GENERATED_ITEMS =			\
	rsvg.vcxproj			\
	rsvg.vcxproj.filters		\
	rsvg-convert.vcxproj		\
	rsvg-convert.vcxproj.filters	\
	rsvg-install.props

EXTRA_DIST =	\
	librsvg.sln				\
	rsvg-build-defines.props		\
	rsvg-gen-srcs.props			\
	rsvg-install.propsin			\
	rsvg-version-paths.props		\
	libpixbufloader-svg.vcxproj		\
	libpixbufloader-svg.vcxproj.filters	\
	rsvg.vcxprojin				\
	rsvg.vcxproj.filtersin			\
	rsvg-convert.vcxprojin			\
	rsvg-convert.vcxproj.filtersin		\
	rsvg-install.vcxproj			\
	rsvg-install.vcxproj.filters		\
	rsvg-rust.vcxproj			\
	rsvg-view-3.vcxproj			\
	rsvg-view-3.vcxproj.filters		\
	README.txt				\
	$(GENERATED_ITEMS)

rsvg-install.props: $(top_srcdir)/win32/vs12/rsvg-install.propsin rsvg.vs12.headers
	-$(RM) $(top_builddir)/win32/vs14/rsvg-install.props
	-$(RM) $(top_builddir)/win32/vs15/rsvg-install.props
	$(CPP) -P - <$(top_srcdir)/win32/vs12/rsvg-install.propsin >$@
	rm rsvg.vs12.headers

DISTCLEANFILES = $(GENERATED_ITEMS)

-include $(top_srcdir)/git.mk
