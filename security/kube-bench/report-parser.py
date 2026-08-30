#!/usr/bin/env python3
"""Render a human-readable summary from a kube-bench JSON report.

Used by the CI compliance scan to produce the GitHub Actions job summary and
the attached artifact. Parsing is tolerant of kube-bench's key casing so the
script keeps working across kube-bench releases.

Usage:
    python3 security/kube-bench/report-parser.py security/kube-bench/reports/kube-bench-report.json
"""

from __future__ import annotations

import json
import sys
from typing import Any


def _find(obj: dict[str, Any], *keys: str) -> Any:
    """Return the first value whose key matches any of the given (case-insensitive) keys."""
    lowered = {str(k).lower(): v for k, v in obj.items()}
    for key in keys:
        if key.lower() in lowered:
            return lowered[key.lower()]
    return None


def _status_label(status: str) -> str:
    return {
        "PASS": "PASS",
        "FAIL": "FAIL",
        "WARN": "WARN",
        "INFO": "INFO",
    }.get(str(status).upper(), str(status).upper())


def summarize(report: dict[str, Any]) -> str:
    totals = _find(report, "Totals") or {}
    control_map = _find(report, "Controls") or []

    lines: list[str] = []
    lines.append("## kube-bench compliance report (stellar-bench)")

    if not isinstance(control_map, list) or not control_map:
        lines.append("")
        lines.append("_No structured controls found; raw report below._")
        lines.append("")
        lines.append("```json")
        lines.append(json.dumps(report, indent=2))
        lines.append("```")
        return "\n".join(lines)

    status_totals: dict[str, int] = {}
    for control in control_map:
        group_map = _find(control, "Groups") or []
        if not isinstance(group_map, list):
            continue
        for group in group_map:
            lines.append("")
            lines.append(f"### {_find(group, 'id')} {_find(group, 'text')}")
            check_map = _find(group, "Checks") or []
            for check in check_map or []:
                status = str(_find(check, "Status") or "INFO").upper()
                status_totals[status] = status_totals.get(status, 0) + 1
                marks = {
                    "PASS": ":white_check_mark:",
                    "FAIL": ":x:",
                    "WARN": ":warning:",
                    "INFO": ":information_source:",
                }
                icon = marks.get(status, ":white_small_square:")
                title = _find(check, "Desc") or _find(check, "text") or _find(check, "id")
                lines.append(f"- {icon} **{title}** `{_status_label(status)}`")
                actual = _find(check, "Actual_Value") or _find(check, "actual_value")
                reason = _find(check, "Reason") or _find(check, "reason")
                if reason:
                    lines.append(f"  - Reason: {reason}")
                elif actual:
                    lines.append(f"  - Observed: `{actual}`")

    lines.append("")
    lines.append("| Status | Count |")
    lines.append("| --- | --- |")
    for status in ("PASS", "FAIL", "WARN", "INFO"):
        lines.append(f"| {status} | {status_totals.get(status, 0)} |")
    if totals:
        lines.append("")
        lines.append(f"kube-bench totals: {totals}")
    lines.append("")
    lines.append("_This scan is non-blocking; review the attached "
                 "`kube-bench-report.json` artifact for details._")
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    if len(argv) < 1:
        print("usage: report-parser.py <kube-bench-report.json>", file=sys.stderr)
        return 1
    try:
        with open(argv[0], encoding="utf-8") as fh:
            report = json.load(fh)
    except (OSError, json.JSONDecodeError) as exc:
        print(f"error: could not read report {argv[0]}: {exc}", file=sys.stderr)
        return 1
    print(summarize(report))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))