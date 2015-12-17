SUBDIRS = \
	vs9	\
	vs10	\
	vs11	\
	vs12	\
	vs14

if HAVE_INTROSPECTION
GENERATED_ITEMS = \
	introspection.body.mak	\
	Rsvg_@RSVG_API_VERSION_U@_gir_list

MSVC_INTROSPECTION_INTERMEDIATE_FILES = Rsvg-@RSVG_API_VERSION@.gir.msvc.introspect

introspection.body.mak: $(MSVC_INTROSPECTION_INTERMEDIATE_FILES)
	-$(RM) introspection.body.mak
	for F in `ls *.msvc.introspect`; do \
		case $$F in \
			*)	cat $(top_builddir)/build/win32/$$F >>introspection.body.mak \
			;; \
		esac; \
	done
	$(RM) $(MSVC_INTROSPECTION_INTERMEDIATE_FILES)

DISTCLEANFILES = $(GENERATED_ITEMS)

else
GENERATED_ITEMS =
endif

EXTRA_DIST = \
	rsvg-introspection-msvc.mak	\
	introspection-msvc.mak	\
	detectenv-msvc.mak	\
	$(GENERATED_ITEMS)
