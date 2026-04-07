from . import disassembler
import logging
from . import mutator
import random
import os
import sys
import time
from capstone import *
from . import utils
import tempfile
import subprocess
import shutil
import enum
import tqdm
import itertools
import typing
import threading
import uuid
import concurrent.futures
import riscv_mutator
import logging_common

logger = logging.getLogger('mutator_main')

class FileSource(enum.Enum):
    Seed = 1
    OldBenignExamples = 2
    Mutations = 3
    PreDeterminedGenerated = 4
    OriginalCex = 5
    MustFulfill = 6    
            

def check(instr: int) -> bool:
    """Check if the instruction is valid by disassembling it with Capstone
    Args:
        instr (int): The instruction to check
    Returns:
        bool: True if the instruction is valid, False otherwise
    """
    md = Cs(CS_ARCH_RISCV, CS_MODE_RISCV32)
    instr_bytecode = None
    if type(instr) is int:
        instr_bytecode = instr.to_bytes(4, "little")
    else:
        instr_bytecode = instr
    try:
        disasm = md.disasm(instr_bytecode, 0x0)
        if len(list(disasm)) == 0:
            logger.debug(f"Instruction {instr} is not valid")
            return False
        for i in md.disasm(instr_bytecode, 0x0):
            logger.debug(f"Disassembled instruction: {i.mnemonic} {i.op_str}")
        return True
    except OverflowError as e:
        logger.critical(f"Can't represent {instr} as 4 bytes")
        raise e
    return False

def _random_mutation_worker(args):
    """
    Worker for random mutations. Runs in a separate process.
    """
    (code_bytes, mutation_steps, constants, interesting_instructions, 
     ignore_check, out_dir, save_idx) = args
    
    try:
        # Each process creates its own Mutator instance
        local_mut = mutator.Mutator(code_bytes, mutation_steps, constants, interesting_instructions, check_mutations_valid=not ignore_check)
        mutated_program = local_mut.mutate_random()
        
        out_file_path = os.path.join(out_dir, f"mutated_sequence_{save_idx}")

        if mutated_program:
            with open(out_file_path, "wb") as outf:
                outf.write(mutated_program.get_code())
            return {"path": out_file_path, "source": FileSource.Mutations}
        return None
    except Exception as e:
        logger.error(f"Random mutation worker for save_idx {save_idx} failed: {e}")
        return None

class InstructionMutatorWrapper:
    
    def __init__(self, in_file, out_dir, mutation_steps, mutation_number, log_level, seed=None, ignore_check=False, constants=None, interesting_instructions=None, max_program_length=20000):
        self.in_file = in_file
        self.out_dir = out_dir
        self.mutation_steps = mutation_steps
        self.mutation_number = mutation_number
        self.log_level = log_level
        self.seed = seed
        self.ignore_check = ignore_check
        self.compiler_script_path = "./scripts/assembly_and_write_testcase.sh"
        self.constants = constants or []
        self.interesting_instructions = interesting_instructions
        self.max_program_length = max_program_length


    def compile_and_write_testcase(self, source_code, out_file_path):
        with tempfile.NamedTemporaryFile(delete=False, suffix=".s") as tmp_assembly_file:
            tmp_assembly_file.write(source_code.encode('utf-8'))
            tmp_assembly_file.flush()

            result = subprocess.run(
                [f"{self.compiler_script_path} {tmp_assembly_file.name} {out_file_path}"],
                stderr=subprocess.PIPE,
                stdout=subprocess.PIPE,
                shell=True
            )
            os.unlink(tmp_assembly_file.name)

            if result.returncode != 0:
                logger.warning(f"Error compiling: {result.stderr.decode()}")
                return None
            return out_file_path
                

        
    def do_mutation(self, save_as_idx, mutation_list=None):
        #try:
