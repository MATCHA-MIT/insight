#!/bin/bash
set -e
BOOM_VERSION=$1
OUTPUT_DIR=${2:-verilog_externalmem_original}  # Default to original if not provided
mkdir -p "$OUTPUT_DIR"
if [ -z "$BOOM_VERSION" ]; then
    BOOM_VERSION=SmallBoomV3CascadeConfig
fi
echo $BOOM_VERSION
#list="AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_524288x32.sv regfile_32x32.sv"
#list="AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_524288x32.sv regfile_32x64.sv"
list="CircularBufferNoReadOut.sv PipelineBuffer.sv AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_*x64.sv regfile_32x64.sv"
for file in $list; do 
    echo "Downloading $file"
    rsync -r --progress -v /home/vincent/formal/chipyard-private/sims/verilator/generated-src/chipyard.harness.TestHarness.SodorStage1CascadeBit64Config/gen-collateral/Sodor_$file sodor_verilog/
done
rm -rf "$OUTPUT_DIR/"
mkdir -p "$OUTPUT_DIR"
echo "Copying from gen-collateral for $BOOM_VERSION"
cp -r /home/vincent/formal/chipyard-private/sims/verilator/generated-src/chipyard.harness.TestHarness.$BOOM_VERSION/gen-collateral/* "$OUTPUT_DIR/" 2>/dev/null || echo "cp failed"
# If filelist.f is not copied, abort
if [ ! -f "$OUTPUT_DIR/filelist.f" ]; then
    echo "filelist.f not found in gen-collateral for $BOOM_VERSION, aborting"
    exit 1
else
    echo "filelist.f found in gen-collateral"
fi
# Ensure mems.v lines are present
if ! grep -q "chipyard.harness.TestHarness.$BOOM_VERSION.model.mems.v" "$OUTPUT_DIR/filelist.f"; then
    echo "chipyard.harness.TestHarness.$BOOM_VERSION.model.mems.v" >> "$OUTPUT_DIR/filelist.f"
fi
if ! grep -q "chipyard.harness.TestHarness.$BOOM_VERSION.top.mems.v" "$OUTPUT_DIR/filelist.f"; then
    echo "chipyard.harness.TestHarness.$BOOM_VERSION.top.mems.v" >> "$OUTPUT_DIR/filelist.f"
fi
# Remove SimDRAM.v from filelist.f
sed -i '/SimDRAM\.v/d' "$OUTPUT_DIR/filelist.f" 

# Remove SimUART.v from filelist.f (it pulls DPI symbols like uart_tick/uart_init)
sed -i '/SimUART\.v/d' "$OUTPUT_DIR/filelist.f"

# Remove TestHarness.sv from filelist.f (we build with --top-module correctness)
sed -i '/TestHarness\.sv/d' "$OUTPUT_DIR/filelist.f"
sed -i '/UARTAdapter\.sv/d' "$OUTPUT_DIR/filelist.f"
