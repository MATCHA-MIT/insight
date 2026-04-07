#!/bin/bash
pushd .
set -o pipefail &&     cd /home/kellyx/formal/chipyard && java -cp /home/kellyx/formal/chipyard/.classpath_cache/chipyard.jar chipyard.Generator  --target-dir /home/kellyx/formal/chipyard/sims/verilator/generated-src/chipyard.harness.TestHarness.SmallBoomV3Config --name chipyard.harness.TestHarness.SmallBoomV3Config --top-module chipyard.harness.TestHarness --legacy-configs chipyard:SmallBoomV3Config   | tee /home/kellyx/formal/chipyard/sims/verilator/generated-src/chipyard.harness.TestHarness.SmallBoomV3Config/chipyard.harness.TestHarness.SmallBoomV3Config.chisel.log
popd .
