import json
import os
import re
import sys
from pathlib import Path

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))

from itertools import combinations
from collections import defaultdict
import matplotlib.pyplot as plt
import matplotlib as mpl
from matplotlib.ticker import MaxNLocator
import numpy as np


# ============================================================
# Helper functions
# ============================================================

def build_gt_positive_pairs(ground_truth_labels):
    """
    Build a set of CE pairs that share at least one ground-truth class.
    Each CE may appear in multiple classes (k1..k5).
    """
    pairs = set()
    all_ces = set()
    for label, ces in ground_truth_labels.items():
        ces = [c.split("/")[-1] for c in ces]
        all_ces.update(ces)
        for ce1, ce2 in combinations(sorted(set(ces)), 2):
            pairs.add((ce1, ce2))
    return pairs, all_ces


def build_result_pairs(this_results):
    """Build a set of CE pairs that are grouped together in a given result."""
    pairs = set()
    all_ces = set()
    total_len = 0
    group_sizes = []
    for group, ces in this_results.items():
        try:
            if len(ces) == 0:
                ces = []
                #raise Exception(f"No cex for {group}")
            elif type(ces[0]) == dict:
                ces = [c["file"].split("/")[-1] for c in ces]
            elif type(ces[0]) == str:
                ces = [c.split("/")[-1] for c in ces]
        except Exception as e:
            print(ces)
            raise e

        group_sizes.append(len(set(ces)))
        ces = list(set(ces))
        total_len += len(ces)
        all_ces.update(ces)

        i = 0
        for ce1, ce2 in combinations(ces, 2):
            if ce1 == ce2:
                continue
            x, y = (ce1, ce2) if ce1 < ce2 else (ce2, ce1)
            i += 1
            pairs.add((x, y))
        assert i == len(ces) * (len(ces) - 1) // 2, f"Mismatch in combinations for group {group}"
    
    if group_sizes:
        print(f"[DEBUG] Number of groups: {len(group_sizes)}")
        print(f"[DEBUG] Min/Max/Avg group size: {min(group_sizes)} / {max(group_sizes)} / {sum(group_sizes)/len(group_sizes):.2f}")

    return pairs, all_ces

# This function is necessary to build inverted maps
# Instead of class -> cexs, we need cex -> classes
# Both predicted and ground-truth
def build_label_maps(ground_truth_labels, this_results):
    """
    Returns:
      GT_map: dict[ce] -> set[str]        # ground-truth classes per CE
      PR_map: dict[ce] -> set[str]        # predicted groups per CE (overlapping ok)
    """

    GT_map = {}
    for cls, ces in ground_truth_labels.items():
        norm = [c.split("/")[-1] for c in ces]
        for ce in norm:
            GT_map.setdefault(ce, set()).add(cls)

    PR_map = {}
    for group, ces in this_results.items():
        if not ces:
            continue
        if isinstance(ces[0], dict):
            ces = [c["file"].split("/")[-1] for c in ces]
        else:
            ces = [c.split("/")[-1] for c in ces]
        for ce in set(ces):
            PR_map.setdefault(ce, set()).add(group)

    return GT_map, PR_map

def remove_large_predicted_classes(results, threshold=0.8):
    """
    Remove predicted classes that cover more than `threshold` fraction
    of all unique counterexamples (CEs). Handles both dict and str types.
    """
    # Collect all unique CEs across all groups
    all_items = set()
    for ces in results.values():
        if not ces:
            continue
        if isinstance(ces[0], dict):
            all_items.update(c["file"].split("/")[-1] for c in ces)
        elif isinstance(ces[0], str):
            all_items.update(c.split("/")[-1] for c in ces)
        else:
            raise ValueError(f"Unexpected CE type: {type(ces[0])}")
    
    total = len(all_items)
    if total == 0:
        print("[WARN] No items found in results — skipping filter.")
        return results

    filtered = {}
    removed = []
    
    for group, ces in results.items():
        if not ces:
            continue
        if isinstance(ces[0], dict):
            norm_ces = [c["file"].split("/")[-1] for c in ces]
        elif isinstance(ces[0], str):
            norm_ces = [c.split("/")[-1] for c in ces]
        else:
            continue

        unique_count = len(set(norm_ces))
        frac = unique_count / total

        if frac > threshold:
            removed.append((group, frac))
        else:
            filtered[group] = ces

    if removed:
        print("[INFO] Removed large predicted classes:")
        for name, frac in removed:
            print(f"  {name}: covers {frac*100:.1f}% of all items")

    return filtered

