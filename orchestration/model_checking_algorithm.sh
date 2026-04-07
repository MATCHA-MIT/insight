#!/bin/bash

# Set the PATH environment variable, appending the new value
#export PATH=$PATH:$HOME/gurobi/gurobi1202/linux64/bin

# Set the GUROBI_HOME environment variable
#export GUROBI_HOME=$HOME/gurobi/gurobi1202/linux64

# Set the LD_LIBRARY_PATH environment variable
#export LD_LIBRARY_PATH=$HOME/gurobi/gurobi1202/linux64/lib

pushd .
cd formula_finder/
cargo build --release
popd
# Call the Python script with all provided arguments, with -u so we can pipe
python3 -u orchestration/model_checking_algorithm.py "$@"
