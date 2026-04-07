#!/bin/bash
mkdir -p verilog_original
#list="AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_524288x32.sv regfile_32x32.sv"
list="AsyncScratchPadMemory.sv  CSRFile.sv  DatPath.sv  MemReader.sv  SodorInternalTile.sv Core.sv CtlPath.sv MemWriter.sv SodorRequestRouter.sv mem_*x32.sv regfile_32x32.sv PipelineBuffer.sv CircularBufferNoReadOut.sv mem_64x64.sv"
for file in $list; do 
    echo "Downloading $file"
    rsync -r --progress -v /home/viniul/formal/chipyard-private/sims/verilator/generated-src/chipyard.harness.TestHarness.SodorStage1Bit64Config/gen-collateral//Vincent_$file sodor_verilog/Vincent_$file
done