def omega_index(GT_map, PR_map):
    """
    Compute the Omega Index for overlapping clusterings.
    Adapted from:
        - Collins & Dent (1988)
        - McDaid et al. (2011), "Normalized Mutual Information for Overlapping Clusterings"
    """

    # Build inverted maps: label -> items
    inv_GT = defaultdict(set)
    inv_PR = defaultdict(set)
    for ce, labs in GT_map.items():
        for l in labs:
            inv_GT[l].add(ce)
    for ce, labs in PR_map.items():
        for l in labs:
            inv_PR[l].add(ce)

    items = sorted(set(GT_map.keys()) | set(PR_map.keys()))
    if len(items) < 2:
        return 1.0  # Trivial case

    # For each pair (i, j), count how many GT and PR groups they share
    gt_counts = []
    pr_counts = []

    for i, j in combinations(items, 2):
        gt_shared = len(GT_map.get(i, set()) & GT_map.get(j, set()))
        pr_shared = len(PR_map.get(i, set()) & PR_map.get(j, set()))
        gt_counts.append(gt_shared)
        pr_counts.append(pr_shared)

    # Build histograms of counts
    max_shared = max(max(gt_counts, default=0), max(pr_counts, default=0))
    n_pairs = len(gt_counts)
    if n_pairs == 0:
        return 1.0

    gt_hist = np.zeros(max_shared + 1)
    pr_hist = np.zeros(max_shared + 1)

    for g in gt_counts:
        gt_hist[g] += 1
    for p in pr_counts:
        pr_hist[p] += 1

    # Observed agreement: fraction of pairs that share the same number of clusters
    obs = sum(1 for g, p in zip(gt_counts, pr_counts) if g == p) / n_pairs

    # Expected agreement: chance agreement given the marginal distributions
    exp = sum((gt_hist[k] / n_pairs) * (pr_hist[k] / n_pairs) for k in range(max_shared + 1))

    omega = (obs - exp) / (1 - exp) if (1 - exp) != 0 else 1.0
    return omega

def bcubed_overlapping(labels, clusters, beta=1.0):
    items = set(labels.keys()) | set(clusters.keys())

    L = {i: set(labels.get(i, set())) for i in items}
    C = {i: set(clusters.get(i, set())) for i in items}

    items_in_label = defaultdict(set)
    items_in_cluster = defaultdict(set)

    for i in items:
        for l in L[i]:
            items_in_label[l].add(i)
        for c in C[i]:
            items_in_cluster[c].add(i)
    
    def precision_i(i):
        neigh = set()
        for c in C[i]:
            neigh |= items_in_cluster[c]
        neigh.discard(i)

        if not neigh:
            return 1.0

        num = 0.0
        den = 0.0
        for j in neigh:
            common_c = len(C[i] & C[j])
            common_l = len(L[i] & L[j])
            
            num += min(common_c, common_l) / common_c
            den += 1.0

        return num / den if den > 0 else 1.0
    
    def recall_i(i):
        neigh = set()
        for l in L[i]:
            neigh |= items_in_label[l]
        neigh.discard(i)
    
        if not neigh:
            return 1.0

        num = 0.0
        den = 0.0
        for j in neigh:
            common_c = len(C[i] & C[j])
            common_l = len(L[i] & L[j])
            
            num += min(common_c, common_l) / common_l
            den += 1.0

        return num / den if den > 0 else 1.0

    precision = sum(precision_i(i) for i in items) / len(items) if items else 1.0
    recall = sum(recall_i(i) for i in items) / len(items) if items else 1.0

    if precision == 0 and recall == 0:
        f1 = 0.0
    else:
        b2 = beta * beta
        f1 = (1 + b2) * (precision * recall) / (b2 * precision + recall)

    return precision, recall, f1


