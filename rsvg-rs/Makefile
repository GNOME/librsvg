GIR = gir/target/bin/gir
GIR_SRC = gir/Cargo.toml gir/Cargo.lock gir/build.rs $(shell find gir/src -name '*.rs')
GIR_FILES = gir-files/Rsvg-2.0.gir
SYS_FILES = rsvg-sys/src/lib.rs

src/auto/mod.rs : Gir.toml $(GIR) $(GIR_FILES) $(SYS_FILES)
	$(GIR) -c Gir.toml

.PHONY: clean
clean:
	rm -rf src/auto
	rm -rf $(SYS_FILES)

$(SYS_FILES): rsvg-sys/Gir.toml $(GIR) $(GIR_FILES)
	$(GIR) -c $< -o $(abspath rsvg-sys) -d gir-files

$(GIR) : $(GIR_SRC)
	rm -f gir/target/bin/gir
	cargo install --path gir --root gir/target
	rm -f gir/target/.crates.toml

$(GIR_SRC) $(GIR_FILES) :
	git submodule update --init

.PHONY: gir
gir : src/auto/mod.rs

.PHONY: gir-sys
gir-sys : $(SYS_FILES)

