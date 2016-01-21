GENERATED_ITEMS =			\
	rsvg.vcxproj			\
	rsvg.vcxproj.filters		\
	rsvg-convert.vcxproj		\
	rsvg-convert.vcxproj.filters	\
	rsvg-install.props		\
	math.h

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
	rsvg-view-3.vcxproj			\
	rsvg-view-3.vcxproj.filters		\
	README.txt				\
	$(GENERATED_ITEMS)

rsvg-install.props: $(top_srcdir)/build/win32/vs10/rsvg-install.propsin rsvg.vs10.headers
	$(CPP) -P - <$(top_srcdir)/build/win32/vs10/rsvg-install.propsin >$@
	rm rsvg.vs10.headers

math.h: $(top_srcdir)/build/win32/vs9/math.h
	cp $< $@

DISTCLEANFILES = $(GENERATED_ITEMS)

-include $(top_srcdir)/git.mk