def compute_metrics(gt_positive_pairs, all_gt_ces, this_results):
    """
    Compute over-/under-generalization metrics:
        - TP: grouped & share ≥1 GT class
        - FP: grouped but share none (over-generalization)
        - FN: not grouped but share ≥1 GT class (under-generalization)
        - TN: not grouped & share none
    """
    #print("This results", this_results)
    result_pairs, result_ces = build_result_pairs(this_results)
    n_groups = len(this_results)
    all_ces = all_gt_ces | result_ces
    print("Total unique CEs considered:", len(all_ces))
    total_pairs = len(all_ces) * (len(all_ces) - 1) // 2

    tp = len(result_pairs & gt_positive_pairs)
    print(f"Result pairs: {len(result_pairs)}, GT pairs: {len(gt_positive_pairs)}, Overlap: {tp}")
    fp = len(result_pairs - gt_positive_pairs)
    fn = len(gt_positive_pairs - result_pairs)
    tn = total_pairs - (tp + fp + fn)

    return {
        "true_positive": tp,
        "true_negative": tn,
        "false_positive": fp,
        "false_negative": fn,
        "total_pairs": total_pairs,
        "n_groups": n_groups,
        "under_rate": fp / (tp+fp) if (tp+fp) else 0.0,
        "over_rate": fn / (tp+fn) if (tp+fn) else 0.0,
        "precision": tp / (tp + fp) if (tp + fp) else 0.0,
        "recall": tp / (tp + fn) if (tp + fn) else 0.0,
        "f1_score": 2 * tp / (2*tp + fp + fn) if (2*tp + fp + fn) else 0.0,
    }

def compute_per_group_overfit_underfit(gt_positive_pairs, all_gt_ces, this_results):
    """
    Compute overfit (over-generalization) and underfit (under-generalization) metrics for each predicted class individually.
    Returns a list of dicts:
      [
        {
          "group": str,
          "true_positive": int,
          "false_positive": int,
          "total_pairs": int,
          "overfit_rate": float
        },
        ...
      ]
    """
    group_metrics = []

    for group, ces in this_results.items():
        # Normalize CE names
        if not ces:
            continue
        if isinstance(ces[0], dict):
            ces = [c["file"].split("/")[-1] for c in ces]
        else:
            ces = [c.split("/")[-1] for c in ces]
        ces = list(set(ces))  # remove duplicates

        # Skip groups with <2 CEs (no pairs)
        if len(ces) < 2:
            continue

        # Build all pairs inside this group
        pairs = set()
        for ce1, ce2 in combinations(sorted(ces), 2):
            x, y = (ce1, ce2) if ce1 < ce2 else (ce2, ce1)
            pairs.add((x, y))

        tp = len(pairs & gt_positive_pairs)
        fp = len(pairs - gt_positive_pairs)
        fn = len(gt_positive_pairs - pairs)

        underfit_rate = fp / (tp + fp) if (tp + fp) else 0.0
        overfit_rate = fn / (tp + fn) if (tp + fn) else 0.0
        print(f"[DEBUG]\tGroup {group}: TP={tp}, FP={fp}, FN={fn}, Overfit rate={overfit_rate:.4f}, Underfit rate={underfit_rate:.4f}")

        group_metrics.append({
            "group": group,
            "true_positive": tp,
            "false_positive": fp,
            "false_negative": fn,
            "overfit_rate": overfit_rate,
            "underfit_rate": underfit_rate
        })

    return group_metrics

