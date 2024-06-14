Fuzzing with cargo-fuzz
=======================

* `cargo install cargo-fuzz`

* `rustup default nightly` - cargo-fuzz requires the nightly compiler, unfortunately.

* `cargo fuzz run render_document`

To pass options to the fuzzer, do it after `--`, for example:

```
cargo fuzz run render_document -- -seed_inputs=corpus1.svg,corpus2.svg,corpus3.svg -only_ascii=1
```

To get a list of available options, `cargo fuzz run render_document -- -help=1`


## Reproducing a failure
You can reproduce a failure by supplying a path to the fuzzed data:

`cargo fuzz run render_document fuzzed.svg`

Fuzz targets can also be run inside of a debugger for further debugging information:

```
FUZZ_TARGET=$(find ./target/*/release/ -type f -name render_document)
gdb --args "$FUZZ_TARGET" fuzzed.svg
```

## Related documents
See `../afl-fuzz/README.md` for a to-do list for people who want to help with fuzzing.

See `../devel-docs/oss_fuzz.rst` for an overview of librsvg's integration with OSS-Fuzz.
