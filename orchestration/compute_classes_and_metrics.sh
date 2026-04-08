#!/bin/bash
#python3 evaluation_scripts/compute_classes.py --cexs_dir evaluation_data/dedup/testcases/reduced-testcases-k1-k4-k5/ --class_file evaluation_data/dedup/testcases/ground_truth.json --sweep_dir  evaluation_data/dedup/insight_output/20251103_120711/
#python3 evaluation_scripts/generalization_metrics_computation.py evaluation_data/dedup/testcases/ground_truth.json evaluation_data/dedup/insight_output/20251103_120711/

# Parse command line arguments
CEXS_DIR="${1:-evaluation_data/dedup/testcases/reduced-testcases-k1-k4-k5/}"
# Remove default value from SWEEP_DIR to make it required
SWEEP_DIR="${2:?Error: SWEEP_DIR is required}"
CLASS_FILE="${3:-evaluation_data/dedup/testcases/ground_truth.json}"


# Display usage if --help is provided
if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo "Usage: $0 [CEXS_DIR] [SWEEP_DIR] [CLASS_FILE]"
    echo ""
    echo "Arguments:"
    echo "  CEXS_DIR    - Directory containing test cases (default: evaluation_data/dedup/testcases/reduced-testcases-k1-k4-k5/)"
    echo "  CLASS_FILE  - Ground truth JSON file (default: evaluation_data/dedup/testcases/ground_truth.json)"
    echo "  SWEEP_DIR   - Insight output directory (default: evaluation_data/dedup/insight_output/20251103_120711/)"
    exit 0
fi

python3 orchestration/compute_classes.py --cexs_dir "$CEXS_DIR" --class_file "$CLASS_FILE" --sweep_dir "$SWEEP_DIR"
python3 orchestration/generalization_metrics_computation.py "$CLASS_FILE" "$SWEEP_DIR"