def compute_overlap_purity_matrices(ground_truth_labels, predicted_labels):
    """
    Returns:
      gt_classes, pred_classes, overlap_matrix, purity_matrix
      where overlap[i, j] = |GT_i ∩ Pred_j| / |GT_i|
            purity[i, j] = |GT_i ∩ Pred_j| / |Pred_j|
    """
    gt_classes = sorted(ground_truth_labels.keys())
    pred_classes = sorted(predicted_labels.keys())
    overlap = np.zeros((len(gt_classes), len(pred_classes)))
    purity = np.zeros((len(gt_classes), len(pred_classes)))

    for i, gt in enumerate(gt_classes):
        gt_ces = {c.split("/")[-1] for c in ground_truth_labels[gt]}
        for j, pred in enumerate(pred_classes):
            pred_ces = {
                c["file"].split("/")[-1] if isinstance(c, dict) else c.split("/")[-1]
                for c in predicted_labels[pred]
            }
            inter = len(gt_ces & pred_ces)
            overlap[i, j] = inter / len(gt_ces) if gt_ces else 0.0
            purity[i, j] = inter / len(pred_ces) if pred_ces else 0.0

    return gt_classes, pred_classes, overlap, purity

# ============================================================
# Visualization
# ============================================================

def visualize_metrics(metrics, output_dir="plots"):
    """
    Save visualizations to files instead of displaying them:
      - Scatter plot of over vs under rates
      - Bar chart summary of all configurations
    """
    os.makedirs(output_dir, exist_ok=True)


    # Sort metrics by bex then predicate cost (both numeric). Keep unknown configs at the end.
    _cfg_re = re.compile(r"bex_(\d+)_pred_(\d+)")
    def _sort_key(m):
        cfg = m.get("config", "")
        mobj = _cfg_re.search(cfg)
        if mobj:
            return (int(mobj.group(1)), int(mobj.group(2)))
        # Place unparsable configs after parsed ones
        return (float("inf"), float("inf"))

    metrics = sorted(metrics, key=_sort_key)
    configs = [m["config"] for m in metrics]
    # bcubed_precision = [m["bcubed_precision"] for m in metrics]
    # bcubed_recall = [m["bcubed_recall"] for m in metrics]
    # bcubed_f1 = [m["bcubed_f1"] for m in metrics]
    # omega_indices = [m["omega_index"] for m in metrics]

    over_rates = [m["over_rate"] for m in metrics]
    under_rates = [m["under_rate"] for m in metrics]
    n_classes = [m["n_groups"] for m in metrics]
    # precision = [m["precision"] for m in metrics]
    # recall = [m["recall"] for m in metrics]
    # f1 = [m["f1_score"] for m in metrics]

    #Scatter plot
    scatter_path = os.path.join(output_dir, "scatter_over_under_boom.png")
    plt.figure()
    plt.scatter(over_rates, under_rates, c="royalblue", s=80, alpha=0.8, edgecolors="black")
    for i, cfg in enumerate(configs):
        plt.text(over_rates[i] + 0.0005, under_rates[i] + 0.0005, cfg, fontsize=8)
    plt.xlabel("Over-generalization rate (False Positive)")
    plt.ylabel("Under-generalization rate (False Negative)")
    plt.title("Over/Under Generalization per Configuration")
    plt.grid(True, linestyle="--", alpha=0.5)
    plt.tight_layout()
    plt.savefig(scatter_path, dpi=300)
    plt.close()

    # Bar chart
    n_configs = len(configs)
    n_bars = 3  # precision, recall, f1, omega