#        logger.debug(f"Calling with choice: {choice}, idx: {save_as_idx}, mutate_field: {mutate_field}, arg_new_value, {arg_new_value}")
        if mutation_list is None:
            mutated_program = self.mut.mutate_random()
        else:
            mutated_program = self.mut.do_list_of_mutations(mutation_list)
        out_file_path = self.out_dir + "/" + f"mutated_sequence_{save_as_idx}"
        if mutated_program:
            logger.info(f"Generated mutated instruction sequence {save_as_idx}")
            #if self.ignore_check:
            #logger.debug("Ignoring check for valid instructions")
            #We do the check directly when we mutate
            with open(out_file_path, "wb") as outf:
                outf.write(mutated_program.get_code())
                    #for instr in :
                    #    outf.write(instr.to_bytes(4, "little"))
            #else:
                #if all([check(instr.instruction_bytes) for instr in mutated_program.instructions]):
                #    logger.debug(f"Step {save_as_idx}: all mutated instructions passed the check")
                #    with open(out_file_path, "wb") as outf:
                #        outf.write(mutated_program.get_code())
                        #for instr in mutated_sequence:
                        #    outf.write(instr.to_bytes(4, "little"))
                #else:
                #    logger.warning(f"Step {save_as_idx}: some mutated instructions failed the check; Skipping")
                #    return None
        else:
            logger.critical("Failed to generate mutated instruction sequence")
            raise Exception("Failed to generate mutated instruction sequence")
            return None #sys.exit(1)
        return out_file_path

    def do_deterministic_mutations(self, deterministic_mutations: list[typing.Tuple[int, str]], current_save_idx=0):
        seen_paths = set()
        result_list = []
        list_of_mutations = []
        logger.info(f"Program length {self.mut.program.get_length()}, deterministic mutations {deterministic_mutations}, interesting instructions {self.interesting_instructions}")
        for instr_idx in range(self.mut.program.get_length()):
            if instr_idx not in self.interesting_instructions:
                #print("Skipping instruction", instr_idx, "as it is not in interesting instructions", self.interesting_instructions)
                continue # Do not modify interesting instructions+
            logger.info(f"Doing deterministic mutations for instruction {instr_idx}: {deterministic_mutations}")
            for (num_times, mutation_choice) in deterministic_mutations:
                this_choice = (instr_idx, mutation_choice)
                # print("Doing deterministic mutation", this_choice, "num_times", num_times)
                if mutation_choice == "operands":
                    for mutate_field in ["rd", "rs1", "rs2"]:
                        for new_val in range(33):
                            list_of_mutations.append(((instr_idx, mutation_choice), mutate_field, new_val))
                            # i += 1
                            # out_file_path = self.do_mutation((instr_idx, mutation_choice), save_as_idx=i, mutate_field,new_val)
                            # new_item = {"path": out_file_path, "source": FileSource.Mutations}
                            # if new_item["path"] is None:
                            #     logger.warning(f"Mutation {i} failed")
                            #     continue
                            # result_list.append(new_item)
                            # if new_item["path"] in seen_paths:
                            #     raise Exception(f"Line 232 Duplicate path found in result_list: {new_item['path']}")
                            # seen_paths.add(new_item["path"])
                else:
                    for _ in range(num_times):
                        list_of_mutations.append((this_choice, None, None))
        #logger.debug(f"List of deterministic mutations: {list_of_mutations}")
        #Now: Do a self.mutation_steps cross prodcut of list_of_mutations 
        #First just list_of_mutations, then list_of_mutations x list_of_mutations and so on
        logger.info(f"Generated {len(list_of_mutations)} deterministic mutations")
        if len(list_of_mutations) == 0:
            raise Exception("No deterministic mutations generated, something is wrong")
        all_mutations = []
        for _ in range(500):
            length = random.randint(1, self.mutation_steps)
            all_mutations.append([random.choice(list_of_mutations) for _ in range(length)]) 
        # list_of_mutations = random.sample(list_of_mutations, min(len(list_of_mutations), 100))
        # all_mutations = []
        # for idx in range(1, self.mutation_steps):
        #     all_mutations += [list(x) for x in itertools.product(list_of_mutations, repeat=idx+1)]
        logger.info(f"All mutations {len(all_mutations)} interesting instructions {self.interesting_instructions} mutation steps {self.mutation_steps} deterministic mutations {deterministic_mutations}")
        # all_mutations = random.sample(all_mutations, self.mutation_number)
        # all_mutations += list_of_mutations # Add the original mutations as well
        # print("doing num", len(all_mutations), "mutations", len(list_of_mutations), "mutations per step and", self.mutation_steps, "steps")
        # exit(0)
        
        
        # Thread-safe counter and set
        counter_lock = threading.Lock()
        seen_paths_lock = threading.Lock()
        
        def process_mutation(mutation_list):
            nonlocal current_save_idx
            nonlocal seen_paths
            with counter_lock:
                current_save_idx += 1
                local_save_idx = current_save_idx
            
            logger.debug(f"Now doing mutation list {mutation_list}", "local save idx", local_save_idx)
            out_file_path = self.do_mutation(save_as_idx=local_save_idx, mutation_list=mutation_list)
            new_item = {"path": out_file_path, "source": FileSource.Mutations}
            
            if new_item["path"] is None:
                logger.info(f"Mutation {local_save_idx} failed")
                return None
            
            with seen_paths_lock:
                if new_item["path"] in seen_paths:
                    raise Exception(f"Line 236 Duplicate path found in result_list: {new_item['path']}")
                seen_paths.add(new_item["path"])
            
            return new_item
        # for mutation_list in tqdm.tqdm(all_mutations):
        #     new_item = process_mutation(mutation_list)
        #     if new_item is not None:
        #         result_list.append(new_item)
        # Use ThreadPoolExecutor to process mutations in parallel
        with concurrent.futures.ThreadPoolExecutor() as executor:
            futures = [executor.submit(process_mutation, mutation_list) for mutation_list in all_mutations]
            
            for future in tqdm.tqdm(concurrent.futures.as_completed(futures), total=len(futures), desc="Processing deterministic mutations"):
                try:
                    new_item = future.result()
                    if new_item is not None:
                        result_list.append(new_item)
                except Exception as exc:
                    logger.error(f"Mutation generated an exception: {exc}")
        return result_list, seen_paths, current_save_idx
               
    def _generate_load_immediate_jobs(self):
        jobs = []
        for constant in self.constants:
            for out_register in range(32):
                source_code = utils.generate_riscv_load_immediate_program(out_register, constant)
                out_file_path  =os.path.join(self.out_dir, f"load_immediate__{out_register}_{hex(constant)}_{str(uuid.uuid4())[:6]}.bin")
                jobs.append((source_code, out_file_path))
        return jobs

    def run(self):
        # Setup logging
        logging_common.setup_logging(self.log_level)
        # logging.disable(logging.CRITICAL)
        logger.info("Starting RISC-V instruction mutator")
        generated_program_paths = riscv_mutator.generate_mutations(
            input_path=self.in_file,
            output_dir=self.out_dir,
            num_mutations=self.mutation_number,
            mutations_per_sequence=self.mutation_steps,
            interesting_instructions=self.interesting_instructions,
            seed=42,
            num_workers=os.cpu_count(),
            max_program_length=self.max_program_length
        )
        result_list = []
        for res in generated_program_paths:
            result_list.append({"path": res, "source": FileSource.Mutations})
        return result_list
        logger.info(f"Reading instructions from {self.in_file}")
        result_list = []

        # Setup seed for RNG
        if self.seed:
            random.seed(self.seed)
            logger.debug(f"Setting seed for RNG to {self.seed}")
        else:   
            self.seed = time.time()
            random.seed(self.seed)
            logger.info(f"Setting seed for RNG to current time: {self.seed}")

        # Create the output directory if it doesn't exist
        if not os.path.exists(self.out_dir):
            os.makedirs(self.out_dir)

        # Read the instruction sequence
        with open(self.in_file, "rb") as f:
            code = f.read()

        logger.info(f"Instruction sequence is {len(code)//4} instructions long")

        #instructions = utils.get_instruction_dict_from_bytestring(code)
        #for instr in instructions.keys():
        #    if instructions[instr]["index"] in self.interesting_instructions:
        #        instructions[instr]["interesting"] = True
        #    else:
        #        instructions[instr]["interesting"] = False
            
        #print("instructions", instructions)
        logger.debug(f"Constants in commit log: {self.constants}")
        logger.info(f"Generating {len(self.constants)*32} load immediate programs")

        load_immediate_jobs = self._generate_load_immediate_jobs()
        with concurrent.futures.ProcessPoolExecutor() as executor:
            futures = [executor.submit(self.compile_and_write_testcase, source_code, out_file_path) for source_code, out_file_path in load_immediate_jobs]
            for future in tqdm.tqdm(concurrent.futures.as_completed(futures), total=len(futures)):
                path = future.result()
                if path is not None:
                    result_list.append({"path": path, "source": FileSource.PreDeterminedGenerated})

        # with tqdm.tqdm(total=len(self.constants)*32, desc="Load immediate generation progress") as pbar:
        #     for constant in self.constants:
        #         pbar.set_postfix({"constant": hex(constant)})
        #         with tqdm.tqdm(total=32, leave=False, desc=f"Generating load immediate for {hex(constant)}") as inner_pbar:
        #             for out_register in range(0,32):
        #                 source_code = utils.generate_riscv_load_immediate_program(out_register, constant)
        #                 tmp_bin_file_name = self.compile_and_write_testcase(source_code, file_prefix=f"load_immediate__{out_register}_{hex(constant)}_")
        #                 final_bin_file_path = os.path.join(self.out_dir, os.path.basename(tmp_bin_file_name))
        #                 shutil.move(tmp_bin_file_name, final_bin_file_path)
        #                 logger.debug(f"Vincent: Generated {final_bin_file_path}")
        #                 result_list.append({"path": final_bin_file_path, "source": FileSource.PreDeterminedGenerated})
                        
        #                 pbar.update(1)
        #                 inner_pbar.update(1)

        # Mutate the instruction sequence
        logging.info("Load immediate generation done, now starting mutator")
        self.mut = mutator.Mutator(code, self.mutation_steps, self.constants, self.interesting_instructions, check_mutations_valid=not self.ignore_check,
                                   max_program_length=self.max_program_length)
        self.mut._mutation_steps = self.mutation_steps
        current_save_idx = 0
        single_instruction_bug = False
        if self.interesting_instructions is not None and len(self.interesting_instructions) == 1:
            single_instruction_bug = True
        deterministic_mutations = [(10, "opcode"), (1,"operands")]
        if single_instruction_bug is True:
            deterministic_mutations = [(1,"operands")]
            self.mut.choices["opcode"] = 0.05
            self.mut.choices["delete_instruction"] = 0
            self.mut.choices["insert_instruction"] = 0
            self.mut.choices["operands"] += 1-sum(self.mut.choices.values())
            self.mut.choices["switch_instructions"] = 0

        logger.info(f"Doing mutations according to dict {self.mut.choices}")

        logging.disable(logging.CRITICAL)
        this_result_list, seen_paths, current_save_idx = self.do_deterministic_mutations(deterministic_mutations, current_save_idx)
        result_list += this_result_list
        logging.disable(logging.NOTSET)

        logger.info("#### Determinstic mutations done ###")

        with open(self.in_file, "rb") as f:
            code_bytes = f.read()

        logging.disable(logging.CRITICAL)
        for this_mutation_steps in range(1, self.mutation_steps+1):
            self.mut._mutation_steps = this_mutation_steps
            for j in tqdm.tqdm(range(self.mutation_number), desc=f"Mutating ({this_mutation_steps} steps)", total=self.mutation_number):
                #idx = (j+1)*this_mutation_steps+i
                current_save_idx += 1
                out_file_path = self.do_mutation(save_as_idx=current_save_idx, mutation_list=None)
                new_item = {"path": out_file_path, "source": FileSource.Mutations}
                if new_item["path"] is None:
                    logger.warning(f"Mutation {current_save_idx} failed")
                    continue
                result_list.append(new_item)
                if new_item["path"] in seen_paths:
                    raise Exception(f"Line 255 Duplicate path found in result_list: {new_item['path']}")
                seen_paths.add(new_item["path"])
        logging.disable(logging.NOTSET)

        logger.info(f"Generated a total of {len(result_list)} mutated instruction sequences")

        #print("Result list", result_list)
        # Check for duplicates in result_list
        seen_paths = set()
        for result in result_list:
            if result["path"] in seen_paths:
                raise Exception(f"Duplicate path found in result_list: {result['path']}")
            seen_paths.add(result["path"])

        logging.disable(logging.NOTSET)
        return result_list
        
               
        
    
