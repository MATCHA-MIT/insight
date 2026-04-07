import argparse
import os
import subprocess
import shutil
import json
import time
import constants as constants_module
import hashlib
import sys
from pathlib import Path
import threading
import multiprocessing
import analyzer as analyzer_module
import tempfile
import cffi
import copy
import enum
import logging
import Levenshtein
import tqdm
import common
import typing
import os

# class EnumEncoder(json.JSONEncoder):
#     def default(self, obj):
#         if isinstance(obj, enum.Enum):
#             return f"{obj.__class__.__name__}.{obj.name}"
#         return super().default(obj)

# Configure the root logger
logging.basicConfig(level=logging.DEBUG,
                    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
                    handlers=[logging.StreamHandler(sys.stdout)])

logger = logging.getLogger("cex-generator")


subdir = Path(__file__).parent / "formal-verif" / "invariant_generation" / "vincent_invariant_generator"
sys.path.append(str(subdir))

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))


subdir = Path(__file__).parent.parent / "mutation_engine" 
sys.path.insert(0, str(subdir))

from riscv_instruction_mutator import InstructionMutatorWrapper, utils, program, FileSource
from vcd_trace import vcdTrace

SODOR_COMMIT_LOG_MARKER = "Sodor commit log"
DUT_COMMIT_LOG_MARKER = "DUT commit log"


def edit_distance_with_move(bytestring1, bytestring2):
    # Convert bytestrings to lists for easier manipulation
    seq1 = list(bytestring1)
    seq2 = list(bytestring2)

    # Create a distance matrix
    len1, len2 = len(seq1), len(seq2)
    distance = [[0] * (len2 + 1) for _ in range(len1 + 1)]

    # Initialize the distance matrix
    for i in range(len1 + 1):
        distance[i][0] = i
    for j in range(len2 + 1):
        distance[0][j] = j

    # Fill the distance matrix
    for i in range(1, len1 + 1):
        for j in range(1, len2 + 1):
            cost = 0 if seq1[i - 1] == seq2[j - 1] else 1
            distance[i][j] = min(
                distance[i - 1][j] + 1,      # Deletion
                distance[i][j - 1] + 1,      # Insertion
                distance[i - 1][j - 1] + cost  # Substitution
            )

            # Check for possible move operations
            if i > 1 and j > 1 and seq1[i - 1] == seq2[j - 2] and seq1[i - 2] == seq2[j - 1]:
                distance[i][j] = min(
                    distance[i][j],
                    distance[i - 2][j - 2] + cost  # Move
                )

    return distance[len1][len2]

def calculate_program_distance(program1, program2):
    """
    Calculate the distance between two programs using Edit Distance With Move.
    """
    return edit_distance_with_move(program1, program2)
    # Convert the programs to strings
    #program1_str = program1.decode("utf-8", errors="ignore")
    #program2_str = program2.decode("utf-8", errors="ignore")
    # Calculate the Levenshtein distance
    #distance = Levenshtein.distance(program1_str, program2_str)
    #return distance

def disassemble_and_extract(file_path):
    # Call the shell script and capture the output
    result = subprocess.run(['./util_scripts/disassemble_objdump.sh', file_path], capture_output=True, text=True)

    # Check if the command was successful
    if result.returncode != 0:
        logger.warning("Error in disassembling the file.")
        return

    # Split the output into lines
    lines = result.stdout.splitlines()

    # Find the start of the .data section
    extracting = False
    data_section = []

    for line in lines:
        if line.strip() == "0000000000000000 <.data>:":
            extracting = True
            continue
        if extracting:
            # Stop if we reach an empty line or another section
            if not line.strip() or line.strip().endswith('>:'):
                break
            data_section.append(line.strip().replace("\t"," "))

    return data_section

def file_fingerprint(path: str, chunk_size: int = 1024 * 1024) -> bytes:
    """Return a stable, small fingerprint for file content without loading the whole file."""
    hasher = hashlib.sha256()
    with open(path, "rb") as f:
        while True:
            chunk = f.read(chunk_size)
            if not chunk:
                break
            hasher.update(chunk)
    return hasher.digest()


def substitute_with_nop(instr_dict: dict, out_dir: str, instr: int):
    new_sequence = copy.deepcopy(instr_dict)
    new_sequence[0x13] = new_sequence.pop(instr)
    new_sequence[0x13]["mnemonics"] = ["nop"]
    out_file = f"nop_substitution_{instr}"
    with open(os.path.join(out_dir, out_file), "wb") as outf:
        for instr in sorted(new_sequence.keys(), key=lambda x: new_sequence[x]["index"]):
            outf.write(instr.to_bytes(4, "little"))

    return out_file



def check_cex_invariant(cex, invariants_path):
    return cex, analyzer_module.waveform_fullfills_any_invariant(cex[constants_module.WAVEFORM_PATH_KEY], invariants_path)

