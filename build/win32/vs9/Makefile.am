GENERATED_ITEMS =		\
	rsvg.vcproj		\
	rsvg-convert.vcproj	\
	rsvg-install.vsprops

EXTRA_DIST =	\
	librsvg.sln			\
	rsvg-build-defines.vsprops	\
	rsvg-gen-srcs.vsprops		\
	rsvg-install.vspropsin		\
	rsvg-version-paths.vsprops	\
	libpixbufloader-svg.vcproj	\
	rsvg.vcprojin			\
	rsvg-convert.vcprojin		\
	rsvg-install.vcproj		\
	rsvg-view-3.vcproj		\
	README.txt			\
	math.h				\
	$(GENERATED_ITEMS)

rsvg-install.vsprops: $(top_srcdir)/build/win32/vs9/rsvg-install.vspropsin rsvg.headers
	$(CPP) -P - <$(top_srcdir)/build/win32/vs9/rsvg-install.vspropsin >$@
	rm rsvg.headers

DISTCLEANFILES = $(GENERATED_ITEMS)

-include $(top_srcdir)/git.mk
