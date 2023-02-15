#!/usr/bin/env python3

# Benchmark name, Directory with SVGs to render
BENCHMARKS = [
    [ "hicolor-apps", "./hicolor-apps" ],
    [ "symbolic-icons", "../tests/fixtures/reftests/adwaita" ],
]

def run_benchmark(name, directory):
    # FIXME

def main():
    for name, directory in BENCHMARKS:
        run_benchmark(name, directory)

if __name__ == "__main__":
    main()
