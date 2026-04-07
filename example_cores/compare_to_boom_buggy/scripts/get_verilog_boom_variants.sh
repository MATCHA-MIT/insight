#!/bin/bash
# get_verilog_boom_variants.sh
#
# Compile BOOM Verilog for one or all variants and populate boom_designs/.
#
# Fix-inclusion matrix:
#   boom_baseline  -> SmallBoomV3AllBugsConfig  (b1+b2+b3+b4+b5 injected)
#   boom_all_fix   -> SmallBoomV3NoBugConfig    (no bugs)
#   boom_no_b1_fix -> SmallBoomV3Bug1Config     (only b1 injected)
#   boom_no_b2_fix -> SmallBoomV3Bug2Config     (only b2 injected)
#   boom_no_b3_fix -> SmallBoomV3Bug3Config     (only b3 injected)
#   boom_no_b4_fix -> SmallBoomV3Bug4Config     (only b4 injected)
#   boom_no_b5_fix -> SmallBoomV3Bug5Config     (only b5 injected)
#
# Usage:
#   ./get_verilog_boom_variants.sh              # build all 7 variants
#   ./get_verilog_boom_variants.sh boom_no_b3_fix   # build a single variant

set -e

source /home/vincent/formal/chipyard-private/env.sh

CHIPYARD_SIM_DIR=/home/vincent/formal/chipyard-private/sims/verilator
GEN_COLLATERAL_BASE="${CHIPYARD_SIM_DIR}/generated-src/chipyard.harness.TestHarness"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BOOM_DESIGNS_DIR="${SCRIPT_DIR}/../boom_designs"

declare -A VARIANT_TO_CONFIG=(
    [boom_baseline]="SmallBoomV3AllBugsConfig"
    [boom_all_fix]="SmallBoomV3NoBugConfig"
    [boom_no_b1_fix]="SmallBoomV3Bug1Config"
    [boom_no_b2_fix]="SmallBoomV3Bug2Config"
    [boom_no_b3_fix]="SmallBoomV3Bug3Config"
    [boom_no_b4_fix]="SmallBoomV3Bug4Config"
    [boom_no_b5_fix]="SmallBoomV3Bug5Config"
)

ALL_VARIANTS=(boom_baseline boom_all_fix boom_no_b1_fix boom_no_b2_fix boom_no_b3_fix boom_no_b4_fix boom_no_b5_fix)

build_variant() {
    local VARIANT="$1"
    local CONFIG="${VARIANT_TO_CONFIG[$VARIANT]}"
    if [[ -z "$CONFIG" ]]; then
        echo "ERROR: Unknown variant '$VARIANT'. Valid variants: ${ALL_VARIANTS[*]}"
        exit 1
    fi

    echo "=== Building $VARIANT ($CONFIG) ==="
    pushd "$CHIPYARD_SIM_DIR" > /dev/null
    make verilog CONFIG="$CONFIG"
    popd > /dev/null

    local SRC="${GEN_COLLATERAL_BASE}.${CONFIG}/gen-collateral"
    local DEST="${BOOM_DESIGNS_DIR}/${VARIANT}"
    echo "=== Copying generated verilog -> $DEST ==="
    rm -rf "$DEST"
    mkdir -p "$DEST"
    cp -r "${SRC}/." "$DEST/"

    # build_config/filelist.f expects these canonical filenames regardless of variant.
    local MODEL_MEMS_SRC="${DEST}/chipyard.harness.TestHarness.${CONFIG}.model.mems.v"
    local TOP_MEMS_SRC="${DEST}/chipyard.harness.TestHarness.${CONFIG}.top.mems.v"
    local MODEL_MEMS_DST="${DEST}/chipyard.harness.TestHarness.SmallBoomV3Config.model.mems.v"
    local TOP_MEMS_DST="${DEST}/chipyard.harness.TestHarness.SmallBoomV3Config.top.mems.v"

    if [[ -f "$MODEL_MEMS_SRC" && -f "$TOP_MEMS_SRC" ]]; then
        cp "$MODEL_MEMS_SRC" "$MODEL_MEMS_DST"
        cp "$TOP_MEMS_SRC" "$TOP_MEMS_DST"
    else
        echo "ERROR: Expected mem files not found for $CONFIG in $DEST"
        echo "       Missing: $MODEL_MEMS_SRC and/or $TOP_MEMS_SRC"
        exit 1
    fi

    echo "=== Done: $VARIANT ==="
}

if [[ $# -eq 0 ]]; then
    # Build all variants
    for v in "${ALL_VARIANTS[@]}"; do
        build_variant "$v"
    done
else
    # Build requested variants
    for v in "$@"; do
        build_variant "$v"
    done
fi

echo ""
echo "All requested BOOM variants populated in boom_designs/."
echo "Run 'make all_boom_variants' to compile shared libraries."
