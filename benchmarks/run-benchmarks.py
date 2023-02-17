#!/usr/bin/env python3

import os
import tempfile
import requests
from subprocess import check_call, check_output, CalledProcessError
from typing import TextIO


# Benchmark name, Directory with SVGs to render
BENCHMARKS = [
    ["hicolor-apps", "./hicolor-apps"],
    ["symbolic-icons", "../tests/fixtures/reftests/adwaita"],
]
METRICS_URL = "https://librsvg-metrics.fly.dev/api/metrics/"
PATH_TO_RSVG_BENCH = "../target/release/rsvg-bench"


def parse_output_file(file: TextIO):
    """ parse the cachegrind output file for metrics"""
    keys, values = None, None
    for line in file.readlines():
        line = line.strip()
        if line.startswith("events: "):
            keys = line.removeprefix("events: ").split(" ")
        if line.startswith("summary: "):
            values = line.removeprefix("summary: ").split(" ")

    if keys is None or values is None:
        raise Exception("Couldn't parse cachegrind file, event names or summary metrics not found")

    return {k: v for k, v in zip(keys, values)}


def get_commit_details():
    """ Get commit details on which benchmarking is run """
    if os.environ.get("CI_COMMIT_SHA") and os.environ.get("CI_COMMIT_TIMESTAMP"):
        return {
            "commit": os.environ["CI_COMMIT_SHA"],
            "time": os.environ["CI_COMMIT_TIMESTAMP"]
        }

    commit_hash = check_output(["git", "show", "--format=%cI"]).strip()
    commit_time = check_output(["git", "show", "--format=%H"]).strip()
    return {
        "commit": str(commit_hash),
        "time": str(commit_time)
    }


def submit_metrics(data):
    token = os.environ["METRICS_TOKEN"]
    response = requests.post(METRICS_URL, json=data, headers={"Authorization": f"Token {token}"})
    response.raise_for_status()


def run_with_cachegrind(directory, path):
    command = ["valgrind", "--tool=cachegrind", f"--cachegrind-out-file={path}", PATH_TO_RSVG_BENCH, directory]
    check_call(command)


def check_working_tree():
    cmd = ["git", "diff-index", "--quiet", "HEAD"]
    try:
        check_call(cmd)
    except CalledProcessError as e:
        print("git working tree not clean, exiting.")
        raise e


def run_benchmark(name, directory):
    with tempfile.NamedTemporaryFile("r+") as file:
        run_with_cachegrind(directory, file.name)

        metrics = parse_output_file(file)
        metrics["value"] = metrics["Ir"]
        metrics["name"] = name

        metadata = get_commit_details()
        data = metadata | metrics
        submit_metrics(data)


def main():
    check_working_tree()
    for name, directory in BENCHMARKS:
        run_benchmark(name, directory)


if __name__ == "__main__":
    main()
