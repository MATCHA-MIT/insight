#!/bin/bash
# Kronos model checking pipeline reproduction

if [ -d ".venv" ]; then
    source .venv/bin/activate
fi

mkdir -p logs

# Kronos baseline
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_kronos_pipeline_config.json --bex-weight 25 --predicate-cost 50 | tee logs/output_kronos.txt

# Kronos without insight
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_kronos_pipeline_config.json --no-insight | tee logs/output_kronos_no_insight.txt

# Kronos cascade baseline
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_kronos_cascade_config.json --bex-weight 25 --predicate-cost 50 | tee logs/output_kronos_cascade.txt

# Kronos cascade without insight
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_kronos_cascade_config.json --no-insight | tee logs/output_kronos_cascade_no_insight.txt
