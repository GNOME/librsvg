Fuzzing with cargo-fuzz
=======================

* `cargo install cargo-fuzz`

* `rustup default nightly` - cargo-fuzz requires the nightly compiler, unfortunately.

* `cargo fuzz run fuzz_target_1`

To pass options to the fuzzer, do it after `--`, for example:

```
cargo fuzz run fuzz_target_1 -- -seed_inputs=corpus1.svg,corpus2.svg,corpus3.svg -only_ascii=1
```

To get a list of available options, `cargo fuzz run fuzz_target_1 -- -help=1`

See `../afl-fuzz/README.md` for a to-do list for people who want to help with fuzzing.
