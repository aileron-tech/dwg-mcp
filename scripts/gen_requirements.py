"""Regenerate docs/requirements.md from the REQ-MCP tags on the tests.

Run from the repository root: python3 scripts/gen_requirements.py
"""
import pathlib
import re

BLOCKS = [
    ("Server and tool definitions", 1, 29),
    ("parse_file", 30, 59),
    ("Result cache and reset_file_cache", 60, 89),
    ("get_result", 90, 129),
    ("get_job, list_jobs, delete_job", 130, 149),
    ("parse_url and reset_url_cache", 150, 199),
]
TAG = re.compile(r"\s*/\* (REQ-MCP-(U?)(\d+)): (.*) \*/")
FN = re.compile(r"\s*(?:async )?fn (\w+)")


def collect():
    rows = []
    files = sorted(pathlib.Path("tests").glob("mcp_*.rs")) + sorted(pathlib.Path("src").glob("*.rs"))
    for path in files:
        lines = path.read_text().splitlines()
        for i, line in enumerate(lines):
            tag = TAG.match(line)
            if not tag:
                continue
            name = next(FN.match(l).group(1) for l in lines[i + 1 : i + 4] if FN.match(l))
            rows.append((tag.group(1), tag.group(2) == "U", int(tag.group(3)), tag.group(4), f"{path.as_posix()}::{name}"))
    return rows


def table(rows):
    out = ["| Id | Requirement | Test |", "|----|-------------|------|"]
    out += [f"| {r[0]} | {r[3]} | `{r[4]}` |" for r in sorted(rows, key=lambda r: r[2])]
    return out + [""]


def main():
    rows = collect()
    ids = [r[0] for r in rows]
    assert len(ids) == len(set(ids)), "duplicate requirement id"
    out = [
        "# Requirement register: dwg-mcp",
        "",
        "Every test carries the id of the requirement it checks, as a",
        "`/* REQ-MCP-<id>: <description> */` comment. This file is generated from those",
        "comments by `scripts/gen_requirements.py`. Allocate the next free number in the",
        "block of the tool the requirement belongs to.",
        "",
        "| Block | Ids |",
        "|-------|-----|",
    ]
    out += [f"| {title} | REQ-MCP-{lo:03} to {hi:03} |" for title, lo, hi in BLOCKS]
    out += ["| Pure rules (unit tests) | REQ-MCP-U01 onwards |", ""]
    for title, lo, hi in BLOCKS:
        out += [f"## {title}", ""] + table([r for r in rows if not r[1] and lo <= r[2] <= hi])
    out += ["## Pure rules (unit tests)", ""] + table([r for r in rows if r[1]])
    pathlib.Path("docs/requirements.md").write_text("\n".join(out))
    print(f"{len(rows)} requirements: {sum(not r[1] for r in rows)} end-to-end, {sum(r[1] for r in rows)} unit")


main()
