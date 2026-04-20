#./scripts/run_full_pipeline.sh --config configs/vincent_kronos_pipeline_config.json --bex-weight 25 --predicate-cost 50 | tee output.txt
#./scripts/run_full_pipeline.sh --config configs/vincent_kronos_pipeline_config.json --no-insight | tee output.txt
#exit 0
#./scripts/run_full_pipeline.sh --config configs/vincent_kronos_pipeline_config.json --bex-weight 50 --predicate-cost 50 | tee output.txt
#./scripts/run_full_pipeline.sh --config configs/vincent_kronos_pipeline_config.json --bex-weight 25 --predicate-cost 50 | tee output.txt
#exit 0
./orchestration/model_checking_algorithm.sh --config example_cores/configs/vincent_kronos_pipeline_config.json --bex-weight 25 --predicate-cost 25 --mock-cex-path tests/unit_tests/kronos_cex/ | tee test_kronos.txt
#./scripts/run_full_pipeline.sh --config configs/vincent_kronos_pipeline_config.json --bex-weight 25 --predicate-cost 50 --mock-cex-path unit_tests/kronos_cex/ | tee output.txt
