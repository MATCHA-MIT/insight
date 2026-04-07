#!/bin/bash
# BOOM model checking pipeline reproduction

if [ -d ".venv" ]; then
    source .venv/bin/activate
fi

mkdir -p logs

# BOOM buggy baseline
#./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_boom_buggy_pipeline_config.json --no-insight | tee logs/output_boom_buggy_no_insight.txt

# BOOM buggy bex weight 25 predicate cost 50
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_boom_buggy_pipeline_config.json --bex-weight 25 --predicate-cost 50 | tee logs/output_boom_buggy.txt

# BOOM buggy bex weight 50 predicate cost 50
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_boom_buggy_pipeline_config.json --bex-weight 50 --predicate-cost 50 | tee logs/output_boom_buggy_50_50.txt
