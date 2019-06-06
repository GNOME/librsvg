Fuzzing with afl-fuzz
=====================

FIXME: this README sucks, and running these commands sucks, too.  Need
a script or something.

```
cargo afl build --release
AFL_SKIP_CPUFREQ=1 cargo afl fuzz -i input/ -o out -S f0 target/release/rsvg-afl-fuzz
```

For each CPU core, change `-S f0` for `-S f1`, `-S f2`, etc.

AFL complained when my kernel's configuration for corefiles was this:

```
$ cat /proc/sys/kernel/core_pattern 
|/bin/false
```

Set it with `echo core > /proc/sys/kernel/core_pattern` and AFL was
happy.

