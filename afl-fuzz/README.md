Fuzzing with afl-fuzz
=====================

FIXME: Running these commands sucks.  Need a script or something.

To use `cargo afl`:

* Install `binutils-gold` (your program won't build otherwise).
* `cargo install cargo-afl`

To build and run the fuzzer:

```
cargo afl build --release
AFL_SKIP_CPUFREQ=1 cargo afl fuzz -i input/ -o out -S f0 target/release/rsvg-afl-fuzz
```

For each CPU core, change `-S f0` for `-S f1`, `-S f2`, etc.  To use
multiple CPU cores, run that command with a different `-S` option for
each core; see [the multicore
documentation](https://github.com/AFLplusplus/AFLplusplus/blob/stable/docs/fuzzing_in_depth.md#c-using-multiple-cores)
for details on changing the fuzz configuration for each job.

AFL complained when my kernel's configuration for corefiles was this:

```
$ cat /proc/sys/kernel/core_pattern 
|/bin/false
```

Set it with `echo core > /proc/sys/kernel/core_pattern` and AFL was
happy.  Alternatively, set `AFL_I_DONT_CARE_ABOUT_MISSING_CRASHES=1`
but you won't get corefiles.

Note: afl++ comes with pre-written dictionaries for svg/css/xml
(slightly incomplete, but a useful starting point).  However, these
**don't work out of the box** because the `-x` option to include a
dictionary (or a directory with dictionaries) complains if a
dictionary file is bigger than **128 bytes**.  This is... very
limited?  We may need to split the SVG dictionary into many little
files.

TODO: Help Wanted!
------------------

* We shouldn't fuzz on the CI machines, but we should make it easy for
  people to run the fuzzing framework.  Make a script (and fix the CI
  container image) to do the above well, taking the following into
  account.
  
* Investigate [the many options in
  afl++](https://github.com/AFLplusplus/AFLplusplus/blob/stable/docs/fuzzing_in_depth.md).
  For example, it recommends setting up a main fuzzer instance with
  `-M` and secondary fuzzers with `-S` for each additional core.
  Also, it recommends using different mutators and power schedules -
  no idea what those do.
  
* Improve the corpus.  The files in `input/` are basic SVGs but they
  do not exercise every major feature of librsvg.  Maybe pick up
  relevant files from the test suite and drop them in there.

* Write dictionaries that can actually be consumed by afl++.  See the
  comment above on dictionary files needing to be below 128 bytes in
  size.

* The afl++ documentation mentions that its output directory is pretty
  taxing on I/O systems, and for example SSDs.  It recommends using a
  tmpfs as a RAM disk for that.  Integrate that into the scripts.

* Do we need a VM with a kernel setup up just like afl++ likes it?
