#!/usr/bin/env python3
"""Run deduplication and post-dedup evaluation in one command.

This script orchestrates:
1) scripts/deduplicate.py
2) evaluation_scripts/compute_classes_and_metrics.sh
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))

from pathlib import Path
from typing import Optional


SWEEP_DIR_PATTERN = re.compile(r"deduplication_\d{8}_\d{6}$")


def run_cmd(cmd: list[str]) -> None:
    print("[run]", " ".join(cmd), flush=True)
    subprocess.run(cmd, check=True)


def resolve_cexs_dir(config: dict, explicit_cexs_dir: Optional[str]) -> str:
    if explicit_cexs_dir:
        cexs_dir = Path(explicit_cexs_dir)
        if not cexs_dir.is_dir():
            raise FileNotFoundError(f"CEX directory does not exist: {cexs_dir}")
        return str(cexs_dir)

    output_root = Path(config["output_dir"]) / config["core_name"] / "deduplication"
    if not output_root.exists():
        raise FileNotFoundError(
            f"Cannot auto-detect CEX directory because dedup root does not exist: {output_root}. "
            "Pass --cexs-dir explicitly."
        )

    possible_cexs_dirs: list[Path] = []
    for entry in output_root.iterdir():
        potential = entry / "cexs"
        if potential.is_dir():
            possible_cexs_dirs.append(potential)

    if not possible_cexs_dirs:
        raise FileNotFoundError(
            f"Could not automatically determine CEX directory under {output_root}. "
            "Pass --cexs-dir explicitly."
        )

    if len(possible_cexs_dirs) == 1:
        return str(possible_cexs_dirs[0])

    newest = max(possible_cexs_dirs, key=lambda p: p.stat().st_mtime)
    return str(newest)


def resolve_sweep_dir(config: dict, continue_from: Optional[str]) -> str:
    if continue_from:
        sweep_dir = Path(continue_from)
        if not sweep_dir.is_dir():
            raise FileNotFoundError(f"--continue-from path does not exist: {sweep_dir}")
        return str(sweep_dir)

    dedup_root = Path(config["output_dir"]) / config["core_name"] / "deduplication"
    if not dedup_root.is_dir():
        raise FileNotFoundError(
            f"Expected deduplication root does not exist: {dedup_root}."
        )

    candidates = [
        p
        for p in dedup_root.iterdir()
        if p.is_dir() and SWEEP_DIR_PATTERN.match(p.name)
    ]
    if not candidates:
        raise FileNotFoundError(
            f"Could not find deduplication sweep directories in: {dedup_root}"
        )

    newest = max(candidates, key=lambda p: p.stat().st_mtime)
    return str(newest)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run deduplication and class/metrics computation in one command."
    )
    parser.add_argument("--config", required=True, help="Path to dedup config JSON")
    parser.add_argument(
        "--cexs-dir",
        default=None,
        help="Optional override for input CEX directory. If omitted, auto-detected from config.",
    )
    parser.add_argument(
        "--continue-from",
        default=None,
        help="Optional dedup sweep directory to continue from.",
    )
    parser.add_argument(
        "--class-file",
        default=None,
        help="Path for class output JSON. Default: <sweep_dir>/ground_truth.json",
    )
    parser.add_argument(
        "--python",
        default="python3",
        help="Python executable used to run scripts (default: python3).",
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    dedup_script = repo_root / "orchestration" / "deduplicate.py"
    metrics_script = repo_root / "orchestration" / "compute_classes_and_metrics.sh"

    with open(args.config, "r", encoding="utf-8") as f:
        config = json.load(f)

    cexs_dir = resolve_cexs_dir(config, args.cexs_dir)

    dedup_cmd = [args.python, str(dedup_script), "--config", args.config, "--cexs-dir", cexs_dir]
    if args.continue_from:
        dedup_cmd.extend(["--continue-from", args.continue_from])

    run_cmd(dedup_cmd)

    sweep_dir = resolve_sweep_dir(config, args.continue_from)
    class_file = args.class_file or str(Path(sweep_dir) / "ground_truth.json")

    metrics_cmd = ["bash", str(metrics_script), cexs_dir, sweep_dir, class_file]
    run_cmd(metrics_cmd)

    print("[ok] Dedup + metrics completed.", flush=True)
    print(f"[out] cexs_dir: {cexs_dir}", flush=True)
    print(f"[out] sweep_dir: {sweep_dir}", flush=True)
    print(f"[out] class_file: {class_file}", flush=True)
    print(
        "[out] metrics_json: "
        f"{Path(sweep_dir) / 'generalization_metrics_summary.json'}",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as e:
        print(f"[error] Command failed with exit code {e.returncode}", file=sys.stderr)
        raise
