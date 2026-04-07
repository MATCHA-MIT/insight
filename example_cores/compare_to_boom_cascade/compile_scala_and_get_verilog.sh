#!/bin/bash
# set -e  # Removed to continue on errors
pushd .
# List of BOOM configurations
CONFIGS=(
    "SmallBoomV3CascadeBug2Config"
    "SmallBoomV3CascadeBug3Config"
    "SmallBoomV3CascadeBug4Config"
    "SmallBoomV3CascadeBug5Config"
    "SmallBoomV3CascadeBugRenameConfig"
    "SmallBoomV3CascadeAllBugsConfig"
    "SmallBoomV3CascadeNoBugConfig"
)

for CONFIG in "${CONFIGS[@]}"; do
    echo "Compiling Scala for $CONFIG"
    cd /home/vincent/formal/chipyard-private/sims/verilator
    if make verilog CONFIG=$CONFIG; then
        popd
        OUTPUT_DIR="verilog_externalmem_$CONFIG"
        echo "Generating Verilog for $CONFIG to $OUTPUT_DIR"
        ./get_verilog_vincent.sh "$CONFIG" "$OUTPUT_DIR"
        pushd .
    else
        echo "Failed to compile $CONFIG, skipping"
        popd
        pushd .
    fi
done

# Compile Sodor once
cd /home/vincent/formal/chipyard-private/sims/verilator
make verilog CONFIG=SodorStage1CascadeBit64Config -j SV_MODULE_PREFIX=Sodor_
popd

make


