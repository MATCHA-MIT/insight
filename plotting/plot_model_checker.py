#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Adapted:
- Plot a single cumulative curve for discovered issues, flatlined to end.
- Average query time counts ONLY successful (non-timeout) queries with the sequence:
  jg_query_started -> jg_query_completed -> running_insight_on_cex
  (duration measured from started to completed; sequence must occur before next start).
- One LaTeX table combining bugs and underspecifications into discovered issues.
- Add total runtime (min) to the table.
- Update: LaTeX table now includes:
  * Total time JG (only finished queries)
  * Total time Insight = (mutation_start(ed)?→mutation_completed) + (separator_started→separator_completed)
"""

import pandas as pd
import matplotlib.pyplot as plt
import matplotlib as mpl
import sys
from pathlib import Path

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))

import re
import os
import json
import glob
import itertools
import math
from textwrap import shorten

# ------------------- Matplotlib pseudo-LaTeX configuration -------------------
mpl.rcParams.update({
    "text.usetex": False,
    "font.family": "serif",
    "font.serif": ["Times New Roman", "Times", "DejaVu Serif"],
    "axes.labelsize": 13,
    "axes.titlesize": 13,
    "xtick.labelsize": 11,
    "ytick.labelsize": 11,
    "legend.fontsize": 10,
    "lines.linewidth": 1.5,
    "lines.markersize": 6,
    "figure.figsize": (6.5, 3.6),
    "figure.dpi": 300,
    "savefig.dpi": 600,
    "savefig.format": "pdf",
    "savefig.bbox": "tight",
})



# ------------------- Helpers -------------------

OUTPUT_DIR = "result_plots"
os.makedirs(OUTPUT_DIR, exist_ok=True)

def fmt_percent(x: float) -> str:
    if pd.isna(x) or math.isinf(x):
        return "--"
    return f"{x:.1f}\\%"

def fmt_minutes_with_pct(value_min: float, total_runtime_min: float) -> str:
    if pd.isna(value_min) or pd.isna(total_runtime_min) or total_runtime_min <= 0:
        return "--"
    pct = (value_min / total_runtime_min) * 100.0
    return f"{value_min:.2f} ({pct:.1f}\\%)"


def normalize_time(df: pd.DataFrame) -> pd.DataFrame:
    """Normalize event times continuously relative to genin_start in minutes."""
    if df.empty:
        df["NormTime"] = []
        return df
    normalized_times = []
    last_genin_time = None
    time_offset = 0.0
    last_event_time = None

    for _, row in df.iterrows():
        current_time = row["Time"]
        ev = row["Event"] if isinstance(row["Event"], str) else ""

        if ev.startswith("genin_start"):
            if last_event_time is not None:
                time_offset = normalized_times[-1]
            last_genin_time = current_time
            norm_time = time_offset
        else:
            if last_genin_time is not None:
                norm_time = ((current_time - last_genin_time) / 60.0) + time_offset
            else:
                norm_time = current_time / 60.0
        if norm_time < 0:
            raise ValueError(f"Negative normalized time computed for event '{ev}' at raw time {current_time}s.")
        normalized_times.append(norm_time)
        last_event_time = current_time

    df["NormTime"] = normalized_times
    return df


def extract_weights(path: str):
    """Extract bex_weight and predicate_weight from file path using regex."""
    if "no_insight" in path:
        return None, None
    nums = re.findall(r'\d+', path)
    if len(nums) >= 2:
        return int(nums[0]), int(nums[1])
    elif len(nums) == 1:
        return int(nums[0]), None
    else:
        return None, None


def cumulative_event(df: pd.DataFrame, mask_col: str) -> pd.DataFrame:
    """Compute cumulative count of events based on a column mask (starting from 0)."""
    if mask_col not in df.columns:
        return pd.DataFrame(columns=["NormTime", "Count"])
    events = df[df[mask_col].notna()][["NormTime"]].copy()
    if events.empty:
        return events
    events["Count"] = range(1, len(events) + 1)
    start_row = pd.DataFrame({"NormTime": [0.0], "Count": [0]})
    events = pd.concat([start_row, events], ignore_index=True)
    return events


def extend_or_flatline(events_df: pd.DataFrame, end_time: float) -> pd.DataFrame:
    """Flat line to end_time if no events; otherwise extend last count to end_time."""
    if end_time is None:
        end_time = 0.0
    if events_df is None or events_df.empty:
        return pd.DataFrame({"NormTime": [0.0, float(end_time)],
                             "Count":    [0,   0]})
    out = events_df.copy()
    last_cnt = int(out["Count"].iloc[-1])
    if out["NormTime"].iloc[-1] < float(end_time):
        out = pd.concat(
            [out, pd.DataFrame({"NormTime": [float(end_time)], "Count": [last_cnt]})],
            ignore_index=True
        )
    return out


def avg_interarrival_minutes(times) -> float:
    """Mean inter-arrival time (minutes) given a sorted list/Series of times (minutes)."""
    if len(times) <= 1:
        return float('nan')
    diffs = [t2 - t1 for t1, t2 in zip(times[:-1], times[1:])]
    return sum(diffs) / len(diffs)


def compute_avg_insight_run_minutes(df: pd.DataFrame) -> float:
    """
    Average Insight run duration (minutes).
    A run starts at each 'separator_started' (raw Time, seconds) and ends at the next 'separator_completed'
    or at the file's last Time.
    """
    if "Time" not in df.columns or "Event" not in df.columns or df.empty:
        return float('nan')
    df_sorted = df.sort_values("Time")
    starts = df_sorted[df_sorted["Event"].astype(str).str.startswith("separator_started")]["Time"].tolist()
    if not starts:
        return float('nan')
    end_time = float(df_sorted["Time"].max())
    ends = starts[1:] + [end_time]
    runs = [(e - s) / 60.0 for s, e in zip(starts, ends) if e >= s]
    if not runs:
        return float('nan')
    return sum(runs) / len(runs)


def latex_escape(s: str) -> str:
    """Minimal LaTeX escaping for table cells."""
    if s is None:
        return ""
    s = s.replace("\\", "\\textbackslash{}")
    replacements = {
        "&": "\\&", "%": "\\%", "$": "\\$", "#": "\\#", "_": "\\_",
        "{": "\\{", "}": "\\}", "~": "\\textasciitilde{}",
        "^": "\\textasciicircum{}",
    }
    for k, v in replacements.items():
        s = s.replace(k, v)
    return s


def list_to_latex_cell(values, max_items=20, max_chars=200) -> str:
    """Join unique values into a LaTeX-safe cell; truncate politely if too long (one-column friendly)."""
    if not values:
        return "--"
    uniq, seen = [], set()
    for v in values:
        if v is None:
            continue
        v = str(v).strip()
        if not v:
            continue
        if v not in seen:
            seen.add(v)
            uniq.append(v)
    if not uniq:
        return "--"
    joined = "; ".join(uniq[:max_items])
    if len(uniq) > max_items:
        joined += "; …"
    joined = shorten(joined, width=max_chars, placeholder=" …")
    return latex_escape(joined)


def fmt_minutes(x: float) -> str:
    if pd.isna(x) or math.isinf(x):
        return "--"
    return f"{x:.2f}"


def fmt_ratio(x: float) -> str:
    if pd.isna(x) or math.isinf(x):
        return "--"
    return f"{x:.2f}"


def count_predicates_in_invariant_json(inv_data: dict) -> int | None:
    """
    Return predicate count for one invariant JSON.
    Counts predicates across all disjuncts in separator_formula.InvariantDisjunction.disjunctions.
    Returns None if the expected structure is not present.
    """
    try:
        disjunctions = inv_data["separator_formula"]["InvariantDisjunction"]["disjunctions"]
    except Exception:
        return None

    if not isinstance(disjunctions, list):
        return None

    total = 0
    for disj in disjunctions:
        preds = (((disj or {}).get("predicate_set") or {}).get("predicates"))
        if isinstance(preds, list):
            total += len(preds)
    return total


def compute_predicate_stats_for_run(event_log_path: str) -> tuple[float, float, int]:
    """
    Compute (mean, stddev, count) of predicates per invariant for one run.
    Expects invariant JSON files under <run_dir>/invariants.
    """
    run_dir = os.path.dirname(os.path.abspath(event_log_path))
    invariants_dir = os.path.join(run_dir, "invariants")
    if not os.path.isdir(invariants_dir):
        return float("nan"), float("nan"), 0

    counts = []
    pattern = os.path.join(invariants_dir, "*.json")
    for inv_path in sorted(glob.glob(pattern)):
        try:
            with open(inv_path, "r", encoding="utf-8") as fh:
                data = json.load(fh)
        except Exception:
            continue

        pred_count = count_predicates_in_invariant_json(data)
        if pred_count is not None:
            counts.append(float(pred_count))

    if not counts:
        return float("nan"), float("nan"), 0

    mean_val = sum(counts) / len(counts)
    if len(counts) == 1:
        std_val = 0.0
    else:
        var = sum((x - mean_val) ** 2 for x in counts) / (len(counts) - 1)
        std_val = math.sqrt(var)
    return mean_val, std_val, len(counts)


def safe_json_loads(value):
    if value is None or (isinstance(value, float) and pd.isna(value)):
        return None
    if isinstance(value, (dict, list)):
        return value
    text = str(value).strip()
    if not text:
        return None
    try:
        return json.loads(text)
    except Exception:
        return None


def get_underspecification_col(df: pd.DataFrame) -> str | None:
    if "Underspecification" in df.columns:
        return "Underspecification"
    if "Mismatch" in df.columns:
        # Backward compatibility with older logs.
        return "Mismatch"
    return None


def extract_instruction_lines(details_value) -> list[str]:
    parsed = safe_json_loads(details_value)
    if isinstance(parsed, list):
        return [str(x) for x in parsed]
    if isinstance(parsed, str):
        return [parsed]
    return []


def extract_mnemonic(instr_line: str) -> str:
    m = re.search(r"^\s*\d+:\s+[0-9a-fA-F]+\s+([a-zA-Z.]+)", instr_line)
    if m:
        return m.group(1).lower()
    parts = instr_line.strip().split()
    if len(parts) >= 3:
        return parts[2].lower()
    if parts:
        return parts[-1].lower()
    return ""


def extract_csr_info(instr_line: str):
    """Extract mnemonic, CSR name, and read/write flags from an instruction line."""
    m = re.search(r"^\s*\d+:\s+[0-9a-fA-F]+\s+([a-zA-Z.]+)\s+(.*)$", instr_line)
    if not m:
        m_simple = re.search(r"^\s*([a-zA-Z.]+)\s+(.*)$", instr_line)
        if not m_simple:
             # Try just mnemonic
             mnemonic = extract_mnemonic(instr_line)
             return mnemonic, None, False, False
        mnemonic = m_simple.group(1).lower()
        ops_str = m_simple.group(2)
    else:
        mnemonic = m.group(1).lower()
        ops_str = m.group(2)

    operands = [o.strip() for o in ops_str.split(",")]

    csr_map = {
        "rdcycle": "cycle", "rdcycleh": "cycleh",
        "rdinstret": "instret", "rdinstreth": "instreth",
        "rdtime": "time", "rdtimeh": "timeh"
    }

    csr = None
    if mnemonic in csr_map:
        csr = csr_map[mnemonic]
    elif mnemonic in {"csrrw", "csrrs", "csrrc", "csrrwi", "csrrsi", "csrrci", "csrr"}:
        if len(operands) >= 2:
            csr = operands[1]
    elif mnemonic in {"csrw", "csrs", "csrc", "csrwi", "csrsi", "csrci"}:
        if len(operands) >= 1:
            csr = operands[0]

    is_write = mnemonic in {"csrrw", "csrrs", "csrrc", "csrrwi", "csrrsi", "csrrci", "csrw", "csrs", "csrc", "csrwi", "csrsi", "csrci"}
    is_read = mnemonic in {"csrrw", "csrrs", "csrrc", "csrrwi", "csrrsi", "csrrci", "csrr", "rdcycle", "rdcycleh", "rdinstret", "rdinstreth", "rdtime", "rdtimeh"}

    return mnemonic, csr, is_write, is_read


def classify_all_triggered_cex(instr_lines: list[str]) -> str:
    if not instr_lines:
        return "U?"

    lower_lines = [line.lower() for line in instr_lines]

    # Track CSR info and check for jumps
    csr_info_list = []
    has_jump = False

    for line in instr_lines:
        mnemonic, csr, is_write, is_read = extract_csr_info(line)
        if mnemonic in {"jal", "jalr", "j", "jr"} or mnemonic.startswith("jal"):
            has_jump = True
        if csr or is_write or is_read:
            csr_info_list.append({
                "mnemonic": mnemonic,
                "csr": csr,
                "is_write": is_write,
                "is_read": is_read
            })

    # Rule order:
    # 1. U2 (Jumps)
    if has_jump:
        return "U2"

    # 2. B1 (Minstret mentions >= 2)
    minstret_mentions = sum(line.count("minstret") + line.count("minstreth") for line in lower_lines)
    if minstret_mentions >= 2:
        return "B1"

    # 3. U3 (Same CSR dependency: Write then subsequent Read)
    for i in range(len(csr_info_list)):
        for j in range(i + 1, len(csr_info_list)):
            info_i = csr_info_list[i]
            info_j = csr_info_list[j]
            if info_i["is_write"] and info_j["is_read"]:
                if info_i["csr"] and info_j["csr"] and info_i["csr"] == info_j["csr"]:
                    return "U3"

    # 4. U1 (Any CSR instruction if not U3)
    if csr_info_list:
        return "U1"

    # 5. U4 (WFI)
    if any(re.search(r"\bwfi\b", line) for line in lower_lines):
        return "U4"

    print("Returning U? for ", instr_lines)
    return "U?"


def extract_issue_time_map(df: pd.DataFrame) -> dict[str, float]:
    issue_time_map: dict[str, float] = {}
    if df.empty:
        return issue_time_map

    underspec_col = get_underspecification_col(df)
    fixed_keys = {"ALLFIX", "FIXED", "NOBUG"}
    pending_all_triggered_times: list[float] = []

    d = df.sort_values("NormTime").reset_index(drop=True)
    for _, row in d.iterrows():
        event = str(row.get("Event", ""))
        norm_time = float(row["NormTime"]) if pd.notna(row.get("NormTime")) else 0.0

        if event in {"cex_found_from_jg", "picked_new_cex_from_mutations"}:
            if "Bugs" in d.columns:
                parsed_bugs = safe_json_loads(row.get("Bugs"))
                if isinstance(parsed_bugs, list) and len(parsed_bugs) == 1:
                    issue_time_map.setdefault(str(parsed_bugs[0]), norm_time)
                    continue

            all_triggered = []
            fixed_core_triggered = False

            if underspec_col is not None:
                parsed_under = safe_json_loads(row.get(underspec_col))
                if isinstance(parsed_under, dict):
                    raw = parsed_under.get("all_triggered", [])
                    if isinstance(raw, list):
                        all_triggered = [str(x).strip() for x in raw if str(x).strip()]
                    fixed_core_triggered = bool(parsed_under.get("fixed_core_mismatch", False))
                elif isinstance(parsed_under, list):
                    all_triggered = [str(x).strip() for x in parsed_under if str(x).strip()]
                    fixed_core_triggered = any(
                        t.replace("-", "").replace("_", "").upper() in fixed_keys for t in all_triggered
                    )

            if len(all_triggered) == 1:
                issue_time_map.setdefault(all_triggered[0], norm_time)
                continue

            includes_fixed_key = any(
                t.replace("-", "").replace("_", "").upper() in fixed_keys for t in all_triggered
            )
            if fixed_core_triggered or includes_fixed_key:
                pending_all_triggered_times.append(norm_time)
                continue

        elif event == "running_insight_on_cex" and pending_all_triggered_times:
            t0 = pending_all_triggered_times.pop(0)
            instr_lines = extract_instruction_lines(row.get("Details"))
            issue = classify_all_triggered_cex(instr_lines)
            issue_time_map.setdefault(issue, t0)

    for t0 in pending_all_triggered_times:
        issue_time_map.setdefault("U?", t0)

    return issue_time_map


def _finished_windows(df: pd.DataFrame, start_regex: str, end_regex: str, tag: str):
    """
    Return finished windows as (start_time_sec, duration_min, tag),
    but ONLY when the end event is the *immediate next line* after the start.
    If the next line is not an end (or is another start), ignore the start.
    """
    if df.empty or "Event" not in df.columns or "Time" not in df.columns:
        return []

    d = df.sort_values("Time").reset_index(drop=True)
    ev = d["Event"].astype(str).fillna("")
    t  = d["Time"].astype(float)

    # indices of starts matching start_regex
    start_idxs = ev[ev.str.match(start_regex)].index.tolist()
    if not start_idxs:
        return []

    end_re = re.compile(end_regex)
    out = []

    for i, si in enumerate(start_idxs):
        # the next line after the start
        next_idx = si + 1

        # define the boundary of the current start window (next start or EOF)
        next_start = start_idxs[i+1] if i+1 < len(start_idxs) else len(d)

        # must have a next line, and it must still be within this start→next-start window
        if next_idx >= next_start or next_idx >= len(d):
            continue

        # immediate-next-line must be the proper end
        if not end_re.match(ev.iloc[next_idx]):
            continue

        dur_min = (t.iloc[next_idx] - t.iloc[si]) / 60.0
        if dur_min > 0:
            out.append((t.iloc[si], dur_min, tag))

    return out


def _clip_to_budget(windows, budget_min: float):
    """
    Given a list of (start_sec, duration_min, tag) in any order, clip in chronological order
    so that the sum of durations across ALL tags doesn't exceed budget_min.
    Returns clipped list with possibly shortened last window.
    """
    if not windows or budget_min <= 0:
        return []

    windows_sorted = sorted(windows, key=lambda x: x[0])  # by start_sec
    clipped = []
    used = 0.0
    for start_sec, dur_min, tag in windows_sorted:
        if used >= budget_min:
            print("Reached budget limit; stopping further window inclusion. Last windows", start_sec, dur_min, tag)
            break
        remaining = budget_min - used
        take = dur_min if dur_min <= remaining else remaining
        if take > 0:
            clipped.append((start_sec, take, tag))
            used += take
    return clipped

# ---------- NEW: finished-window totals ----------

def _sum_finished_windows(df: pd.DataFrame, start_regex: str, end_regex: str) -> float:
    """
    Sum durations (minutes) of windows that have a start matching start_regex and a subsequent
    end matching end_regex BEFORE the next start. Unmatched starts are ignored.
    """
    if df.empty or "Event" not in df.columns or "Time" not in df.columns:
        return 0.0

    d = df.sort_values("Time").reset_index(drop=True)
    ev = d["Event"].astype(str).fillna("")
    t = d["Time"].astype(float)

    start_mask = ev.str.match(start_regex)
    start_idxs = start_mask[start_mask].index.tolist()
    if not start_idxs:
        return 0.0

    total_min = 0.0
    end_re = re.compile(end_regex)

    for i, si in enumerate(start_idxs):
        window_end = start_idxs[i+1] if i+1 < len(start_idxs) else len(d)
        window_ev = ev.iloc[si+1:window_end]  # strictly after start
        # find first matching end
        end_candidates = window_ev[window_ev.apply(lambda s: bool(end_re.match(s)))]
        if not len(end_candidates):
            continue
        ei = end_candidates.index[0]
        dur = (t.loc[ei] - t.iloc[si]) / 60.0
        if dur > 0:
            total_min += dur

    return total_min


# def compute_total_jg_finished_minutes(df: pd.DataFrame) -> float:
#     """Total time in JG for finished queries: jg_query_started → jg_query_completed only."""
#     return _sum_finished_windows(
#         df,
#         r"^jg_query_started",
#         r"^jg_query_completed"
#     )
def compute_total_jg_finished_minutes(df: pd.DataFrame) -> float:
    return sum(d for _, d, _ in _finished_windows(df, r"^jg_query_started", r"^jg_query_completed", "JG"))

def compute_total_insight_finished_minutes(df: pd.DataFrame) -> float:
    mut = _finished_windows(df, r"^mutation_start(?:ed)?", r"^mutation_completed", "INSIGHT")
    sep = _finished_windows(df, r"^separator_started", r"^separator_completed", "INSIGHT")
    return sum(d for _, d, _ in (mut + sep))

# def compute_total_insight_finished_minutes(df: pd.DataFrame) -> float:
#     """
#     Total finished Insight time:
#       (mutation_start(ed)? → mutation_completed) + (separator_started → separator_completed)
#     Only windows with a matching completed inside the same start→next-start window are counted.
#     """
#     mut = _sum_finished_windows(
#         df,
#         r"^mutation_start(?:ed)?",
#         r"^mutation_completed"
#     )
#     sep = _sum_finished_windows(
#         df,
#         r"^separator_started",
#         r"^separator_completed"
#     )
#     return mut + sep