# Compute a safe width so all groups fit nicely
    group_width = min(0.8, 0.9 / n_bars)  # optional cap
    width = (group_width / n_bars)

    x = np.array([1, 25, 50, 75, 100])
    x_labels = [f"{i}%" for i in x]

    x = np.arange(n_configs)

    plt.figure()
    # for i in range(n_configs):
    #     plt.text(
    #         x[i], max(max(under_rates), max(over_rates)) + 0.1,
    #         f"{n_classes[i]} cls.",
    #         ha="center", va="bottom",
    #         fontsize=8, color="dimgray"
    #     )

    # plt.bar(x - width - 1, precision, width, label="Precision", color="#0072B2", edgecolor="black")
    # plt.bar(x, recall, width, label="Recall", color="#D55E00", edgecolor="black")
    # plt.bar(x + width + 1, f1, width, label="F1 Score", color="#F0E442", edgecolor="black")

    # plt.ylim(0, 1.05)
    # plt.xticks(x, x, rotation=0, ha="right")
    # plt.ylabel("Rate")
    # plt.xlabel(r"BEX Penalty $\lambda$")
    # # plt.title("Metrics Across Configurations")
    # plt.legend()
    # plt.tight_layout()
    # bar_path = os.path.join(output_dir, "prec_recall_f1_metrics.png")
    # plt.savefig(bar_path, dpi=600, bbox_inches="tight")
    # plt.close()

    # print(f"\nPlots saved to: {os.path.abspath(output_dir)}")
    # # print(f"  - Scatter plot: {scatter_path}")
    # print(f"  - Bar chart:    {bar_path}")

    n_bars = 2
    group_width = min(0.8, 0.9 / n_bars)  # optional cap
    width = (group_width / n_bars)

    # x_labels = [f"{i}%" for i in x]

    fig, ax1 = plt.subplots()
    # for i, x_i in enumerate(x):
    #     ax1.text(
    #         x_i - width, under_rates[i] + 0.05,
    #         f"{under_rates[i]:.3f}",
    #         ha="center", va="center",
    #         fontsize=9, color="black"
    #     )

    ax1.bar(x-width*2/3, under_rates, width=width, color="#0072B2", label="False Positive Rate", edgecolor="black")
    # plt.bar(x + width, under_rates, width, label="Under-generalization Rate")
    # ax1.plot(x, under_rates, color="#0072B2", marker="o", linestyle="--", label="False Positive Rate")
    ax1.set_ylim(0, 1.05)
    ax1.set_xticks(x, x_labels)
    ax1.set_xlabel(r"BEX Penalty $\lambda$")
    ax1.set_ylabel("False Positive Rate")
    ax1.grid(axis="y", linestyle="--", alpha=0.5)

    ax2 = ax1.twinx()
    ax2.bar(x+width*2/3, n_classes, width=width, color="#D55E00", label="# Synth. State Formulas", edgecolor="black")
    # ax2.plot(x, n_classes, color="#D55E00", marker="o", linestyle="-", label="# Synth. State Formulas")
    ax2.set_ylabel("# Synth. State Formulas")
    ax2.yaxis.set_major_locator(MaxNLocator(integer=True))

    bars, labels = ax1.get_legend_handles_labels()
    bars2, labels2 = ax2.get_legend_handles_labels()
    ax1.legend(bars + bars2, labels + labels2, loc="upper left")

    # plt.title("Under-fitting Rates and Number of Classes Across Configurations")
    plt.tight_layout()
    bar_path = os.path.join(output_dir, "over_under_rates.png")
    plt.savefig(bar_path, dpi=600, bbox_inches="tight")
    plt.close()

    print(f"\nPlots saved to: {os.path.abspath(output_dir)}")
    # print(f"  - Scatter plot: {scatter_path}")
    print(f"  - Bar chart:    {bar_path}")


