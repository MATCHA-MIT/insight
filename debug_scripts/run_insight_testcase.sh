#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

export GUROBI_VERSION=gurobi1201
# Set the PATH environment variable, appending the new value
export PATH=$PATH:$HOME/gurobi/$GUROBI_VERSION/linux64/bin

# Set the GUROBI_HOME environment variable
export GUROBI_HOME=$HOME/gurobi/$GUROBI_VERSION/linux64

# Set the LD_LIBRARY_PATH environment variable
export LD_LIBRARY_PATH=$HOME/gurobi/$GUROBI_VERSION/linux64/lib
mkdir -p "$ROOT_DIR/seeds"

# Default values
output="$(mktemp -d -t output-XXXXXX)"
config="$ROOT_DIR/example_cores/configs/vincent_kronos_pipeline_config.json"
bex_multiplier=50
predicate_cost=5

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --testcase)
            testcase="$2"
            shift 2
            ;;
        --output)
            output="$2"
            shift 2
            ;;
        --testbench)
            testbench="$2"
            shift 2
            ;;
        --config)
            config="$2"
            shift 2
            ;;
        --bex-weight)
            bex_multiplier="$2"
            shift 2
            ;;
        --predicate-cost)
            predicate_cost="$2"
            shift 2
            ;;
        --jg-signal-list)
            jg_signal_list="$2"
            shift 2
            ;;
        *)
            echo "Unknown parameter: $1"
            exit 1
            ;;
    esac
done

# Check if required arguments are provided
if [[ -z "${testcase:-}" ]]; then
    echo "Error: --testcase is required."
    exit 1
fi

config="$(realpath "$config")"
output="$(realpath -m "$output")"
testcase="$(realpath "$testcase")"

# Debugging output (optional)
echo "testcase: $testcase"
echo "Output: $output"
echo "Config: $config"

# Main script logic
# Replace the following line with your actual script logic
echo "Running with target=$testcase, output=$output, config=$config"


# Argument provided, use the first argument as output dir
mkdir -p "$output/benign_examples"
mkdir -p "$output/cexs"
mkdir -p "$output/invariants"
additional_args=""
if [[ -n "${jg_signal_list:-}" ]]; then
    additional_args="--test_allowed_signals $jg_signal_list"
fi

# Copy all invariants from the "seed_invariants" field in the config to the output directory
mapfile -t seed_invariants < <(jq -r '.seed_invariants[]? // empty' "$config")
for invariant in "${seed_invariants[@]}"; do
    invariant_path="$(realpath "$ROOT_DIR/$invariant")"
    cp "$invariant_path" "$output/invariants/"
done

python3 -u "$ROOT_DIR/common/cex_generator.py" "$testcase" "$output" --log-level DEBUG --config-path "$config" $additional_args
output_sets_path="$output/output_sets.json"
core_config_path="$(jq -r '.regex_config_path' "$config")"
core_config_path="$(realpath "$ROOT_DIR/$core_config_path")"

formula_finder_bin="$ROOT_DIR/formula_finder/target/release/invariant_finder_rust"
if [[ ! -x "$formula_finder_bin" ]]; then
    (cd "$ROOT_DIR/formula_finder" && cargo build --release)
fi

set -x
"$formula_finder_bin" \
    --output-sets "$output_sets_path" \
    --regex-config "$core_config_path" \
    --invariant-out-path "$output/invariant.json" \
    --bex-multiplier "$bex_multiplier" \
    --predicate-base-cost "$predicate_cost"
set +x
echo "For output set $output/output_sets"
