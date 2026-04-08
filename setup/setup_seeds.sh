#!/bin/bash
mkdir -p testcases/source_files/automatically_generated/
rm testcases/source_files/automatically_generated/*.s
python3 util_scripts/generate_seed_source_files.py testcases/source_files/automatically_generated/
./scripts/compile_seeds.sh testcases/source_files/ seeds/
./scripts/compile_seeds.sh testcases/source_files/automatically_generated seeds/