def visualize_overfit_for_single_config(all_metrics, target_config, output_dir="plots"):
    """
    Plot overfit (and optionally underfit) rates for all predicted classes
    within one selected configuration (e.g., "bex_15_pred_10").
    """
    os.makedirs(output_dir, exist_ok=True)

    # Find the metric entry for the requested config
    target_entry = None
    for entry in all_metrics:
        if entry.get("config") == target_config:
            target_entry = entry
            break

    if not target_entry:
        print(f"[ERROR] Configuration '{target_config}' not found in metrics.")
        return

    # Extract per-group (class) over/under rates
    group_metrics = target_entry.get("group_overfits", [])
    if not group_metrics:
        print(f"[WARN] No per-group metrics found for config '{target_config}'.")
        return

    # Prepare data
    group_names = [g["group"] for g in group_metrics]
    overfit_rates = [g["overfit_rate"] for g in group_metrics]
    underfit_rates = [g["underfit_rate"] for g in group_metrics]

    # Sort by overfit rate (optional, for readability)
    sorted_data = sorted(zip(group_names, overfit_rates, underfit_rates),
                         key=lambda x: x[1], reverse=True)
    group_names, overfit_rates, underfit_rates = zip(*sorted_data)

    # --- Plot ---
    plt.figure(figsize=(max(10, len(group_names) * 0.4), 4))
    plt.rcParams.update({
    "font.size": 7,
    "axes.titlesize": 12,
    "axes.labelsize": 10,
    "xtick.labelsize": 10,
    "ytick.labelsize": 10 
    })
    x = np.arange(len(group_names))

    plt.bar(x, overfit_rates, color="royalblue", edgecolor="black")
    plt.xticks(x, [i+1 for i in range(len(group_names))])
    plt.ylabel("Overfit Rate")
    plt.title(f"Per-Class Overfit Rates — {target_config}")
    plt.ylim(0, 0.6)
    plt.grid(axis="y", linestyle="--", alpha=0.5)
    plt.tight_layout()

    out_path = os.path.join(output_dir, f"overfit_rates_{target_config}.png")
    plt.savefig(out_path, dpi=300)
    plt.close()

    print(f"[INFO] Per-class overfit plot saved: {os.path.abspath(out_path)}")



def print_bug_class_statistics(this_results):
    """Print statistics about the bug classes in the results."""
    num_classes = len(this_results)
    class_size_dict = {class_name: len(ces) for class_name, ces in this_results.items()}
    class_sizes = list(class_size_dict.values())
    
    avg_size = sum(class_sizes) / num_classes if num_classes > 0 else 0
    max_size = max(class_sizes) if class_sizes else 0
    min_size = min(class_sizes) if class_sizes else 0

    print(f"Number of bug classes: {num_classes}")
    print("Class sizes statistics:")
    print(f"  Sizes: {class_size_dict}")
    print(f"Average class size: {avg_size:.2f}")
    print(f"Max class size: {max_size}")
    print(f"Min class size: {min_size}")
    # Check for duplicates in each class
    classes_with_duplicates = []
    for class_name, ces in this_results.items():
        # Extract CE identifiers
        if len(ces) > 0:
            if type(ces[0]) == dict:
                ce_ids = [c["file"].split("/")[-1] for c in ces]
            elif type(ces[0]) == str:
                ce_ids = [c.split("/")[-1] for c in ces]
            else:
                ce_ids = ces
        else:
            ce_ids = []
        
        # Check for duplicates
        if len(ce_ids) != len(set(ce_ids)):
            duplicates = [ce for ce in ce_ids if ce_ids.count(ce) > 1]
            classes_with_duplicates.append((class_name, set(duplicates)))
    
    if classes_with_duplicates:
        print(f"Classes with duplicate values: {len(classes_with_duplicates)}")
        for class_name, dups in classes_with_duplicates:
            print(f"  {class_name}: {dups}")
    else:
        print("No classes with duplicate values")

# ============================================================
# Main script
# ============================================================

