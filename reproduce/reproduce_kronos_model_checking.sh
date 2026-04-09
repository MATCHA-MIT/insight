#!/bin/bash
# Kronos model checking pipeline reproduction

if [ -d ".venv" ]; then
    source .venv/bin/activate
fi

mkdir -p logs

./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_kronos_pipeline_config.json --bex-weight 25 --predicate-cost 50 | tee logs/output_kronos.txt
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_kronos_pipeline_config.json --bex-weight 50 --predicate-cost 50 | tee logs/output_kronos.txt

# Kronos without insight
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_kronos_pipeline_config.json --no-insight | tee logs/output_kronos_no_insight.txt

