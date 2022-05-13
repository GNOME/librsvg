DOCS_SRC =					\
	librsvg.toml				\
	load-and-render.c			\
	migrating.md				\
	overview.md				\
	recommendations.md			\
	urlmap.js				\
	$(NULL)

EXTRA_DIST = $(DOCS_SRC)

HEADERS_WITH_DOCS = $(top_srcdir)/include/librsvg/*.h
GIR_FILE = $(top_builddir)/Rsvg-2.0.gir

CLEANFILES = build.stamp

build.stamp: $(DOCS_SRC) $(HEADERS_WITH_DOCS) $(GIR_FILE)
	$(GI_DOCGEN) generate -C $(srcdir)/librsvg.toml --content-dir=$(top_srcdir)/doc $(GIR_FILE)
	touch build.stamp

clean-local:
	rm -rf Rsvg-2.0

# This is the devhelp convention for documentation paths; it will look for the .devhelp file there
DOC_INSTALL_DIR=$(DESTDIR)/$(datadir)/doc/Rsvg-2.0

install-data-local: build.stamp
	$(MKDIR_P) $(DOC_INSTALL_DIR)
	for f in Rsvg-2.0/*; do $(INSTALL_DATA) $$f $(DOC_INSTALL_DIR); done

uninstall-local:
	rm -rf $(DOC_INSTALL_DIR)

all-local: build.stamp
