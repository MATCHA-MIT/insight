#!/bin/bash
set -e

# Default values
INVARIANT_PATH="tmp_output/invariant.json"
OUTPUT_SETS="tmp_output/output_sets.json"
CONFIG="example_cores/configs/boom_reduced.json"
PREDICATE_BASE_COST="50"
BEX_MULTIPLIER="25"

# Function to display usage
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  -i, --invariant-path PATH      Path to the invariant JSON file to debug (default: invariant.json)"
    echo "  -o, --output-sets FILE         Output sets JSON file (default: output_sets.json)"
    echo "  -c, --config FILE              Regex config JSON file (default: example_cores/configs/boom_reduced.json)"
    echo "  -p, --predicate-base-cost NUM  Predicate base cost (default: 50)"
    echo "  -b, --bex-multiplier NUM       BEX multiplier (default: 25)"
    echo "  -h, --help                     Show this help message"
    exit 0
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -i|--invariant-path)
            INVARIANT_PATH="$2"
            shift 2
            ;;
        -o|--output-sets)
            OUTPUT_SETS="$2"
            shift 2
            ;;
        -c|--config)
            CONFIG="$2"
            shift 2
            ;;
        -p|--predicate-base-cost)
            PREDICATE_BASE_COST="$2"
            shift 2
            ;;
        -b|--bex-multiplier)
            BEX_MULTIPLIER="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

# Set environment variables for Gurobi
export PATH=$PATH:$HOME/gurobi/gurobi1201/linux64/bin
export GUROBI_HOME=$HOME/gurobi/gurobi1201/linux64
export LD_LIBRARY_PATH=$HOME/gurobi/gurobi1201/linux64/lib

echo "Building formula finder..."
pushd formula_finder > /dev/null
cargo build --release
popd > /dev/null

echo "Running debug_invariant with:"
echo "  Invariant path: $INVARIANT_PATH"
echo "  Output sets: $OUTPUT_SETS"
echo "  Config: $CONFIG"
echo "  Predicate base cost: $PREDICATE_BASE_COST"
echo "  BEX multiplier: $BEX_MULTIPLIER"

./formula_finder/target/release/debug_invariant \
    --output-sets "$OUTPUT_SETS" \
    --regex-config "$CONFIG" \
    --invariant-path "$INVARIANT_PATH" \
    --predicate-base-cost "$PREDICATE_BASE_COST" \
    --bex-multiplier "$BEX_MULTIPLIER"
