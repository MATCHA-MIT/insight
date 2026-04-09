#!/bin/bash

# Help message
usage() {
    echo "Usage: $0 <config_file> <testcase_bin> [invariants_dir]"
    echo ""
    echo "Arguments:"
    echo "  <config_file>   Path to the JSON pipeline configuration file (e.g. example_cores/configs/vincent_boom_buggy_pipeline_config.json)."
    echo "  <testcase_bin>  Path to the binary counterexample/testcase file."
    echo "  [invariants_dir] Optional path to a directory containing invariants to check."
    exit 1
}

# Check minimum arguments
if [ "$#" -lt 2 ]; then
    usage
fi

# Set the environment variables if needed (uncomment if using Gurobi)
#export PATH=$PATH:$HOME/gurobi/gurobi1202/linux64/bin
#export GUROBI_HOME=$HOME/gurobi/gurobi1202/linux64
#export LD_LIBRARY_PATH=$HOME/gurobi/gurobi1202/linux64/lib

CONFIG_FILE=$1
TESTCASE_BIN=$2
INVARIANTS_DIR=$3

# Verify files exist
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Config file '$CONFIG_FILE' not found."
    exit 1
fi

if [ ! -f "$TESTCASE_BIN" ]; then
    echo "Error: Testcase binary '$TESTCASE_BIN' not found."
    exit 1
fi

# Extract checker path (verilator_script) from config using jq
CHECKER_PATH=$(jq -r '.verilator_script' "$CONFIG_FILE")

if [ "$CHECKER_PATH" == "null" ] || [ -z "$CHECKER_PATH" ]; then
    echo "Error: 'verilator_script' field not found in $CONFIG_FILE"
    exit 1
fi

echo "--- Analysis Configuration ---"
echo "Config File:  $CONFIG_FILE"
echo "Testcase Bin: $TESTCASE_BIN"
echo "Checker Path: $CHECKER_PATH"
if [ -n "$INVARIANTS_DIR" ]; then
    echo "Invariants:   $INVARIANTS_DIR"
fi
echo "------------------------------"

# Set up PYTHONPATH so analyzer.py can find its modules from the root
export PYTHONPATH=$PYTHONPATH:$(pwd):$(pwd)/common:$(pwd)/plotting

# Run the analyzer using common/analyzer.py
if [ -z "$INVARIANTS_DIR" ]; then
    python3 common/analyzer.py "$CHECKER_PATH" "$TESTCASE_BIN" "$CONFIG_FILE"
else
    python3 common/analyzer.py "$CHECKER_PATH" "$TESTCASE_BIN" "$CONFIG_FILE" "$INVARIANTS_DIR"
fi
