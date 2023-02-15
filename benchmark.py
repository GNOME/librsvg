#!/usr/bin/env python3

import os
import sys
import tempfile
import requests
from subprocess import check_call, check_output, CalledProcessError
from typing import TextIO

METRICS_URL = "https://librsvg-metrics.fly.dev/api/metrics/"


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
    print(response.status_code, response.reason)


def run_benchmark(cmd, path):
    command = ["valgrind", "--tool=cachegrind", f"--cachegrind-out-file={path}", *cmd]
    check_call(command)


def check_working_tree():
    cmd = ["git", "diff-index", "--quiet", "HEAD"]
    try:
        check_call(cmd)
    except CalledProcessError as e:
        print("git working tree not clean, exiting.")
        raise e


def main():
    check_working_tree()
    with tempfile.NamedTemporaryFile("r+") as file:
        run_benchmark(sys.argv[1:], file.name)

        metrics = parse_output_file(file)
        metrics["value"] = metrics["Ir"]

        metadata = get_commit_details()
        data = metadata | metrics
        submit_metrics(data)


if __name__ == "__main__":
    main()
