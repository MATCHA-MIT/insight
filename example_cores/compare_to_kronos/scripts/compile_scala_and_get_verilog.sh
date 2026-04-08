#!/bin/bash
set -e
pushd .

cd /home/vincent/formal/chipyard-private/sims/verilator
#make verilog CONFIG=SmallBoomV3Config 
make clean CONFIG=SodorStage1Bit32Config -j SV_MODULE_PREFIX=Sodor_
make verilog CONFIG=SodorStage1Bit32Config -j SV_MODULE_PREFIX=Sodor_
popd
./get_verilog_vincent.sh

make