def main():
    if len(sys.argv) < 3:
        print("Usage: python evaluate_generalization_visual.py <ground_truth.json> <results_dir> <reuse_results (optional)>")
        sys.exit(1)

    # Load ground truth
    with open(sys.argv[1], "r") as f:
        ground_truth_labels = json.load(f)

    if len(sys.argv) >= 4:
        reuse_results = sys.argv[3].lower() == "true"
    else:
        reuse_results = False

    # Precompute GT pairs once
    print("Precomputing ground truth pairs...")
    gt_positive_pairs, all_gt_ces = build_gt_positive_pairs(ground_truth_labels)
    print(f"  ➤ Ground truth pairs: {len(gt_positive_pairs)}")
    print(f"  ➤ Unique counterexamples: {len(all_gt_ces)}")
    # for cex in list(all_gt_ces):
    #     for cls, ces in ground_truth_labels.items():
    #         if any(cex in c for c in ces):
    #             # print(f"  - CEX {cex} in class {cls}")
    #             break
    #         else:
    #             raise Exception(f"CEX {cex} not found in any class!")
    # print("Done loading ground truth.\n")

    # Find all results directories
    results_dir = sys.argv[2]
    pattern = re.compile(r"sweep_bex_(\d+)_predcost_(\d+)")
    results = []

    for entry in os.listdir(results_dir):
        print("Processing entry", entry)
        match = pattern.match(entry)
        if not match:
            continue
        bex, pred = match.groups()
        json_path = os.path.join(results_dir, entry, "bug_classes.json")
        if not os.path.isfile(json_path):
            continue
        try:
            with open(json_path, "r") as f:
                data = json.load(f)
            config_name = f"bex_{bex}_pred_{pred}"
            results.append((config_name, data))
        except json.JSONDecodeError:
            print(f"Skipping invalid JSON: {json_path}")


    if reuse_results:
        with open(os.path.join(results_dir, "generalization_metrics_summary_mod.json"), "r") as f:
            all_metrics = json.load(f)
    else:
        all_metrics = []
        for name, data in results:
            # data = remove_large_predicted_classes(data, threshold=0.8)
            print(f"\n=== Results for {name}, num classes {len(data.keys())} ===")
            print_bug_class_statistics(data)
            metrics = compute_metrics(gt_positive_pairs, all_gt_ces, data)
            metrics["config"] = name

            group_overfits = compute_per_group_overfit_underfit(gt_positive_pairs, all_gt_ces, data)

            # gt_classes, pred_classes, overlap, purity = compute_overlap_purity_matrices(ground_truth_labels, data)
            # mean_best_overlap = np.mean(np.max(overlap, axis=1))
            # mean_best_purity = np.mean(np.max(purity, axis=0))
            # metrics["mean_best_overlap"] = mean_best_overlap
            # metrics["mean_best_purity"] = mean_best_purity
            # metrics["overlap_matrix"] = {
            #     "gt_classes": gt_classes,
            #     "pred_classes": pred_classes,
            #     "matrix": overlap.tolist()
            # }

            # precision, recall, f1 = bcubed_overlapping(*build_label_maps(ground_truth_labels, data))
            # metrics["bcubed_precision"] = precision
            # metrics["bcubed_recall"] = recall
            # metrics["bcubed_f1"] = f1

            # all_metrics.extend(group_overfits)
            metrics["group_overfits"] = group_overfits
            for k, v in metrics.items():
                if k != "config" and k != "group_overfits":
                    print(f"{k:>25}: {v:.4f}" if isinstance(v, float) else f"{k:>25}: {v}")
            all_metrics.append(metrics)
    
        with open(os.path.join(results_dir, "generalization_metrics_summary.json"), "w") as f:
            json.dump(all_metrics, f, indent=4)
    
    if all_metrics:
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
            "figure.figsize": (6.5, 3),
            "figure.dpi": 300,
            "savefig.dpi": 600,
            "savefig.format": "pdf",
            "savefig.bbox": "tight",
        })

        visualize_metrics(all_metrics, os.path.join(results_dir, "plots"))
        # visualize_overfit_for_single_config(all_metrics, target_config="bex_50_pred_50", output_dir=os.path.join(results_dir, "plots"))

if __name__ == "__main__":
    main()