def parse_args():
    parser = argparse.ArgumentParser(description="Generate and verify Counterexamples from a given instruction sequence.")
    parser.add_argument("input_file", type=str, help="Path to the input file.")
    #parser.add_argument("waveform_dir", type=str, help="Path to the waveform directory.")
    #parser.add_argument("mutated_files_dir", type=str, help="Path to the directory containing mutated files.")
    parser.add_argument("output_dir", type=str, help="Path to the output directory.", default=None)
    #parser.add_argument("checker_path", type=str, help="Path to the checker executable.")
    parser.add_argument("--log-level", "-l", help="Logging level: one of {DEBUG, INFO, CRITICAL, WARNING, FATAL, ERROR}", default="INFO")
    parser.add_argument("--config-path", "-c", help="Path to the configuration file.")
    parser.add_argument("--test_allowed_signals", type=str, help="Path to the file containing allowed signals for JG. If not set, we all all signals to be used.", default=None)

    return parser.parse_args()

class ClassifyMutations:
    def __init__(self, checker_path, waveform_dir, output_dir, mutated_files_dir, invariants_path, memory_offset=32*4):
        self.output_sets = {
                            "cex": [],      # Counterexamples
                            "bex": [],      # Benign examples
                            "invalid": []   # Invalid examples
                        }
        self.filtered_benign = 0
        self.benign_from_mutations = 0
        self.filtered_cex = 0
        self.invalid_invariants = set()
        self.checker_path = checker_path
        self.waveform_dir = waveform_dir
        self.output_dir = output_dir
        self.mutated_files_dir = mutated_files_dir
        self.original_bug_result = None
        self.original_bug_type = None
        self.invariants_path = invariants_path
        self.already_seen_programs = set()
        self.memory_offset = memory_offset
        self.seed_directories = [constants_module.SEEDS_DIR]
        os.makedirs(os.path.join(self.output_dir, constants_module.CEX_PATH, constants_module.BINARY_SUBDIR), exist_ok=True)
        os.makedirs(os.path.join(self.output_dir, constants_module.CEX_PATH, constants_module.WAVEFORM_SUBDIR), exist_ok=True)
        os.makedirs(os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.BINARY_SUBDIR), exist_ok=True)
        os.makedirs(os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.WAVEFORM_SUBDIR), exist_ok=True)

        self._pool_ctx = multiprocessing.get_context("fork")
        self._pool_processes = max(1, (os.cpu_count() or 1) - 2)
        self._pool_maxtasksperchild = 200
        self._pool = None

                
    def process_file_wrapper(self, current_dir, mutated_file, file_source=None):
        try:
            mutated_file_path = os.path.join(current_dir, mutated_file)
            return run_file(
                mutated_file,
                mutated_file_path,
                current_dir,
                checker_path=self.checker_path,
                waveform_dir=self.waveform_dir,
                output_dir=self.output_dir,
                invariants_path=self.invariants_path,
                seed_directories=self.seed_directories,
                original_bug_type=self.original_bug_type,
                memory_offset=self.memory_offset,
                file_source_arg=file_source,
            )
        except Exception as e:
            logger.warning(f"Exception when running process file wrapper {e}")
            raise e

    def _build_worker_state(self):
        """Collect the minimal amount of state that workers need to classify files."""
        return {
            "checker_path": self.checker_path,
            "waveform_dir": self.waveform_dir,
            "output_dir": self.output_dir,
            "mutated_files_dir": self.mutated_files_dir,
            "invariants_path": self.invariants_path,
            "memory_offset": self.memory_offset,
            "seed_directories": tuple(self.seed_directories),
            "original_bug_type": self.original_bug_type,
        }
    
    def _get_or_create_pool(self):
        if self._pool is None:
            self._pool = self._pool_ctx.Pool(
                processes=self._pool_processes,
                maxtasksperchild=self._pool_maxtasksperchild,
            )
        return self._pool

    def close_pool(self):
        if self._pool is not None:
            self._pool.close()
            self._pool.join()
            self._pool = None

    def multithreaded_run_files(self, original_bug_result: analyzer_module.AnalyzerResult, result_list=None, first_time=False):
        self.output_sets = {
                            "cex": [],      # Counterexamples
                            "bex": [],      # Benign examples
                            "invalid": []   # Invalid examples
        }
        os.makedirs(os.path.join(self.output_dir, constants_module.CEX_PATH, constants_module.BINARY_SUBDIR), exist_ok=True)
        os.makedirs(os.path.join(self.output_dir, constants_module.CEX_PATH, constants_module.WAVEFORM_SUBDIR), exist_ok=True)
        os.makedirs(os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.BINARY_SUBDIR), exist_ok=True)
        os.makedirs(os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.WAVEFORM_SUBDIR), exist_ok=True)
        benign_examples_full_path = os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.BINARY_SUBDIR)
        #List from mutations directly
        self.original_bug_result = original_bug_result
        self.original_bug_type = getattr(original_bug_result, "bug_type", None)
        self.output_sets["cex"] = []
        if first_time == False:
            #We already known that everyin benign examples directory is benign, so we don't need to run them again
            directories = self.seed_directories
        else:
            directories = self.seed_directories
        if result_list is None:
            directories = [self.mutated_files_dir]+directories
        #directories = [self.mutated_files_dir]
        #print("Trying ", self.output_dir)
        #exit(0)
        #ran_file = []
        files_to_run = []
        already_seen_files = set()
        if result_list is not None:
            for item in result_list:
                mutated_file = item["path"]
                file_source = item["source"]
                current_dir = self.mutated_files_dir
                if mutated_file.endswith(".vcd") or mutated_file.endswith(".fst"):
                    continue
                file_sig = file_fingerprint(mutated_file)
                if file_sig in already_seen_files:
                    continue
                else:
                    mutated_file = os.path.basename(mutated_file)
                    files_to_run.append((current_dir, mutated_file, file_source))
                    already_seen_files.add(file_sig)
        for current_dir in directories:
            for mutated_file in os.listdir(current_dir):
                mutated_file_path = os.path.join(current_dir, mutated_file)
                if mutated_file.endswith(".vcd")  or mutated_file.endswith(".fst"):
                    continue
                if not os.path.isfile(mutated_file_path):
                    continue
                file_sig = file_fingerprint(mutated_file_path)
                if file_sig in already_seen_files:
                    continue
                else:
                    files_to_run.append((current_dir, mutated_file, None))
                    already_seen_files.add(file_sig)
        logger.info(f"Now launching multithreaded classification process on {len(files_to_run)} files")
        seen = set()
        duplicates = []
        for current_dir, mutated_file, file_source in files_to_run:
            file_tuple = (current_dir, mutated_file)
            if file_tuple in seen:
                duplicates.append(file_tuple)
            else:
                seen.add(file_tuple)
            # logger.info(f"Adding {current_dir}/{mutated_file}")
        if duplicates:
            pass
            # logger.info(f"Duplicate files found: {duplicates}")
        else:
            pass
            # logger.info("No duplicate files found.")
        #return
        total_files = len(files_to_run)
        if total_files == 0:
            logger.info("No files to classify; skipping process pool execution.")
            return

        worker_payload = self._build_worker_state()
        chunksize = max(1, total_files // (self._pool_processes * 4))
        iterator = (
            (current_dir, mutated_file, file_source, worker_payload)
            for (current_dir, mutated_file, file_source) in files_to_run
        )
        logger.info(
            "Submitting files to process pool for classification (workers=%s, chunksize=%s)",
            self._pool_processes,
            chunksize,
        )
        pool = self._get_or_create_pool()
        worker_results = pool.imap_unordered(classification_worker_process, iterator, chunksize)
        #Serializing to avoid issues with fork and logging
        #worker_results = map(classification_worker_process, iterator)
        for result in tqdm.tqdm(worker_results, total=total_files, desc="Processing files for classification"):
            # try:
            #     result = result
            # except Exception as e:
            #     logger.warning(f"Got Exception from worker process: {e}")
            #     raise e
            if result is None:
                continue
            kind, item = result
            if kind == analyzer_module.CheckerResultKind.CEX:
                self.output_sets["cex"].append(item)
            elif kind == analyzer_module.CheckerResultKind.BENIGN:
                self.output_sets["bex"].append(item)
                if item["file_source"] == FileSource.Mutations:
                    self.benign_from_mutations += 1
            elif kind == analyzer_module.CheckerResultKind.INVALID:
                self.output_sets["invalid"].append(item)
            
        logger.info("Now checking cex for invariants")
    
        to_keep = []# common.identify_non_covered_cex_items_through_ffi_call(self.output_sets["cex"], self.invariants_path)
        #TODO: Do we need to filter out BEX here?
        #Yes, to filter out invalid instructions for example.
        for set_keys in ["cex", "bex"]: #, "bex"]: #"cex"
            #continue
            to_keep = common.identify_non_covered_cex_items_through_ffi_call(self.output_sets[set_keys], self.invariants_path,look_at_invariants_only=self.seed_invariants)
            keep_keys = {
                (
                    item["file"],
                    item[constants_module.WAVEFORM_PATH_KEY],
                    item["file_source"],
                )
                for item in to_keep
            }
            rebuilt_items = []
            for cex_item in self.output_sets[set_keys]:
                if cex_item["file_source"] == FileSource.Seed and set_keys == "bex":
                    rebuilt_items.append(cex_item)  # Do not filter out seeds
                    continue

                item_key = (
                    cex_item["file"],
                    cex_item[constants_module.WAVEFORM_PATH_KEY],
                    cex_item["file_source"],
                )
                if item_key in keep_keys:
                    rebuilt_items.append(cex_item)
                    continue

                if cex_item["file_source"] == FileSource.OriginalCex: #actually, this is never processed here, so this check is useless
                    invariant = analyzer_module.waveform_fullfills_any_invariant(cex_item[constants_module.WAVEFORM_PATH_KEY], self.invariants_path)
                    raise Exception(f"We would filter out the original cex, as it fulfills invariant {invariant}, original cex {cex_item}")
                self.filtered_cex += 1
                os.remove(cex_item["file"])
                os.remove(cex_item[constants_module.WAVEFORM_PATH_KEY])
            self.output_sets[set_keys] = rebuilt_items

    def __del__(self):
        self.close_pool()

def run_and_check_commit_log(checker_path, mutated_file_path, waveform_output_path):
    return analyzer_module.check_commit_log(checker_path, mutated_file_path, waveform_output_path)


def run_file(
    mutated_file,
    mutated_file_path,
    current_dir,
    checker_path,
    waveform_dir,
    output_dir,
    invariants_path,
    seed_directories,
    original_bug_type,
    memory_offset,
    file_source_arg=None,
):
    waveform_output_path = os.path.join(waveform_dir, mutated_file + ".fst")
    with open(mutated_file_path, "rb") as f:
        file_content = f.read()

    benign_binary_dir = os.path.join(output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.BINARY_SUBDIR)
    benign_waveform_dir = os.path.join(output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.WAVEFORM_SUBDIR)
    cex_binary_dir = os.path.join(output_dir, constants_module.CEX_PATH, constants_module.BINARY_SUBDIR)
    cex_waveform_dir = os.path.join(output_dir, constants_module.CEX_PATH, constants_module.WAVEFORM_SUBDIR)

    if file_source_arg is not None:
        file_source = file_source_arg
    elif current_dir in seed_directories:
        file_source = FileSource.Seed
    elif current_dir == benign_binary_dir:
        file_source = FileSource.OldBenignExamples
    else:
        file_source = FileSource.Mutations
    #
    # get the current time
    #start = time.time()
    checker_result = run_and_check_commit_log(checker_path, mutated_file_path, waveform_output_path)
    print(f"Classifying file {mutated_file_path} from source {file_source.name} result {checker_result.Kind.name}")
    #end = time.time()
    #print(f"Checker time: {end - start} seconds for {mutated_file_path}")
    if checker_result is None:
        logger.debug(f"Checker returned None for {mutated_file_path}, possibly due to an error.")
        return (analyzer_module.CheckerResultKind.INVALID, mutated_file)
    #print(f"Checker result: {checker_result.Kind.name} for {mutated_file_path}")
    if checker_result.Kind == analyzer_module.CheckerResultKind.INVALID:
        logger.debug(f"Ignoring {mutated_file} as invalid")
        return (analyzer_module.CheckerResultKind.INVALID, mutated_file)

    benign = checker_result.Kind == analyzer_module.CheckerResultKind.BENIGN
    program_distance = 0

    if not benign:
        if file_source == FileSource.OldBenignExamples and not (
            mutated_file_path.endswith("null.fst")
            or mutated_file_path.endswith("null.vcd")
            or mutated_file_path.endswith("null.bin")
        ):
            raise Exception(f"{mutated_file_path} was stored in the benign folder but is actually a cex?")
        if mutated_file_path.endswith("null.vcd") or mutated_file_path.endswith("null.fst") or mutated_file_path.endswith("null.bin"):
            logger.debug(f"Ignoring {mutated_file_path} as it is a null file")
            return None
        if file_source == FileSource.Seed:
            logger.debug(f"Ignoring cex seed {mutated_file_path}")
            return None
        if checker_result.Kind == analyzer_module.CheckerResultKind.FILTERED:
            logger.debug(f"Ignoring Cex waveform {waveform_output_path}, as it fulfills invariant")
            return None
        if checker_result.Kind == analyzer_module.CheckerResultKind.CEX:
            if constants_module.IGNORE_DIFFERING_BUG_TYPE is False and original_bug_type is not None:
                analyzer = analyzer_module.BugAnalyzer(checker_path, mutated_file_path, invariants_path, memory_offset)
                this_analyzer_result = analyzer.analyse(minimize=False)
                if this_analyzer_result.bug_type != original_bug_type:
                    logger.debug(f"We have a change in bug type. Most likely we generated a different CEX. Ignoring! {mutated_file_path}")
                    return
            new_file_path = os.path.join(cex_binary_dir, os.path.basename(mutated_file_path))
            shutil.move(mutated_file_path, new_file_path)
            waveform_output_path_new = os.path.join(cex_waveform_dir, os.path.basename(waveform_output_path))
            shutil.move(waveform_output_path, waveform_output_path_new)
            return (
                analyzer_module.CheckerResultKind.CEX,
                {
                    "file": new_file_path,
                    constants_module.WAVEFORM_PATH_KEY: waveform_output_path_new,
                    "file_source": file_source,
                    "program_distance": program_distance,
                },
            )
    else:
        if current_dir == benign_binary_dir:
            file_path = mutated_file_path
            waveform_output_path_new = os.path.join(benign_waveform_dir, os.path.basename(waveform_output_path))
            shutil.move(waveform_output_path, waveform_output_path_new)
            return (
                analyzer_module.CheckerResultKind.BENIGN,
                {
                    "file": file_path,
                    constants_module.WAVEFORM_PATH_KEY: waveform_output_path_new,
                    "file_source": file_source,
                    "program_distance": program_distance,
                },
            )
        if current_dir == constants_module.SEEDS_DIR:
            new_file_path = os.path.join(benign_binary_dir, mutated_file)
            shutil.copy(mutated_file_path, new_file_path)
            waveform_output_path_new = os.path.join(benign_waveform_dir, os.path.basename(waveform_output_path))
            shutil.move(waveform_output_path, waveform_output_path_new)
            new_waveform_path = waveform_output_path_new
        else:
            hash_object = hashlib.md5(file_content)
            new_file_path = os.path.join(
                benign_binary_dir,
                "new_" + os.path.basename(mutated_file_path) + "_" + hash_object.hexdigest()[:5] + ".bin",
            )
            shutil.move(mutated_file_path, new_file_path)
            new_waveform_path = os.path.join(benign_waveform_dir, os.path.basename(new_file_path) + ".fst")
            shutil.move(waveform_output_path, new_waveform_path)
        return (
            analyzer_module.CheckerResultKind.BENIGN,
            {
                "file": new_file_path,
                constants_module.WAVEFORM_PATH_KEY: new_waveform_path,
                "file_source": file_source,
                "program_distance": program_distance,
            },
        )

# _classification_worker_state: typing.Optional[dict] = None


# def _classification_worker_initializer(state_payload: dict):
#     """Store shared worker state instead of re-instantiating ClassifyMutations."""
#     global _classification_worker_state
#     _classification_worker_state = state_payload


def classification_worker_process(args):
    # if _classification_worker_state is None:
    #     raise RuntimeError("Classification worker was not initialized")
    current_dir, mutated_file, file_source, classification_worker_state = args
    mutated_file_path = os.path.join(current_dir, mutated_file)
    try:
        return run_file(
            mutated_file,
            mutated_file_path,
            current_dir,
            checker_path=classification_worker_state["checker_path"],
            waveform_dir=classification_worker_state["waveform_dir"],
            output_dir=classification_worker_state["output_dir"],
            invariants_path=classification_worker_state["invariants_path"],
            seed_directories=classification_worker_state["seed_directories"],
            original_bug_type=classification_worker_state.get("original_bug_type"),
            memory_offset=classification_worker_state["memory_offset"],
            file_source_arg=file_source,
        )
    except Exception as e:
        logger.warning(f"Exception when running process file wrapper {e}")
        raise e

class CexGenerator():
    def __init__(self, output_dir, checker_path,invariants_path=None, log_level="INFO", first_symbolic_instruction_idx=0, start_address=0x80000000,
                 max_program_length=constants_module.MAX_CEX_LENGTH, additional_seed_dirs: typing.Optional[typing.List[str]]=None):
        self.output_dir = output_dir
        self.checker_path = checker_path
        self.waveform_dir = os.path.join(self.output_dir, "waveforms")
        self.mutated_files_dir = os.path.join(self.output_dir, "mutations")
        self.log_level = log_level
        self.first_symbolic_instruction_idx = first_symbolic_instruction_idx
        self.start_address = start_address
        print("start address cex generator", hex(self.start_address))
        self.max_program_length = max_program_length
        if invariants_path is None:
            self.invariants_path = os.path.join(self.output_dir,constants_module.INVARIANT_PATH)
        else:
            self.invariants_path = invariants_path
        # Set the logging level (e.g., DEBUG, INFO, WARNING, ERROR, CRITICAL)
        # logger.setLevel(logging.DEBUG)

        # # Create a handler that outputs to stdout
        # handler = logging.StreamHandler(sys.stdout)

        # # Set the logging level for the handler
        # handler.setLevel(logging.DEBUG)

        # # Create a formatter and set it for the handler
        # formatter = logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')
        # handler.setFormatter(formatter)

        # # Add the handler to the logger
        # logger.addHandler(handler)
        logger.info("Starting cex generator")
        # Create the output directory if it doesn't exist
        if not os.path.exists(output_dir):
            os.makedirs(output_dir)
        # Create the waveform directory if it doesn't exist
        if not os.path.exists(self.waveform_dir):
            os.makedirs(self.waveform_dir)
        # Create the mutated files directory if it doesn't exists
        if not os.path.exists(self.mutated_files_dir):
            os.makedirs(self.mutated_files_dir)
        # Check that the checker path if it doesn't exist
        if not os.path.exists(checker_path):
            raise FileNotFoundError(f"Checker path {checker_path} does not exist.")
        #The first time we call run_cex_generator, we also classify the benign examples. We don't do that after.
        self.first_time_cex_generator_called = True
        self.freeze_instructions = []
        self.classifier =  ClassifyMutations(checker_path=self.checker_path, waveform_dir=self.waveform_dir,output_dir=self.output_dir, mutated_files_dir=self.mutated_files_dir,invariants_path=self.invariants_path, memory_offset=self.first_symbolic_instruction_idx)
        if additional_seed_dirs is not None:
            self.classifier.seed_directories += additional_seed_dirs
        
    def set_seed_invariants(self, seed_invariants: typing.List[str]):
        self.seed_invariants = seed_invariants
        self.classifier.seed_invariants = seed_invariants
        
    def set_freeze_instructions(self, freeze_instructions: typing.List[int]):
        self.freeze_instructions = freeze_instructions
        
    def run_cex_generator(self, input_file, minimize=True):
        logger.info(f"Running cex generator on {input_file} with minimize={minimize}")
        print(f"Running cex generator on {input_file} with minimize={minimize}")
        if not os.path.exists(self.output_dir):
            os.makedirs(self.output_dir)
        bug_analyzer = analyzer_module.BugAnalyzer(self.checker_path, input_file, self.invariants_path, self.first_symbolic_instruction_idx, start_address=self.start_address)
        bug_analyzer.set_freeze_instructions(self.freeze_instructions)
        analyse_result, all_benign_examples = bug_analyzer.analyse(minimize, check_invariant_satisfaction=True, return_nopped_benign_examples=True)
        #print("analyse result", type(analyse_result))
        bug_type = analyse_result.bug_type
        minimized_example = analyse_result.minimized_example
        interesting_instrs = analyse_result.interesting_instructions
        kind = analyse_result.kind
        constants = analyse_result.constants
        #minimized_example, bug_type, interesting_instrs, minimized_example_res_check
        minimized_example_path = os.path.join(self.output_dir, "minimized_cex.bin")
        with open(minimized_example_path, "wb") as fp:
            fp.write(minimized_example)
        input_file_path = os.path.join(self.output_dir, "input_cex.bin")
        shutil.copy(input_file, input_file_path)
        wrapper = InstructionMutatorWrapper(minimized_example_path, self.mutated_files_dir, mutation_steps=constants_module.MAX_MUTATION_STEPS, mutation_number=constants_module.MUTATION_PER_STEP, log_level=self.log_level, ignore_check=True, constants=analyse_result.constants, interesting_instructions=interesting_instrs,
                                            max_program_length=self.max_program_length+1)
        #with tempfile.NamedTemporaryFile(delete=True) as tmp:
            #print("Checking input file", input_file)
        #    res =mutation_classifer.run_and_check_commit_log(minimized_example, tmp.name)
        #    if res.Kind != check_commit_log.CheckerResultKind.CEX:
        #        raise Exception(f"Input file is not a counterexample, exiting {res}")
                #return
        logger.info("Mutating instructions to new programs")
        result_list = wrapper.run()
        sys.stdout.flush()
        logger.info("Multithreaded classification")
        sys.stdout.flush()
        print("Result list len", len(result_list))
        self.classifier.multithreaded_run_files(original_bug_result=analyse_result, result_list=result_list, first_time=self.first_time_cex_generator_called)
        sys.stdout.flush()
        logger.info("Classification done")
        sys.stdout.flush()
        
        
        logger.info(f"{self.classifier.filtered_cex} counterexample(s) filtered out for satisfying an invariant.")
        if self.classifier.filtered_benign > 0:
            logger.info(f"{self.classifier.filtered_benign} benign example(s) filtered out for satisfying an invariant.")
            logger.debug("Invalid invariants:")
            for inv in self.classifier.invalid_invariants:
                logger.debug(f"-\t\"{inv}\"")
        else:
            logger.info(f"No, i.e., {self.classifier.filtered_benign},  benign example(s) filtered out for satisfying an invariant.")
        logger.info(f"Found {len(self.classifier.output_sets['cex'])} counterexamples")
        logger.info(f"Found {len(self.classifier.output_sets['bex'])} benign examples")
        logger.info(f"Found {self.classifier.benign_from_mutations} benign examples from mutations")
        logger.info(f"Found {len(self.classifier.output_sets['invalid'])} invalid examples")
        out_dict = self.classifier.output_sets
        out_dict["input_cex"] = {"path": input_file_path, "instructions": disassemble_and_extract(input_file)}
        minimized_example_waveform_path = os.path.join(self.output_dir, constants_module.CEX_PATH, constants_module.WAVEFORM_SUBDIR, f"minimized_cex{constants_module.WAVEFORM_FILE_SUFFIX}")
        minimized_example_binary_dir = os.path.join(self.output_dir, constants_module.CEX_PATH, constants_module.BINARY_SUBDIR, "minimized_cex.bin")
        input_file_example_binary_dir = os.path.join(self.output_dir, constants_module.CEX_PATH, constants_module.BINARY_SUBDIR, "input_cex.bin")
        input_file_example_waveform_path = os.path.join(self.output_dir, constants_module.CEX_PATH, constants_module.WAVEFORM_SUBDIR, f"input_cex{constants_module.WAVEFORM_FILE_SUFFIX}")
        shutil.copy(minimized_example_path, minimized_example_binary_dir)
        shutil.copy(input_file_path, input_file_example_binary_dir)
        res = analyzer_module.check_commit_log(self.checker_path, minimized_example_path, minimized_example_waveform_path)
        res2 = analyzer_module.check_commit_log(self.checker_path, input_file_path, input_file_example_waveform_path)
        out_dict["minimized_cex"] = {"path": minimized_example_path, constants_module.WAVEFORM_PATH_KEY: minimized_example_waveform_path} # "inst_number": res.difference_location.instruction_number, "ref_cycle": res.difference_location.refcore_cycle, "dut_cycle": res.difference_location.dut_cycle}
        out_dict["minimized_cex"]["instructions"] =  disassemble_and_extract(minimized_example_path)
        out_dict["bug_type"] = bug_type
        original_cex_item_minimized =             {"file": minimized_example_path,
             constants_module.WAVEFORM_PATH_KEY: minimized_example_waveform_path,
             "file_source": FileSource.OriginalCex,
             "program_distance": 0,
            }
        original_cex_item =            {"file": input_file_path,
             constants_module.WAVEFORM_PATH_KEY: input_file_example_waveform_path,
             "file_source": FileSource.OriginalCex,
             "program_distance": 0,
            }
        res_cex, res_invariant =  check_cex_invariant(original_cex_item_minimized, self.invariants_path)
        #if res_invariant is not None:
        #    raise Exception(f"We would filter out the original cex, as it fulfills invariant {res_invariant}, original cex {original_cex_item_minimized}")
        out_dict["cex"].append(
            original_cex_item_minimized
        )
        out_dict["cex"].append(
            original_cex_item
        )
        res_cex_non_minimized, res_invariant_non_minimized =  check_cex_invariant(original_cex_item, self.invariants_path)
        if res_invariant_non_minimized is not None:
            raise Exception(f"We would filter out the original cex, as it fulfills invariant {res_invariant_non_minimized}, original cex {original_cex_item}")
        
        # Process benign examples generated during minimization
        logger.info(f"Processing {len(all_benign_examples)} benign examples from minimization")
        for idx, bex_bytes in enumerate(all_benign_examples):
            benign_output_path = os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.BINARY_SUBDIR, f"benign_example_from_nop_{idx}_{hashlib.md5(bex_bytes).hexdigest()[:5]}.bin")
            benign_output_path_waveform = os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.WAVEFORM_SUBDIR, f"benign_example_from_nop_{idx}_{hashlib.md5(bex_bytes).hexdigest()[:5]}{constants_module.WAVEFORM_FILE_SUFFIX}")
            with open(benign_output_path, "wb") as fp:
                fp.write(bex_bytes)
            res_bex = analyzer_module.check_commit_log(self.checker_path, benign_output_path, benign_output_path_waveform)
            if res_bex.Kind != analyzer_module.CheckerResultKind.BENIGN:
                logger.warning(f"Benign example from nop is not benign. It's actually {benign_output_path} {res_bex}")
            else:
                bex_entry = {
                    "file": benign_output_path,
                    constants_module.WAVEFORM_PATH_KEY: benign_output_path_waveform,
                    "file_source": FileSource.Mutations,
                    "program_distance": 0,
                }
                out_dict["bex"].append(bex_entry)
            
        bex_entry = self.create_null_binary_and_waveform()
        out_dict["bex"].append(bex_entry)
        # print("out_dict", out_dict, flush=True)
        #with open("output_sets.json", "w") as fp:
        #    json.dump(out_dict, fp, default=str)
        self.first_time_cex_generator_called = False
        return out_dict
        
    def create_null_binary_and_waveform(self):
        #We need this because JG reset analysis is broken..
        null_binary_path = os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.BINARY_SUBDIR, "null.bin")
        null_waveform_path = os.path.join(self.output_dir, constants_module.BENIGN_EXAMPLES_PATH, constants_module.WAVEFORM_SUBDIR, "null.fst")
        Path(null_binary_path).touch()
        #with open(null_binary_path, "wb") as fp:
            #fp.write(b"\x00" * 4)
        out = analyzer_module.check_commit_log(self.checker_path, null_binary_path, null_waveform_path)
        if out.Kind != analyzer_module.CheckerResultKind.BENIGN:
            raise Exception("Null binary is not benign")
        bex_entry = {
            "path": null_binary_path,
            constants_module.WAVEFORM_PATH_KEY: null_waveform_path,
            "file_source": FileSource.MustFulfill
        }
        return bex_entry
        
        
        
    

