SUBDIRS = vs12 vs14 vs15

if HAVE_INTROSPECTION
GENERATED_ITEMS = \
	introspection.body.mak	\
	Rsvg_@RSVG_API_VERSION_U@_gir_list

MSVC_INTROSPECTION_INTERMEDIATE_FILES = Rsvg-@RSVG_API_VERSION@.gir.msvc.introspect

introspection.body.mak: $(MSVC_INTROSPECTION_INTERMEDIATE_FILES)
	-$(RM) introspection.body.mak
	for F in `ls *.msvc.introspect`; do \
		case $$F in \
			*)	cat $(top_builddir)/win32/$$F >>introspection.body.mak \
			;; \
		esac; \
	done
	$(RM) $(MSVC_INTROSPECTION_INTERMEDIATE_FILES)

DISTCLEANFILES = $(GENERATED_ITEMS)

else
GENERATED_ITEMS =
endif

EXTRA_DIST = \
	build-rules-msvc.mak	\
	config-msvc.mak.in	\
	config-msvc.mak		\
	create-lists.bat	\
	create-lists-msvc.mak	\
	detectenv-msvc.mak	\
	generate-msvc.mak	\
	info-msvc.mak		\
	install-msvc.mak	\
	introspection-msvc.mak	\
	Makefile.vc		\
	README.txt		\
	rsvg-rust.mak		\
	rsvg-introspection-msvc.mak	\
	pc_base.py		\
	replace.py		\
	rsvgpc.py		\
	$(GENERATED_ITEMS)