def compute_successful_queries_avg_time(df: pd.DataFrame) -> tuple[float, int]:
    """
    Consider only windows with the full sequence:
      jg_query_started -> jg_query_completed -> running_insight_on_cex
    (all before the next jg_query_started). Duration = completed - started (minutes).
    Returns (avg_time_minutes, num_success).
    """
    if df.empty or "Event" not in df.columns or "Time" not in df.columns:
        return float('nan'), 0

    d = df.sort_values("Time").reset_index(drop=True)
    events = d["Event"].astype(str).fillna("")
    times = d["Time"].astype(float)

    start_idxs = events[events.str.contains(r"^jg_query_started", na=False)].index.tolist()
    if not start_idxs:
        return float('nan'), 0

    durations = []

    for i, si in enumerate(start_idxs):
        window_end = start_idxs[i+1] if i+1 < len(start_idxs) else len(d)
        window_events = events.iloc[si:window_end]

        try:
            ci_rel = window_events.str.contains(r"^jg_query_completed", na=False).idxmax()
            if not window_events.loc[ci_rel].startswith("jg_query_completed"):
                raise ValueError
        except Exception:
            continue  # no completed in this window

        after_completed = window_events.loc[ci_rel+1:window_events.index[-1]]
        if (after_completed.str.contains(r"^genin_start", na=False).any()):
            continue

        t_start = times.iloc[si]
        t_comp = times.loc[ci_rel]
        if t_comp >= t_start:
            durations.append((t_comp - t_start) / 60.0)

    if not durations:
        return float('nan'), 0
    return (sum(durations) / len(durations)), len(durations)


