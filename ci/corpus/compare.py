#!/usr/bin/env python3
"""Compares a RuboCop JSON report against an elysium JSON report.

Both `bundle exec rubocop --format json` and `elysium check -f json` emit the
same schema (elysium's JSON formatter matches RuboCop's): a top-level
`summary` with `inspected_file_count`/`offense_count`, and `files[].offenses[]`
entries carrying `cop_name` and a `location` with `start_line`/`start_column`.

`cargo xtask conformance` (crates/xtask/src/conformance_rule.rs) does the
equivalent comparison for one cop at a time, keyed by (path, line, column)
since it already knows which single cop it's checking. This script compares
every cop in a `--only <rules49>` run in one pass, so `cop_name` joins the
key: (path, line, column, cop_name).

Usage: compare.py <rubocop.json> <elysium.json>

Exit 0 when both reports inspected the same number of files and agree on
every offense. Exit 1 when they differ (missing/extra offenses, or a
files-inspected mismatch) -- this is the "diff" the corpus CI job gates on.
Exit 2 on a usage or read error.
"""
import json
import sys


def load(path):
    with open(path) as f:
        return json.load(f)


def index(report):
    """Offenses keyed by (relative path, line, column, cop_name) -> message."""
    out = {}
    for file in report["files"]:
        path = file["path"]
        if path.startswith("./"):
            path = path[2:]
        for offense in file["offenses"]:
            loc = offense["location"]
            key = (path, loc["start_line"], loc["start_column"], offense["cop_name"])
            out[key] = offense["message"]
    return out


def print_samples(label, keys, index_by_key):
    if not keys:
        return
    keys = sorted(keys)
    print(f"\n{label}: {len(keys)} (showing up to 20)")
    for path, line, col, cop in keys[:20]:
        message = index_by_key[(path, line, col, cop)]
        print(f"  {path}:{line}:{col}  {cop}  {message!r}")


def main(argv):
    if len(argv) != 3:
        print(f"usage: {argv[0]} <rubocop.json> <elysium.json>", file=sys.stderr)
        return 2

    try:
        rubocop = load(argv[1])
        elysium = load(argv[2])
    except (OSError, json.JSONDecodeError) as err:
        print(f"error: {err}", file=sys.stderr)
        return 2

    rc_files = rubocop.get("summary", {}).get("inspected_file_count")
    el_files = elysium.get("summary", {}).get("inspected_file_count")

    rc_idx = index(rubocop)
    el_idx = index(elysium)
    rc_keys = set(rc_idx)
    el_keys = set(el_idx)

    matched = rc_keys & el_keys
    missing = rc_keys - el_keys  # rubocop reported, elysium didn't
    extra = el_keys - rc_keys  # elysium reported, rubocop didn't

    ok = True
    print(f"files inspected: rubocop={rc_files} elysium={el_files}")
    if rc_files != el_files:
        ok = False
        print("MISMATCH: files-inspected counts differ")

    print(
        f"offenses: rubocop={len(rc_keys)} elysium={len(el_keys)} "
        f"matched={len(matched)} missing={len(missing)} extra={len(extra)}"
    )

    if missing:
        ok = False
        print_samples("missing (rubocop reported, elysium did not)", missing, rc_idx)
    if extra:
        ok = False
        print_samples("extra (elysium reported, rubocop did not)", extra, el_idx)

    print()
    if ok:
        print("OK: reports match exactly")
        return 0
    print("FAIL: reports differ")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
