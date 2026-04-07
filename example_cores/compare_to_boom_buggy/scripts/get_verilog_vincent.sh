#!/bin/bash
set -e
mkdir -p verilog_original
#list="AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_524288x32.sv regfile_32x32.sv"
#list="AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_524288x32.sv regfile_32x64.sv"
list="CircularBufferNoReadOut.sv PipelineBuffer.sv AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_16x64.sv regfile_32x64.sv"
for file in $list; do 
    echo "Downloading $file"
    rsync -r --progress -v /home/vincent/formal/chipyard-private/sims/verilator/generated-src/chipyard.harness.TestHarness.SodorStage1Bit64Config/gen-collateral/Sodor_$file sodor_verilog/Sodor_$file
done
#cp -r /home/vincent/formal/chipyard-private/sims/verilator/generated-src/chipyard.harness.TestHarness.SmallBoomV3Config/gen-collateral/* verilog_externalmem_original/ 
