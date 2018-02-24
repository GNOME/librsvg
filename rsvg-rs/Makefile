GIR = target/gir/bin/gir
GIR_FILES_DIR = $(shell pkg-config --variable girdir gobject-introspection-1.0)
GIR_FILES = $(GIR_FILES_DIR)/Rsvg-2.0.gir
SYS_FILES = rsvg-sys/src/lib.rs

src/auto/mod.rs : Gir.toml $(GIR) $(GIR_FILES) $(SYS_FILES)
	$(GIR) -c Gir.toml -d $(GIR_FILES_DIR)

.PHONY: clean
clean:
	rm -rf src/auto $(SYS_FILES)

$(SYS_FILES): rsvg-sys/Gir.toml $(GIR) $(GIR_FILES)
	$(GIR) -c $< -o $(abspath rsvg-sys) -d $(GIR_FILES_DIR)

$(GIR) :
	cargo install --root target/gir --git https://github.com/gtk-rs/gir --rev d50d839ceaed9cc5eabac729dbc161c295306270 gir

.PHONY: gir
gir : src/auto/mod.rs

.PHONY: gir-sys
gir-sys : $(SYS_FILES)

