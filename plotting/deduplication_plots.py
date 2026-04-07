import pandas as pd
import matplotlib.pyplot as plt
import sys
from pathlib import Path

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))

import math

def plot_aggregate(input_csv="deduplication_results.csv", output_png="deduplication_stacked_bar.png"):
    df = pd.read_csv(input_csv)

    # Make sure numeric columns are actually numeric
    df["covered_cexs"] = pd.to_numeric(df["covered_cexs"], errors="coerce")

    # Filter only rows where aggregate == False
    filtered = df[df["aggregate"] == False]

    # Pivot: rows = bex_multiplier, columns = invariant, values = covered_cexs
    pivot = filtered.pivot_table(
        index="bex_multiplier",
        columns="invariant",
        values="covered_cexs",
        aggfunc="sum",
        fill_value=0
    )

    # --- Sort index numerically by the suffix ---
    pivot = pivot.reindex(
        sorted(pivot.index, key=lambda x: int(x.split("_")[1]))
    )

    # Plot stacked bar chart
    ax = pivot.plot(
        kind="bar",
        stacked=True,
        figsize=(12, 7),
        legend=False,
        colormap="tab20"  # Use a colormap for better color variety
    )

    plt.title("Covered CEXs per bex_multiplier (grouped by invariant)")
    plt.xlabel("bex_multiplier")
    plt.ylabel("Number of Covered CEXs")
    plt.xticks(rotation=0)
    plt.savefig(output_png)
    plt.close()


def plot_counts(filename):
    df = pd.read_csv(filename)

    df["coverage_count"] = pd.to_numeric(df["coverage_count"], errors="coerce")

    multipliers = sorted(df["bex_multiplier"].unique(), key=lambda x: int(x.split("_")[1]))
    n = len(multipliers)

    ncols = math.ceil(math.sqrt(n))
    nrows = math.ceil(n / ncols)

    fig, axes = plt.subplots(nrows, ncols, figsize=(5*ncols, 4*nrows), sharex=True, sharey=True)
    axes = axes.flatten()

    for ax, m in zip(axes, multipliers):
        subset = df[df["bex_multiplier"] == m]
        counts = subset["coverage_count"].value_counts().sort_index()
        print(counts)

        bars = ax.bar(counts.index, counts.values)

        # Add labels on top of bars
        ax.bar_label(bars, labels=[str(v) for v in counts.values], padding=2)

        ax.set_title(f"bex_multiplier = {m}")
        ax.set_xlabel("Coverage Count")
        ax.set_ylabel("# of CEXs")

    # for ax in axes[len(multipliers):]:
    #     ax.set_visible(False)

    plt.tight_layout()
    plt.savefig("cex_coverage_counts.png")

def plot_classes(filename):
    df = pd.read_csv(filename)

    df["covered_cexs"] = pd.to_numeric(df["covered_cexs"], errors="coerce")

    # Filter only rows where aggregate == False
    filtered = df[df["aggregate"] == False]

    multipliers = sorted(filtered["bex_multiplier"].unique(), key=lambda x: int(x.split("_")[1]))
    n = len(multipliers)

    # Count how many rows with each multiplier we have in filtered
    counts = filtered["bex_multiplier"].value_counts().reindex(multipliers, fill_value=0)
    counts = counts.sort_index(key=lambda x: x.map(lambda y: int(y.split("_")[1])))
    print(counts)

    # Create a bar plot
    ax = counts.plot(kind="bar", figsize=(10, 6))
    bars = ax.bar(counts.index, counts.values)
    ax.bar_label(bars, labels=[str(v) for v in counts.values], padding=2)
    ax.set_title("Number of Rows per bex_multiplier")
    ax.set_xlabel("bex_multiplier")
    ax.set_ylabel("Count")
    plt.tight_layout()
    plt.savefig("classes_per_multiplier.png")
    plt.close()

if __name__ == "__main__":
    if len(sys.argv) == 3:
        plot_aggregate(sys.argv[1])
        plot_classes(sys.argv[1])
        plot_counts(sys.argv[2])
    else:
        print("Provide the names of both input CSV files.")
