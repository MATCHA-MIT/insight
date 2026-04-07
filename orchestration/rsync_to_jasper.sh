#!/bin/bash
#set -x
# Check if an argument was provided
echo "First arguemtn $1"
if [ -z "$2" ]; then
  # No argument provided, assign default value
  target="mtlsim:/homes/dlangus/formal/formal-verif"
else
  # Argument provided, assign the first argument to target
  target="$2"
fi
if [ -z "$1" ]; then
  echo "No source directory provided, using default: $target"
else
  source_dir="$1"
  echo "Using provided source directory: $source_dir"
fi
echo "Rysncing formal from $source_dir to $target"
RSYNC_OPTIONS="--exclude=compare_cores/src/compare_to_ibex/ibex/vendor/riscv-isa-sim --exclude=obj_dir --exclude=target --exclude=testbench_verilator_dir --exclude=testbench_library_verilator_dir --exclude=.git"
set -x
rsync $RSYNC_OPTIONS -t -r --progress "$source_dir" "$target"
