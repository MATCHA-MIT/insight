#!/bin/bash
set -x
BINARY="./example_cores/compare_to_boom_buggy/build/boom_baseline/Vcorrectness"
FILE_PATH=$(realpath "$1")
EXTRA_ARGS="${@:2}"

if [ -n "$DEBUG" ]; then
    # -u generates the name but does not create the file
    TEMP_WAVE=$(mktemp -u /tmp/waveform.XXXXXX.fst)
    
    # Run binary with waveform argument
    $BINARY $FILE_PATH $EXTRA_ARGS +waveform=$TEMP_WAVE
    # Process the generated log
    echo $TEMP_WAVE
    ./formula_finder/target/release/debug_diff_waveform "$TEMP_WAVE"
else
    # Run normally without debug overhead
    $BINARY "$FILE_PATH" $EXTRA_ARGS
fi

