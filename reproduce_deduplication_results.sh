#!/bin/bash
set -e

# Source the virtual environment if it exists
if [ -f .venv/bin/activate ]; then
    echo "Sourcing virtual environment..."
    source .venv/bin/activate
else
    echo "Virtual environment (.venv) not found. Please run ./setup.sh first."
    exit 1
fi

# 1. Create results directory
mkdir -p results
cd results

# 2. Download testcases from Zenodo (if not already downloaded)
if [ ! -f boom_testcase.zip ]; then
    echo "Downloading BOOM testcases..."
    wget -O boom_testcase.zip "https://zenodo.org/records/19474847/files/boom_testcase.zip?download=1"
else
    echo "BOOM testcases already downloaded."
fi

if [ ! -f kronos_testcase.zip ]; then
    echo "Downloading Kronos testcases..."
    wget -O kronos_testcase.zip "https://zenodo.org/records/19474847/files/kronos_testcase.zip?download=1"
else
    echo "Kronos testcases already downloaded."
fi

# 3. Extract testcases
echo "Extracting testcases..."
unzip -o boom_testcase.zip -d boom_cexs
unzip -o kronos_testcase.zip -d kronos_cexs

# 4. Remove stale classification and waveforms to force re-simulation
# These files from Zenodo reflect the 128B memory core, not the 1MB cascade core.
echo "Cleaning up stale classification data and waveforms..."
find boom_cexs kronos_cexs -name "dedup-classification.json" -delete
find boom_cexs kronos_cexs -name "*.vcd" -delete

cd ..

# 4. Build Cores
echo "Building BOOM cascade core (allbugs target)..."
cd example_cores/compare_to_boom_cascade
# Building only allbugs target as some individual bug verilog directories may be missing
make -j$(nproc) obj_dir_allbugs/libcorrectness_allbugs.so
cd ../..

echo "Building Kronos cascade core (baseline target)..."
cd example_cores/compare_to_kronos_cascade
make -j$(nproc) build/baseline/libcorrectness.so
cd ../..

# 5. Build Rust Formula Finder Engine
echo "Building Rust formula finder engine..."
cd formula_finder
cargo build --release
cd ..

# 6. Run Deduplication and Metrics for BOOM
echo "Running deduplication for BOOM..."
#python3 orchestration/run_dedup_and_metrics.py \
#    --config example_cores/configs/vincent_boom_cascade_pipeline_config.json \
#    --cexs-dir results/boom_cexs/boom_cexs

# 7. Run Deduplication and Metrics for Kronos
echo "Running deduplication for Kronos..."
python3 orchestration/run_dedup_and_metrics.py \
    --config example_cores/configs/vincent_kronos_cascade_config.json \
    --cexs-dir results/kronos_cexs/kronos_testcase

echo "===================================================="
echo "Deduplication reproduction complete!"
echo "Evaluation plots can be found in the latest 'insight_output/<core>/deduplication/deduplication_*/plots' directory."
echo "===================================================="
