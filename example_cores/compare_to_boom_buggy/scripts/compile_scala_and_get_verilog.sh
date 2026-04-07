#!/bin/bash
set -e
pushd .
source /home/vincent/formal/chipyard-private/env.sh
cd /home/vincent/formal/chipyard-private/sims/verilator
#make clean CONFIG=SmallBoomV3Config 
#make verilog CONFIG=SmallBoomV3Config 
#exit 0
make verilog CONFIG=SodorStage1Bit64Config -j SV_MODULE_PREFIX=Sodor_
popd
./scripts/get_verilog_vincent.sh
make