# ------------------ MAIN ------------------
if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python script.py file1.csv file2.csv ...")
        sys.exit(1)
    
    files = sys.argv[1:]
    
    # Distinct colors per file (good contrast for print)
    base_colors = [
        "#1b9e77", "#d95f02", "#7570b3", "#e7298a",
        "#66a61e", "#e6ab02", "#a6761d", "#666666"
    ]
    color_cycle = itertools.cycle(base_colors)
    
    # --- Plot 1: Combined Issues ---
    plt.figure()
    
    summary_rows = []  # collect per-file stats for LaTeX table
    
    issue_records = []  # rows for issue_detection_times.csv across all runs
    for f, color in zip(files, color_cycle):
        df = pd.read_csv(f, sep=';')
        df = normalize_time(df)
    
        bex_weight, predicate_weight = extract_weights(f)
        label_suffix = (f"($\\lambda$={bex_weight}%, $\\mu$=0.{predicate_weight}%)"
                        if predicate_weight is not None else
                        "Baseline (BOOM)")
        end_time = float(df["NormTime"].max()) if "NormTime" in df and len(df) else 0.0
    
        # ---- Successful queries only (non-timeout) ----
        avg_q_time, num_success_q = compute_successful_queries_avg_time(df)
    
        # ---- Avg Insight run ----
        avg_run = compute_avg_insight_run_minutes(df)
    
        # ---- Total runtime (minutes) ----
        if "Time" in df.columns and len(df):
            total_runtime_min = (float(df["Time"].max()) - float(df["Time"].min())) / 60.0
        else:
            total_runtime_min = float('nan')
    
        # ---- Finished totals per your request ----
        # total_jg_finished_min = compute_total_jg_finished_minutes(df)
        # total_insight_finished_min = compute_total_insight_finished_minutes(df)
        # ---- Determine cutoff/budget (minutes) based on config) ----
        # If we parsed a predicate_weight, treat it as a non-baseline (12h); otherwise baseline (24h).
        budget_min = 720.0 if predicate_weight is not None else 1440.0  # 12h vs 24h
    
        # ---- Collect finished windows (JG + Insight) and clip to budget on ACTIVE time ----
        jg_windows  = _finished_windows(df, r"^jg_query_started", r"^jg_query_completed", "JG")
        mut_windows = _finished_windows(df, r"^mutation_start(?:ed)?", r"^mutation_completed", "INSIGHT")
        sep_windows = _finished_windows(df, r"^separator_started", r"^separator_completed", "INSIGHT")
    
        all_windows = jg_windows + mut_windows + sep_windows
        clipped = _clip_to_budget(all_windows, budget_min)
    
        # ---- Compute clipped totals per category and overall active runtime ----
        total_runtime_min = sum(d for _, d, _ in clipped)  # active processing time within budget
        total_jg_finished_min = sum(d for _, d, tag in clipped if tag == "JG")
        total_insight_finished_min = sum(d for _, d, tag in clipped if tag == "INSIGHT")
    
        # ---- Invariant predicate complexity stats ----
        avg_predicates_per_invariant, std_predicates_per_invariant, num_invariants = compute_predicate_stats_for_run(f)
    
    
        # ---- Discovered issues with first-detection times ----
        issue_time_map = extract_issue_time_map(df)
    
        # Emit per-issue rows for the CSV (sorted by time)
        for txt, tmin in sorted(issue_time_map.items(), key=lambda kv: kv[1]):
            issue_records.append({
                "Config": label_suffix,
                "Issue": txt,
                "DetectedAtMin": round(tmin, 4)
            })
    
        # Build LaTeX cell: "Issue (12.34m); Issue2 (45.00m) ..."
        issues_with_times = [f"{txt} ({issue_time_map[txt]:.2f}m)" for txt in sorted(issue_time_map, key=lambda k: issue_time_map[k])]
        issues_cell = list_to_latex_cell(issues_with_times)
    
        # Count for queries-per-issue
        num_issues_found = len(issue_time_map)
        queries_per_issue = (num_success_q / num_issues_found) if num_issues_found > 0 else float('nan')
    
        summary_rows.append({
            "Config": label_suffix,
            "AvgTimePerQueryMin": avg_q_time,
            "NumSuccessQueries": num_success_q,
            "QueriesPerIssue": queries_per_issue,
            "AvgInsightRunMin": avg_run,
            "TotalRuntimeMin": total_runtime_min,
            "TotalJGFinishedMin": total_jg_finished_min,
            "TotalInsightFinishedMin": total_insight_finished_min,
            "AvgPredicatesPerInvariant": avg_predicates_per_invariant,
            "StdPredicatesPerInvariant": std_predicates_per_invariant,
            "NumInvariants": num_invariants,
            "IssuesText": issues_cell,
            "NumIssues": num_issues_found,
        })
        print(df)
    
        # ---- Plotting: cumulative discovered issues from the new CEX classification ----
        issue_event_times = sorted(issue_time_map.values())
        if issue_event_times:
            combined_events_only = pd.DataFrame({
                "NormTime": [0.0] + issue_event_times,
                "Count": [0] + list(range(1, len(issue_event_times) + 1)),
            })
        else:
            combined_events_only = pd.DataFrame(columns=["NormTime", "Count"])
        # Build the line series extended to end_time (adds synthetic last point if needed)
        combined_line = extend_or_flatline(combined_events_only, end_time)
    
        # Draw the line (includes the synthetic flat tail)
        plt.plot(combined_line["NormTime"], combined_line["Count"],
                 linestyle='-', color=color, alpha=0.9)
    
        # Draw dots ONLY at real events (no dot at the synthetic end point)
        scatter_df = combined_events_only[combined_events_only["Count"] > 0] if not combined_events_only.empty else combined_events_only
        if not scatter_df.empty:
            plt.scatter(scatter_df["NormTime"], scatter_df["Count"],
                        color=color, marker='o', s=28, label=f"Configuration {label_suffix}")
        else:
            plt.scatter([], [], color=color, marker='o', s=28, label=f"Configuration {label_suffix}")
    
    plt.xlabel("Time (min)")
    plt.ylabel("Cumulative Issues")
    plt.title("Cumulative Issues Found Over Time (BOOM)", pad=10)
    plt.legend(frameon=False, ncol=1)
    plt.grid(True, linestyle=':', linewidth=0.5, alpha=0.8)
    plt.tight_layout(pad=0.5)
    plt.savefig(os.path.join(OUTPUT_DIR, "issues_vs_time.pdf"))
    plt.close()
    print("✅ Saved: issues_vs_time.pdf")
    
    # --- Save issue detection times (across all runs) ---
    if issue_records:
        pd.DataFrame(issue_records).sort_values(["Config", "DetectedAtMin"]).to_csv(os.path.join(OUTPUT_DIR, "issue_detection_times.csv"), index=False)
        print("✅ Saved: issue_detection_times.csv")
    else:
        print("ℹ️ No issues found to save (issue_detection_times.csv not created).")
    
    # --- Plot 2: jg_query_started (unchanged) ---
    plt.figure()
    color_cycle = itertools.cycle(base_colors)
    
    for f, color in zip(files, color_cycle):
        df = pd.read_csv(f, sep=';')
        df = normalize_time(df)
        bex_weight, predicate_weight = extract_weights(os.path.basename(f))
        label_prefix = (f"($\\lambda$={bex_weight}, $\\mu$={predicate_weight})"
                        if predicate_weight is not None else
                        os.path.basename(f))
        end_time = float(df["NormTime"].max()) if "NormTime" in df and len(df) else 0.0
    
        jg = df[df["Event"].str.contains("jg_query_started", na=False)][["NormTime"]].copy()
        if jg.empty:
            jg_plot = pd.DataFrame({"NormTime": [0.0, end_time], "Count": [0, 0]})
        else:
            jg["Count"] = range(1, len(jg) + 1)
            start_row = pd.DataFrame({"NormTime": [0.0], "Count": [0]})
            jg_plot = pd.concat([start_row, jg], ignore_index=True)
            if jg_plot["NormTime"].iloc[-1] < end_time:
                jg_plot = pd.concat(
                    [jg_plot, pd.DataFrame({"NormTime": [end_time], "Count": [jg_plot["Count"].iloc[-1]]})],
                    ignore_index=True
                )
    
        plt.plot(jg_plot["NormTime"], jg_plot["Count"],
                 linestyle='-', color=color, alpha=0.9)
        if jg_plot["Count"].max() > 0:
            plt.scatter(jg_plot["NormTime"], jg_plot["Count"],
                        color=color, marker='o', s=28, label=label_prefix)
        else:
            plt.scatter([], [], color=color, marker='o', s=28, label=label_prefix)
    
    plt.xlabel("Time (min)")
    plt.ylabel("Cumulative Queries")
    plt.title("Cumulative jg_query_started Events", pad=10)
    plt.legend(frameon=False, ncol=1)
    plt.grid(True, linestyle=':', linewidth=0.5, alpha=0.8)
    plt.tight_layout(pad=0.5)
    plt.savefig(os.path.join(OUTPUT_DIR, "jg_query_started_vs_time.pdf"))
    plt.close()
    print("✅ Saved: jg_query_started_vs_time.pdf")
    
    # --- LaTeX summary table (combined) ---
    summary_df = pd.DataFrame(summary_rows)
    
    rows_tex = []
    for _, r in summary_df.iterrows():
        row = " & ".join([
            latex_escape(str(r["Config"])),
            # fmt_minutes(r["AvgTimePerQueryMin"]),
            fmt_ratio(r["QueriesPerIssue"]),
            fmt_ratio(r["AvgPredicatesPerInvariant"]),
            fmt_ratio(r["StdPredicatesPerInvariant"]),
            #fmt_minutes(r["AvgInsightRunMin"]),
            fmt_minutes(r["TotalRuntimeMin"]),
            fmt_minutes_with_pct(r["TotalJGFinishedMin"], r["TotalRuntimeMin"]),
            fmt_minutes_with_pct(r["TotalInsightFinishedMin"], r["TotalRuntimeMin"]),
            r["IssuesText"],
        ]) + r" \\"
        rows_tex.append(row)
    
    table_tex = r"""
    \begin{table*}[t]
    \centering
    \caption{JasperGold query stats, average \Insight{} run time, discovered issues, and finished-time totals.}
    \label{tab:modelcheckstats}
    \renewcommand{\arraystretch}{1.05}
    \scriptsize
    \begin{tabularx}{\textwidth}{l c c c c c c X}
    \toprule
        extbf{Config} & \textbf{\#Queries / issue} & \textbf{Avg \#pred/inv} & \textbf{Stddev \#pred/inv} & \textbf{Total runtime (min)} & \textbf{Total time JG (min)} & \textbf{Total time Insight (min)} & \textbf{Discovered issues} \\
    \midrule
    """ + "\n".join(rows_tex) + r"""
    \bottomrule
    \end{tabularx}
    \end{table*}
    """.strip()
    
    with open(os.path.join(OUTPUT_DIR, "summary_table.tex"), "w", encoding="utf-8") as fh:
        fh.write(table_tex + "\n")
    
    print("✅ Saved: summary_table.tex")
