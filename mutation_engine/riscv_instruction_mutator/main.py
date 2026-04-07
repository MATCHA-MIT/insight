import argparse
from . import instruction_mutator_wrapper

def parse_args():
    parser = argparse.ArgumentParser(description="RISC-V Instruction Mutator")
    parser.add_argument("--instruction-file", "-if", help="File containing the instruction sequence to mutate in binary format", required=True)
    parser.add_argument("--output-dir", "-o", help="Directory to store the mutated instruction sequences", required=True)
    parser.add_argument("--mutation-steps", "-s", help="Number of mutation steps to perform", required=True)
    parser.add_argument("--mutation-number", "-n", help="Number of mutated instruction sequences to generate", required=True)
    parser.add_argument("--log-level", "-l", help="Logging level: one of {DEBUG, INFO, CRITICAL, WARNING, FATAL, ERROR}", default="INFO")
    parser.add_argument("--seed", "-S", help="Seed for the random number generator", default=None)
    parser.add_argument("--ignore-check", "-i", help="Ignore the check for valid instructions", action="store_true")

    return parser.parse_args()

def main():
    
    args = parse_args()
    in_file = args.instruction_file
    out_dir = args.output_dir
    mutation_steps = int(args.mutation_steps)
    mutation_number = int(args.mutation_number)
    log_level = args.log_level
    seed = float(args.seed) if args.seed else None
    ignore_check = args.ignore_check
    wrapper = instruction_mutator_wrapper.InstructionMutatorWrapper(in_file, out_dir, mutation_steps, mutation_number, log_level, seed, ignore_check)
    wrapper.run()

if __name__ == '__main__':
    main()