def main():
    args = parse_args()
    input_file = args.input_file
    #mutated_files_dir = args.mutated_files_dir
    #output_dir = args.output_dir
    #checker_path = args.checker_path
    #waveform_dir = args.waveform_dir
    mutator_log_level = args.log_level
    # Wrap the main part of your script with profiling
    config_path = args.config_path
    with open(config_path, "r") as f:
        config = json.load(f)
    if "verilator_script" in config:
        checker_path = config["verilator_script"]
    else:
        raise Exception("No verilator script in config file")
    output_dir = args.output_dir
    if not output_dir and "output_dir" in config:
        output_dir = config["output_dir"]
    elif not output_dir:
        raise Exception("No output dir in config file")
    if "regex_config_path" in config:
        core_config_path = config["regex_config_path"]
    else:
        raise Exception("No regex config path in config file")
    with open(core_config_path, "r") as f:
        core_config = json.load(f)
    #waveform_dir = os.path.join(output_dir, "waveforms")
    #mutated_files_dir = os.path.join(output_dir, "mutations")
    invariants_path = os.path.join(output_dir, constants_module.INVARIANT_PATH)
    if "start_address" in core_config:
        start_address = int(core_config["start_address"], 16)
    else:
        start_address = 0x80000000
    print("Start address", hex(start_address))
    print("Core config", core_config)
    if "first_symbolic_instruction_idx" in core_config:
        first_symbolic_instruction_idx = core_config["first_symbolic_instruction_idx"]
    else:
        first_symbolic_instruction_idx = 0
    with open(input_file, "rb") as f:
        input_bytes = f.read()
        #num_instructions in input_bytes
        num_instructions = len(input_bytes) // 4
    seed_invariants = config.get("seed_invariants", [])
    cex_generator_instance = CexGenerator(output_dir, checker_path, invariants_path, log_level=mutator_log_level, first_symbolic_instruction_idx=first_symbolic_instruction_idx, start_address=start_address, max_program_length=num_instructions)
    cex_generator_instance.set_seed_invariants(seed_invariants)
    #cex_generator_instance.freeze_instructions = list(range(0,39))
    # cex_generator_instance.first_time_cex_generator_called = False
    ret_dict = cex_generator_instance.run_cex_generator(input_file, minimize=True)
    allowed_signals_path = args.test_allowed_signals
    if allowed_signals_path is not None:
        with open(allowed_signals_path, "r") as f:
            allowed_signals = f.read().splitlines()
        ret_dict["allowed_signals"] = allowed_signals
    conditional_signals_mapping_path = config.get("conditional_signals_mapping_path", None)
    if conditional_signals_mapping_path is not None:
        with open(conditional_signals_mapping_path, "r") as f:
            cond_mapping = json.load(f)
            ret_dict["conditional_signals_to_condition_mapping"] = cond_mapping

    with open(os.path.join(output_dir, "output_sets.json"), "w") as f:
        json.dump(ret_dict, f, default=str, indent=4)
    print("Wrote output_sets.json")
    #profiler = cProfile.Profile()
    #profiler.enable()
    #run_cex_generator(input_file, mutated_files_dir, output_dir, checker_path, waveform_dir, mutator_log_level)
    #profiler.disable()
    #profiler.print_stats(sort='cumulative')

    #for constant_name, constant_value in vars(constants).items():
    #    if constant_name.isupper():
    #        if not os.path.exists(constant_value):
    #            os.makedirs(constant_value)

    #with tempfile.NamedTemporaryFile() as tmp:
    #    input_file_check_result = check_commit_log.check_commit_log(checker_path, input_file, tmp.name)
    #    commit_log_constants = input_file_check_result.constants
    # print("Commit log constants", commit_log_constants)
    
    #shutil.copy(input_file, os.path.join(mutated_files_dir, os.path.basename(input_file)))

    # Substitute each instruction with a NOP and check if the commit log is benign
    # If so, we found the instruction(s) responsible for the counterexample
#    with open(input_file, "rb") as f:
#        instr_dict = utils.get_instruction_dict_from_bytestring(f.read())
#    instr_dict_orig = copy.deepcopy(instr_dict)
    #VU: I am not sure this works, because the instruction_dict seems to be updated in the loop itself? Is this intended?
    #First: Minimize the example

    #First: Let's find out the "bug type"
    #if check_result.difference_location.instruction_number == 0:
    #    interesting_instrs = []
    

if __name__ == "__main__":
    # Set the logging level (e.g., DEBUG, INFO, WARNING, ERROR, CRITICAL)
    # logger.setLevel(logging.DEBUG)

    # # Create a handler that outputs to stdout
    # handler = logging.StreamHandler(sys.stdout)

    # # Set the logging level for the handler
    # handler.setLevel(logging.DEBUG)

    # # Create a formatter and set it for the handler
    # formatter = logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')
    # handler.setFormatter(formatter)

    # # Add the handler to the logger
    # logger.addHandler(handler)
    # logging.info("Starting cex generator")
    main()
