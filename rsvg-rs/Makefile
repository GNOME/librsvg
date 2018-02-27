RUSTDOC_STRIPPER = target/rustdoc-stripper/bin/rustdoc-stripper
GIR = target/gir/bin/gir
GIR_FILES = $(shell pkg-config --variable girdir gobject-introspection-1.0)
SYS_FILES = rsvg-sys/src/lib.rs
DOCS = vendor.md

src/auto/mod.rs : Gir.toml $(GIR) $(RUSTDOC_STRIPPER) $(GIR_FILES) $(SYS_FILES) $(DOCS)
	$(GIR) -c Gir.toml -d $(GIR_FILES)
	$(RUSTDOC_STRIPPER) -g -o $(DOCS) -d src/auto

$(DOCS): Gir.toml $(GIR) $(GIR_FILES)
	$(GIR) -c $< -d $(GIR_FILES) -m doc

.PHONY: clean
clean:
	rm -rf src/auto $(SYS_FILES) $(DOCS)

$(SYS_FILES): rsvg-sys/Gir.toml $(GIR) $(GIR_FILES)
	$(GIR) -c $< -o $(abspath rsvg-sys) -d $(GIR_FILES)

$(RUSTDOC_STRIPPER) :
	cargo install --root target/rustdoc-stripper rustdoc-stripper --version "0.1.5"

$(GIR) :
	cargo install --root target/gir --git https://github.com/gtk-rs/gir --rev d50d839ceaed9cc5eabac729dbc161c295306270 gir

.PHONY: gir
gir : src/auto/mod.rs

.PHONY: gir-sys
gir-sys : $(SYS_FILES)

