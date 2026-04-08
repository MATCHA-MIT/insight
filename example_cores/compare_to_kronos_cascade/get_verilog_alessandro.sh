#!/bin/bash
set -e
mkdir -p verilog_original
#list="AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_524288x32.sv regfile_32x32.sv"
list="AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_*x32.sv regfile_32x32.sv CircularBufferNoReadOut.sv PipelineBuffer.sv"
for file in $list; do 
    echo "Downloading $file"
    rsync -r --progress -v /home/alessandrob/chipyard/sims/verilator/generated-src/chipyard.harness.TestHarness.SodorStage1Bit32Config/gen-collateral/$file sodor_verilog/$file
done

