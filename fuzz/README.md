Fuzzing with cargo-fuzz
=======================

* `cargo install cargo-fuzz`

* `rustup default nightly` - cargo-fuzz requires the nightly compiler, unfortunately.

* `cargo fuzz run render_document`

## Configuring fuzzer runs
To pass options to the fuzzer, do it after `--`, for example:

```
cargo fuzz run render_document -- -seed_inputs=corpus1.svg,corpus2.svg,corpus3.svg -only_ascii=1
```

To get a list of available options, `cargo fuzz run render_document -- -help=1`

### Using dictionaries
There are existing SVG, XML, and CSS dictionaries that can be used with fuzz targets:

```
curl https://raw.githubusercontent.com/google/fuzzing/master/dictionaries/{css,svg,xml}.dict > combined.dict

cargo fuzz run render_document corpus/ -- -dict=combined.dict
```

## Reproducing a failure
You can reproduce a failure by supplying a path to the fuzzed data:

`cargo fuzz run render_document fuzzed.svg`

Fuzz targets can also be run inside of a debugger for further debugging information:

```
FUZZ_TARGET=$(find ./target/*/release/ -type f -name render_document)
gdb --args "$FUZZ_TARGET" fuzzed.svg
```

## Suppressing leak reports
You can suppress spurious leak reports by specifying a suppressions file via the `LSAN_OPTIONS`
environment variable:

`LSAN_OPTIONS="suppressions=../tools/lsan.supp" cargo fuzz run render_document fuzzed.svg`

## Related documents
See `../afl-fuzz/README.md` for a to-do list for people who want to help with fuzzing.

See `../devel-docs/oss_fuzz.rst` for an overview of librsvg's integration with OSS-Fuzz.
