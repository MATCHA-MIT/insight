from html import parser
import subprocess
import os
import tempfile
import shutil
import sys
from pathlib import Path
import time
subdir = Path(__file__).parent.parent
sys.path.append(str(subdir))

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))
subdir_mut = Path(__file__).parent.parent / "mutation_engine"
sys.path.append(str(subdir_mut))


import json
import constants
import uuid
import datetime
import analyzer
import vcd_trace
import cffi
import hashlib
import logging
import random
import tqdm
import common
import enum
import typing
import generate_csr_separators
import pathlib
import seed_generator

from concurrent.futures import ThreadPoolExecutor, as_completed

# Re-ordered paths to handle early analyzer import

from riscv_instruction_mutator import FileSource
from multiprocessing import Pool
print(sys.path)
import cex_generator
import re
import argparse
import os
import tempfile
import shutil
import random

logger = logging.getLogger("main")

def check_invariant(cex, invariants_path):
    potential_invariants = analyzer.waveform_fullfills_any_invariant(cex[constants.WAVEFORM_PATH_KEY], invariants_path)
    if potential_invariants is not None:
        return (cex, potential_invariants)
    return None

def setup_logging(level: int = logging.DEBUG):
    orig_factory = logging.getLogRecordFactory()

    if os.isatty(sys.stderr.fileno()):
        fmt = '%(asctime)s %(color)s[%(levelname)s:%(name)s:%(funcName)s:%(lineno)d] %(message)s%(color_reset)s'
        level_colors = {
            logging.CRITICAL: '\x1b[1;31m',
            logging.ERROR: '\x1b[31m',
            logging.WARNING: '\x1b[33m',
            logging.INFO: '\x1b[32m',
            logging.DEBUG: '\x1b[34m',
        }

        def record_factory(*args, **kwargs):
            record = orig_factory(*args, **kwargs)
            lvl = record.levelno
            record.color = level_colors.get(lvl, "")
            record.color_reset = '\x1b[0m'
            record.levelname = 'FATAL' if lvl == logging.CRITICAL else record.levelname
            return record
    else:
        fmt = '%(asctime)s [%(levelname)s:%(name)s] %(message)s'

        def record_factory(*args, **kwargs):
            record = orig_factory(*args, **kwargs)
            record.levelname = 'FATAL' if record.levelno == logging.CRITICAL else record.levelname
            return record
        
    logging.basicConfig(level=level, format=fmt, datefmt='%Y-%m-%d %H:%M:%S')
    logging.setLogRecordFactory(record_factory)

# def compile_rust_ffi_library():
#     subprocess.run(["cargo", "build", "--release"], cwd="./formula_finder/")
#     #shutil.copy("./formula_finder/target/release/libinvariant_finder_rust.so", constants.INVARIANT_FINDER_LIBRARY_PATH)
# compile_rust_ffi_library()

ffi = cffi.FFI()
rust_finder_library = ffi.dlopen(constants.INVARIANT_FINDER_LIBRARY_PATH)
ffi.cdef('bool ffi_check_on_waveform(char *waveform_path, char *invariant_json_path, char *clock_signal);')
ffi.cdef('const char *ffi_find_invariant(const char *output_sets_path, const char *regex_config_path, uint64_t bex_multiplier, uint64_t predicate_base_cost);')
ffi.cdef("void ffi_free_library_string(const char *ptr);")
ffi.cdef("char *check_cex_items_against_invariants(const char *cex_items, const char *invariant_directory);")
ffi.cdef("void *ffi_set_bex_multiplier(uint64_t multiplier);")
ffi.cdef("void *ffi_set_predicate_cost(uint64_t cost);")

# def write_hex_little_endian(hex_string, output_file):
#     # Split the hex string into 2-byte chunks and reverse them
#     logger.debug(f"hex_string: {hex_string}")
#     try:
#         reversed_bytes = bytearray.fromhex(hex_string)
#     except ValueError as e:
#         logger.error(f"Invalid hex string: {hex_string}. Error: {e}")
#         raise ValueError(f"Invalid hex string: {hex_string}. Error: {e}")
#     reversed_bytes.reverse()
    
#     # Write the reversed bytes to the binary file
#     with open(output_file, 'ab') as f:  # 'ab' mode to append in binary format
#         f.write(reversed_bytes)


def find_last_checkpoint_in_sweep_dir(sweep_dir):
    pattern = re.compile(r"sweep_bex_weight_(\d+)_predicate_cost_(\d+)")
    
    max_bex_weight = None
    max_predicate_cost = None
    
    for entry in os.listdir(sweep_dir):
        match = pattern.match(entry)
        if match:
            bex_weight = int(match.group(1))
            predicate_cost = int(match.group(2))
            #print(f"Found checkpoint: bex_weight={bex_weight}, predicate_cost={predicate_cost}", flush=True)
            if max_bex_weight is None or bex_weight > max_bex_weight:
                max_bex_weight = bex_weight
            if max_predicate_cost is None or predicate_cost > max_predicate_cost:
                max_predicate_cost = predicate_cost

    return max_bex_weight, max_predicate_cost



## Example usage:
#hex_string = "12345678"
#output_file = "output.bin"

def int_to_hex_with_zeros(num, length):
    hex_str = hex(num)[2:]  # Convert to hex and remove '0x' prefix
    return hex_str.zfill(length)


    
def run_separator(output_sets_path, regex_config_path, bex_weight, predicate_cost):
    """
    regex_config_path: A json file that specifies 
    the prioritizes of the signals. We process in stages. For each stage,
    give a name and a list of regexes that a signal can fulfill.
    """
    
    if type(output_sets_path) == str:
        logger.debug("I need to convert output_sets_path to string to call ffi")
        output_sets_path = output_sets_path.encode()
    if type(regex_config_path) == str:
        logger.debug("I need to convert regex_config_path to string to call ffi")
        regex_config_path = regex_config_path.encode()
    ret_string = ffi.NULL
    try:
        ret_string = rust_finder_library.ffi_find_invariant(output_sets_path, regex_config_path, bex_weight, predicate_cost)
        #if ret_string == ffi.NULL:
        #    ret = None
        ret_dict = json.loads(ffi.string(ret_string).decode("utf-8"))
    except Exception as e:
        print("Exception", e)
        raise e
    finally:
        logger.debug(f"Freeing ret_string {ret_string}")
        if ret_string is not ffi.NULL:
            rust_finder_library.ffi_free_library_string(ret_string)
    return ret_dict
            
def build_opcodes(logfile=None):
    logger.info("Setting up riscv_instruction_mutator")
    subprocess.run(["./setup.sh"], cwd="./mutation_engine/", stdout=logfile)

def clean_invariants_directory():
    logger.info("Cleaning up invariants directory")
    try:
        shutil.rmtree(constants.INVARIANT_PATH)
    except FileNotFoundError:
        pass
    #pass
    
def replace_content_between_markers(file_path, new_text, start_marker, end_marker):
    with open(file_path, 'r') as file:
        lines = file.readlines()
    in_block = False
    with open(file_path, 'w') as file:
        for line in lines:
            if line.strip() == start_marker:
                file.write(line)
                file.write(new_text + '\n')
                in_block = True
            elif line.strip() == end_marker:
                file.write(line)
                in_block = False
            elif not in_block:
                file.write(line)

def good_invariant(path_to_invariant_file, assert_invariant, jg_configs: dict, logfile=None):
    logger.info(f"Testing invariant {assert_invariant}")
    start_marker = "# ADD ASSERTION START (DO NOT EDIT)"
    end_marker = "# ADD ASSERTION END"
    replace_content_between_markers(path_to_invariant_file, assert_invariant, start_marker, end_marker)
    this_dir  = os.path.dirname(os.path.abspath(path_to_invariant_file))
    sync_to_jg_server(jg_configs, this_dir, logfile)
    returncode, stdout, stderr = run_jg(jg_configs["server_invariant_check_tcl_script_path"], jg_configs, logfile)
    #print(path_to_invariant_file)

    if returncode == 255:
        return True
    else:
        return False

def cleanup(logfile=None):
    logger.info("Cleaning up utility files")
    subprocess.run(["rm -rf ./waveforms"], stdout=logfile, stderr=logfile, shell=True)
    subprocess.run(["rm -rf ./mutations"], stdout=logfile, stderr=logfile, shell=True)
    subprocess.run(["rm ./feature_importance.csv"], stdout=logfile, stderr=logfile, shell=True)
    subprocess.run(["rm ./output_ibex.txt"], stdout=logfile, stderr=logfile, shell=True)
    subprocess.run(["rm ./output_sets.json"], stdout=logfile, stderr=logfile, shell=True)
    #os.makedirs(constants.JG_FOUND_CEXS, exist_ok=True)
    #os.makedirs(constants.JG_FOUND_BENIGN, exist_ok=True)
    



def add_seed_invariants(from_dir, to_dir):
    shutil.copytree(from_dir, to_dir, dirs_exist_ok=True)


def copy_seed_invariants(seed_invariants, from_dir, to_dir):
    if not seed_invariants:
        add_seed_invariants(from_dir=from_dir, to_dir=to_dir)
        return
    os.makedirs(to_dir, exist_ok=True)
    for seed_name in seed_invariants:
        #src_path = os.path.join(from_dir, seed_name)
        src_path = seed_name
        dest_name = os.path.basename(seed_name)
        if not os.path.exists(src_path):
            raise FileNotFoundError(f"Seed invariant not found: {src_path}")
        dest_path = str(os.path.join(to_dir, dest_name))
        if str(src_path) == dest_path:
            logger.warning(f"Source and destination are the same for {src_path}, skipping copy")
            continue
        print("src_path", src_path, "dest_path", dest_path, "equal?", src_path == dest_path)
        shutil.copy2(src_path, dest_path)


def compile_testbenches(logfile):
    return
    logger.info("Compiling testbenches")
    subprocess.run(["./scripts/compile_testbenches.sh"], stdout=logfile)

def clean_cexs_directory(logfile, output_dir):
    logger.info("Cleaning up cexs directory")
    output_dir_cex = os.path.join(output_dir, "cexs")
    output_dir_benign_examples = os.path.join(output_dir, "benign_examples")
    # subprocess.run(["rm -rf output/cexs"], stdout=logfile, stderr=logfile, shell=True)
    #subprocess.run(["rm -f input_cex.bin minimized_cex.bin"], stdout=logfile, stderr=logfile, shell=True)
    #shutil.rmtree("output/benign_examples/", ignore_errors=True)
    shutil.rmtree(output_dir_cex, ignore_errors=True)
    shutil.rmtree(output_dir_benign_examples, ignore_errors=True)
    
def memory_signal_to_jg_transformation(memory_signal: str):
    #Transform correctness.sodor_core.memory.mem_ext.Memory[34][31:0]
    #into sodor_core.memory.mem_ext.Memory[34]
    if memory_signal.startswith("correctness."):
        memory_signal = memory_signal[len("correctness."):]
    memory_signal = re.sub(r'\[([0-9]+)\]\[([0-9]+):([0-9]+)\]', r'[\1]', memory_signal)
    return memory_signal

class EventType(enum.Enum):
    genin_start = 1,
    jg_query_started = 2,
    jg_query_completed = 3,
    running_insight_on_cex = 4,
    mutation_started = 5,
    mutation_completed = 6,
    separator_started = 7,
    separator_completed = 8,
    picked_new_cex_from_mutations = 9,
    program_length_increased = 10,
    genin_stop = 11,
    cex_assumption_added = 12,
    time_limit_increased = 13,
    opcode_completed = 14,
    cex_found_from_jg = 15

class EventLogger:
    def __init__(self, event_log_file_path: str):
        self.event_log_file_path = event_log_file_path
        event_log_parent_dir = os.path.dirname(self.event_log_file_path)
        if event_log_parent_dir:
            os.makedirs(event_log_parent_dir, exist_ok=True)
        needs_header = not os.path.exists(self.event_log_file_path) or os.path.getsize(self.event_log_file_path) == 0
        if needs_header:
            with open(self.event_log_file_path, "a") as f:
                f.write("Event;Id;Time;Details;Bugs;Underspecification\n")
    
    def log_event(self, event_type, event_details=None, bugs=None, underspecification=None):
        current_time = time.time()
        #Log event with current time to csv, something like this:
        if event_details is None:
            event_details = ""
        if bugs is None:
            bugs = []
        if underspecification is None:
            underspecification = ""

        def _serialize_field(value):
            if value is None:
                return ""
            if isinstance(value, str):
                return value
            return json.dumps(value, sort_keys=True)

        event_details = _serialize_field(event_details)
        bugs_serialized = _serialize_field(bugs)
        underspecification_serialized = _serialize_field(underspecification)
        #id;event_description;current_time;event_details
        with open(self.event_log_file_path, "a") as f:
            f.write(f"{event_type.name};{event_type.value};{current_time};{event_details};{bugs_serialized};{underspecification_serialized}\n")


class JGConnector:
    def __init__(self, jg_config: dict, config: dict) -> None:
        self.jg_config = jg_config
        with open(config["regex_config_path"], "r") as f:
            regex_config = json.load(f)
        self.sodor_offset = regex_config.get("sodor_offset", 32)
        self.memory_signal_ref_format_string_jg = regex_config.get("memory_format_string_ref", "correctness.sodor_core.memory.mem_ext.Memory[{idx}][63:0]")
        self.memory_signal_dut_format_string_jg = regex_config["memory_format_string_dut"]
        self.dut_offset  = self.sodor_offset
        self.memory_size = regex_config.get("memory_size", 64)
        self.first_symbolic_instruction_idx = regex_config.get("first_symbolic_instruction_idx", 0)
    
    # This function should be configurable 
    # path_to_tcl_template -> depends on the core
    # path_to_tcl_file -> depends on the core 
    def add_invariant(self, path_to_correctness_file: str, assume_invariant, invariant_filepath=None):
        with open(path_to_correctness_file, 'r+') as f:
        # find the line
            line = ""
            while "# ADD ASSUMPTIONS START" not in line:
                line = f.readline()
                if not line:
                    break
            if "# ADD ASSUMPTIONS START" not in line:
                logger.error(f"Could not find # ADD ASSUMPTIONS START in {path_to_correctness_file}")
                raise Exception(f"Could not find # ADD ASSUMPTIONS START in {path_to_correctness_file}")
            # save our position
            pos = f.tell()
            # read the rest of the file
            remainder = f.read()
            # return to the line after the #pragma
            f.seek(pos)
            # write the new method
            f.write(f"{assume_invariant}\n")
            # write the rest of the file
            f.write(remainder)

    
    def set_current_max_cex_length(self, length: int):
        self.current_max_cex_length = length
    
    def initialize_waveform_extractor(self):
        self.waveform_extractor = common.WaveformExtractor(self.memory_signal_ref_format_string_jg,
                                                         self.memory_signal_dut_format_string_jg,
                                                         self.memory_size,
                                                        self.sodor_offset,
                                                        self.dut_offset,
                                                        self.first_symbolic_instruction_idx,
                                                        self.current_max_cex_length)
        
    
    def scp_specific_files_to_jg_server(self, source_file_path: str, target_file_path: str):
        jg_server = self.jg_config["jg_server"]
        #target_file_path = self.jg_config["server_correctness_tcl_script_path"]
        #logger.info(f"SCPing to {jg_server}:{jg_target_dir}")
        args = f"scp {source_file_path} {jg_server}:{target_file_path}"
        print("Executing command:", args, flush=True)
        subprocess.run(args, shell=True, check=True)

    def sync_to_jg_server(self, this_dir: str, logfile=None):
        jg_server = self.jg_config["jg_server"]
        jg_target_dir = self.jg_config["jg_target_dir"]
        if not this_dir.endswith('/'):
            print(f"Warning: Adding a trailing / to this_dir for rsync {this_dir}")
            this_dir += '/'
        logger.info(f"Rsyncing from {this_dir} to {jg_server}:{jg_target_dir.removesuffix('/')}")
        if logfile is None:
            logfile = sys.stdout 
        ret = subprocess.run([f"./orchestration/rsync_to_jasper.sh {this_dir} {jg_server}:{jg_target_dir.removesuffix('/')}"], stdout=logfile, shell=True, stderr=logfile)
        if ret.returncode != 0:
            logger.critical("ERROR: Failed to rsync to JasperGold server")
            sys.exit()



    def run_jg(self, server_tcl_script_path, logfile=None):
        logger.info("Running JasperGold on the remote server")
        jg_server = self.jg_config["jg_server"]
        vcd_out_path = self.jg_config["vcd_out_path"]
        tcl_script_dir = os.path.dirname(server_tcl_script_path)
        tcl_script_filename = os.path.basename(server_tcl_script_path)

        logger.debug(f"VCD output directory: {vcd_out_path}")
        # Create a temporary file to write the Bash script
        """
        with tempfile.NamedTemporaryFile(delete=False, suffix='.sh') as temp_sh_file:
            temp_sh_file.write(b"#!/bin/bash\n")
            #temp_sh_file.write(b"use JASPER\n")
            temp_sh_file.write(f"cd {tcl_script_dir}\n".encode())
            temp_sh_file.write(f"rm {vcd_out_path}/* \n".encode())
            temp_sh_file.write(f"jg {tcl_script_filename} -batch -acquire_proj | tee jg_out.log\n".encode()) # I am not sure whether we should really acquire the project here
            temp_sh_file.write(f" \n".encode())
            temp_sh_file.write(f"cat jg_out.log\n".encode())
            temp_sh_file.write(f"status=$( tac jg_out.log | grep -m 1 \"time_limit\" )\n".encode())
            temp_sh_file.write(f"if [ \"$status\" = \"time_limit\" ]; then exit 255;  fi\n".encode())
            temp_sh_path = temp_sh_file.namerust_finder_library
        ssh_command = f"ssh {jg_server} 'bash -ls' < {temp_sh_path}"
        """
        # Create a temporary file to write the Bash script
        with tempfile.NamedTemporaryFile(delete=False, suffix='.sh') as temp_sh_file:
            temp_sh_file.write(b"#!/bin/tcsh\n")
            temp_sh_file.write(b"use JASPER\n")
            temp_sh_file.write(f"cd {tcl_script_dir}\n".encode())
            temp_sh_file.write(f"rm {vcd_out_path}/* \n".encode())
            temp_sh_file.write(f"jg {tcl_script_filename} -batch -acquire_proj -proj automated_jasper_run | tee jg_out.log\n".encode()) # I am not sure whether we should really acquire the project here
            #temp_sh_file.write(f" \n".encode())
            #temp_sh_file.write(f"cat jg_out.log\n".encode())
            #temp_sh_file.write(f"set status=\"` cat jg_out.log | grep -m 1 \"time_limit\" `\"\n".encode())
            #temp_sh_file.write(f"if [ \"$status\" = \"time_limit\" ]; then exit 255;  fi\n".encode())
            temp_sh_path = temp_sh_file.name
        ssh_command = f"ssh {jg_server} 'tcsh -s' < {temp_sh_path}"
        print("Running SSH command:", ssh_command, flush=True)
        process = subprocess.Popen(ssh_command, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, bufsize=1)
        stdout_chunks = []
        stderr_chunks = []
        for line in iter(process.stdout.readline, b''):
            sys.stdout.buffer.write(line)
            stdout_chunks.append(line)
        for line in iter(process.stderr.readline, b''):
            sys.stderr.buffer.write(line)
            stderr_chunks.append(line)
        process.stdout.close()
        process.stderr.close()
        process.wait()
        stdout = b''.join(stdout_chunks)
        stderr = b''.join(stderr_chunks)
        logger.info(f"SSH command stdout: {stdout.decode('utf-8', errors='replace')}")
        logger.info(f"SSH command execution with return code {process.returncode}")
        result = subprocess.CompletedProcess(args=ssh_command, returncode=process.returncode, stdout=stdout, stderr=stderr)
        if result.returncode != 0 and result.returncode != 255:
            logger.critical("ERROR: Failed to run bash script on server")
            logger.critical(f"Stdout: {stdout}")
            logger.critical(f"Stderr: {stderr}")
            sys.exit()
        return result.returncode, stdout, stderr

    def get_cex_from_server(self, logfile, logging_directory: str):
        jg_server = self.jg_config["jg_server"]
        vcd_out_path = self.jg_config["vcd_out_path"]

        logger.info(f"Retrieving CEX from {jg_server}:{vcd_out_path}")
        temp_bin_path = None
        # Create a temporary directory to store the downloaded files
        with tempfile.TemporaryDirectory() as temp_dir:
            # Construct the scp command to download the directory
            scp_command = f"scp -r {jg_server}:{vcd_out_path} {temp_dir}"
            logger.debug(f"Executing command: {scp_command}")
            try:
                # Execute the scp command
                result = subprocess.run(scp_command, shell=True, stdout=logfile, stderr=logfile)
                logger.debug(f"Result: {result} with return code {result.returncode}")
                if result.returncode == 1:
                    raise Exception(f"Could not reach server {scp_command}, logfile {logfile}")

                # List all files in the temporary directory
                logger.debug(f"Files in {temp_dir}:")
                result_list = []
                for root, dirs, files in os.walk(temp_dir):
                    for file in files:
                        if file.endswith(".gz"):
                            result_list.append(os.path.join(root, file))
                        if file.endswith(".vcd"):
                            result_list.append(os.path.join(root, file))
                if len(result_list) == 0:
                    return None, None
                logger.debug(f"Got the following waveforms {result_list}")
                logger.debug(f"Now unzipping {result_list[0]}")
                for received_file in result_list:
                    if received_file.endswith(".vcd"):
                        vcd_filepath = received_file
                    else:
                        subprocess.check_output([f"gzip -d {received_file}"], stderr=logfile, shell=True)
                        vcd_filepath = os.path.splitext(received_file)[0]
                    logging_vcd_filepath = os.path.join(logging_directory,os.path.basename(vcd_filepath))
                    shutil.copy(vcd_filepath, logging_vcd_filepath)
                    if "noConflict" in vcd_filepath or "no_conflict" in vcd_filepath:
                        raise Exception(f"Got a noConflict waveform {vcd_filepath}, stored at {logging_vcd_filepath} this should not happen?")
                    elif "noDeadEnd" in vcd_filepath:
                        print("############### Eerror got noDeadEnd #### but igonring")
                        continue
                    elif "correct_check" in vcd_filepath:
                        print("Extracting instructions from", logging_vcd_filepath, flush=True)
                        temp_bin_path = self.waveform_extractor.extract_instructions_and_disassemble(logging_vcd_filepath)
                        return temp_bin_path, logging_vcd_filepath
                    else:
                        raise Exception(f"Unexpected waveform file {logging_vcd_filepath}")

            except subprocess.CalledProcessError as e:
                logger.critical(f"Error occurred while downloading the directory:\n{e.output}\n{e.stderr}")
                raise e
        print("Could not find a valid CEX on the server;  returning None", flush=True)
        return None, None
        raise Exception("Could not find a valid CEX on the server; should never end up here")

    def add_invariants_to_tcl_files(self, invariants_path, path_to_correctness_file, path_to_incorrectness_file):
        for i, invariant_file in enumerate(os.listdir(invariants_path)):
            invariant_file_path = os.path.join(invariants_path, invariant_file)
            with open(invariant_file_path, "r") as f:
                try:
                    file_content = json.load(f)
                except Exception as e:
                    print("Exception when loading file ", invariant_file_path)
                    raise e
                #tree = Tree.fromstring(file_content["formula"])
                #model_data = file_content["model_data"]
                #separator_formula = SeparatorFormulaV3(tree, None, None)
                #assume_invariant, _ = separator_formula.generate_jg_assume_and_assert(model_data)
                assume_invariant = file_content["assume_invariant"]
                logger.debug(f"File path: {invariant_file_path}")
                logger.info(f"Adding to correctness check from seed invariant {assume_invariant}")
                self.add_invariant(path_to_correctness_file, assume_invariant)
                self.add_invariant(path_to_incorrectness_file, assume_invariant)

class MockJGConnector:
    def __init__(self, jg_config: dict, config: dict) -> None:
        pass
    
    def set_available_cex(self, available_cex_paths: list):
        self.available_cex_paths = available_cex_paths
        self.current_cex_idx = 0
    
    def set_current_max_cex_length(self, length: int):
        self.current_max_cex_length = length

    def get_cex_from_server(self, logfile, logging_directory: str):
        # Ensure logging directory exists
        os.makedirs(logging_directory, exist_ok=True)

        max_len_bytes = self.current_max_cex_length
        if max_len_bytes is None:
            # If not set, behave as if no constraint
            max_len_bytes = float("inf")
        else:
            # convert instruction count to bytes (4 bytes per instruction)
            max_len_bytes = int(max_len_bytes) * 4

        # Filter available CEXs by file existence and size constraint
        candidates = [
            p for p, _ in self.cex_waveforms
            if os.path.exists(p) and os.path.getsize(p) <= max_len_bytes
        ]

        if not candidates:
            # No suitable CEX found
            return None, None

        chosen = random.choice(candidates)

        # Copy chosen binary to a temporary file (simulate downloaded binary)
        tmp_bin = tempfile.NamedTemporaryFile(delete=False, suffix=".bin")
        tmp_bin_path = tmp_bin.name
        tmp_bin.close()
        shutil.copy(chosen, tmp_bin_path)

        # Create a simple mock VCD file in the logging directory to mimic server waveform
        vcd_basename = os.path.splitext(os.path.basename(chosen))[0] + ".vcd"
        vcd_path = os.path.join(logging_directory, vcd_basename)
        with open(vcd_path, "w") as vf:
            vf.write(f"// Mock VCD for {os.path.basename(chosen)}\n")

        return tmp_bin_path, vcd_path

    def initialize_waveform_extractor(self):
        pass
    
    def sync_to_jg_server(self, this_dir: str, logfile=None):
        pass
    
    def run_jg(self, server_tcl_script_path, logfile=None):
        logger.info("Mock run_jg called")
        return 0, b"Mock stdout", b"Mock stderr"

    def scp_specific_files_to_jg_server(self, source_file_path: str, target_file_path: str):
        pass

    def set_checker_path(self, checker_path):
        self.checker_path = checker_path
        
    def generate_cex_waveforms(self):
        self.cex_waveforms = []
        logging.info("Generating waveforms for available CEXs")
        
        def process_cex(cex_path):
            with tempfile.NamedTemporaryFile(delete=False, suffix=".vcd") as temp_vcd_file:
                temp_vcd_path = temp_vcd_file.name
                this_res: typing.Optional[analyzer.CommitLogCheckerResult] = analyzer.check_commit_log(self.checker_path, cex_path, temp_vcd_path)
                if this_res is None:
                    raise Exception(f"Checker failed to run on {cex_path}")
                if this_res.Kind != analyzer.CheckerResultKind.CEX:
                    print(f"Expected CEX from checker for {cex_path}, got {this_res.Kind}, ignoring")
                    return None
                return (cex_path, temp_vcd_path)
        
        with ThreadPoolExecutor() as executor:
            futures = [executor.submit(process_cex, cex_path) for cex_path in self.available_cex_paths]
            for future in tqdm.tqdm(as_completed(futures), total=len(futures)):
                result = future.result()  # This will raise any exception that occurred
                if result is not None:
                    self.cex_waveforms.append(result)

    def add_invariants_to_tcl_files(self, invariants_path, path_to_correctness_file, path_to_incorrectness_file):
        cex_items = []
        for cex_path, waveform_path in self.cex_waveforms:
            cex_items.append({
                "file": cex_path,
                constants.WAVEFORM_PATH_KEY: waveform_path,
                "file_source": FileSource.Mutations
            })
        remaining_cex = common.identify_non_covered_cex_items_through_ffi_call(cex_items=cex_items, invariant_directory=invariants_path)
        self.cex_waveforms = []
        for cex_item in remaining_cex:
            cex_path = cex_item["file"]
            waveform_path = cex_item[constants.WAVEFORM_PATH_KEY]
            self.cex_waveforms.append((cex_path, waveform_path))
        
    
    def add_invariant(self, path_to_correctness_file: str, assume_invariant, invariant_filepath):
        invariants_path = os.path.dirname(invariant_filepath)
        cex_items = []
        for cex_path, waveform_path in self.cex_waveforms:
            cex_items.append({
                "file": cex_path,
                constants.WAVEFORM_PATH_KEY: waveform_path,
                "file_source": FileSource.Mutations
            })
        remaining_cex = common.identify_non_covered_cex_items_through_ffi_call(cex_items=cex_items, invariant_directory=invariants_path)
        self.cex_waveforms = []
        for cex_item in remaining_cex:
            cex_path = cex_item["file"]
            waveform_path = cex_item[constants.WAVEFORM_PATH_KEY]
            self.cex_waveforms.append((cex_path, waveform_path))
    
    


class PipelineLoop:
    def __init__(self, jg_configs, regex_config_path, core_name, checker_path, seed_invariants=None, inject_jg_connector = None, bug_verification_testbenches=None):
        self.jg_configs = jg_configs
        #self.output_dir: str = output_dir
        #self.invariants_dir_path: str = os.path.join(output_dir, constants.INVARIANT_PATH)
        self.regex_config_path: str = regex_config_path
        self.core_name: str = core_name
        self.checker_path: str = checker_path
        self.cex_generator_instance = None
        # Setup logging
        setup_logging()
        self.correctness_tcl_path = f"./example_cores/compare_to_{self.core_name}/generated_files/correctness.tcl"
        self.allowed_signals = None
        if inject_jg_connector is None:
            self.jg_connector_obj = JGConnector(jg_configs, {"regex_config_path": self.regex_config_path})
            correctness_target_file = str(Path(self.jg_configs["jg_target_dir"]).parent / f"common" / "testbench" / "correctness_inner_pkg.sv")
            print(f"Copying to {correctness_target_file}")
            self.jg_connector_obj.scp_specific_files_to_jg_server(f"./example_cores/common/testbench/correctness_inner_pkg.sv", correctness_target_file)
            self.set_allowed_signals()
            
        else:
            self.jg_connector_obj = inject_jg_connector
        self.conditional_signals_to_condition_mapping = None
        self.sodor_offset = 32
        self.memory_size = 64
        self.start_address = 0x80000000
        self.first_symbolic_instruction_idx = 0
        #self.memory_signal_format_string = "correctness.sodor_core.memory.mem_ext.Memory[{idx}][63:0]"
        #self.memory_signal_dut_format_string_jg = "kronos.dut.mem[{idx}]"
        #self.memory_signal_ref_format_string_jg = "sodor_core.memory.mem_ext.Memory[{idx}]"
        self.memory_signal_dut_format_string_jg = None
        self.memory_signal_ref_format_string_jg = None
        self.current_max_cex_length = None
        self.use_insight = True
        self.cex_waveform_counter = 0
        self.bug_verification_testbenches = bug_verification_testbenches or {}
        if self.core_name not in jg_configs["server_correctness_tcl_script_path"]:
            raise Exception(f"Core name {self.core_name} not in server_correctness_tcl_script_path {jg_configs['server_correctness_tcl_script_path']}. Comment out this exception if this is intended")
        self.output_dir = None
        self.seed_invariants = seed_invariants or []

    
    def setup_output_directories(self, output_dir: str):
        # Create the directory name
        # Get the current datetime
        current_datetime = datetime.datetime.now()
        # Format the datetime as a string (you can customize the format as needed)
        # For example, '2023-10-05_14-30-00' for 'YYYY-MM-DD_HH-MM-SS'
        formatted_datetime = current_datetime.strftime('%Y-%m-%d_%H-%M-%S')
        self.logging_directory = os.path.join(output_dir, "logging", formatted_datetime)
        logfile_name = os.path.join(self.logging_directory, "run_full_pipeline.log")
        os.makedirs(self.logging_directory, exist_ok=True)
        self.logfile = open(logfile_name, "a+")
        # Create the directory
        self.output_dir = output_dir
        os.makedirs(self.output_dir, exist_ok=True)
        self.invariants_dir_path = os.path.join(self.output_dir, constants.INVARIANT_PATH)
        os.makedirs(self.invariants_dir_path, exist_ok=True)
        os.makedirs(os.path.join(self.output_dir, constants.CEX_PATH), exist_ok=True)
        os.makedirs(os.path.join(self.output_dir, constants.BENIGN_EXAMPLES_PATH), exist_ok=True)
        event_log_path = os.path.join(self.output_dir, "event_log.txt")
        self.event_logger = EventLogger(event_log_path)
        if self.allowed_signals is not None:
            with open(os.path.join(self.output_dir, "jg_allowed_signals.txt"), "w") as f:
                for signal in self.allowed_signals:
                    f.write(f"{signal}\n")

    def generate_tcl_from_template(self, correctness_template, invariant_template):
        logger.info("Generating .tcl files from templates")
        generated_files_dir = Path(os.path.dirname(correctness_template)).parent / "generated_files"
        generated_files_dir.mkdir(exist_ok=True)
        shutil.copy(correctness_template, generated_files_dir / "correctness.tcl")
        shutil.copy(invariant_template, generated_files_dir / "invariant.tcl")
        if self.current_max_cex_length is None:
            raise Exception("current_max_cex_length is None, cannot generate tcl from template")
        if self.memory_signal_ref_format_string_jg is None:
            raise Exception("memory_signal_ref_format_string_jg is None, cannot generate tcl from template")
        if self.memory_signal_dut_format_string_jg is None:
            raise Exception("memory_signal_dut_format_string_jg is None, cannot generate tcl from template")
        with open(generated_files_dir / "correctness.tcl", "r+") as f:
            lines = f.readlines()
            start_marker = "#ADD MEMORY ABSTRACTION START (DO NOT EDIT)"
            end_marker = "# ADD MEMORY ABSTRACTION END"
            output_lines = []
            in_block = False
            for line in lines:
                if "TIME_LIMIT" in line:
                    line = line.format(TIME_LIMIT=self.current_time_limit)
                if "TESTED_OPCODE_NO1" in line:
                    line = line.format(TESTED_OPCODE_NO1=self.current_opcode)
                if "TESTED_OPCODE_NO2" in line:
                    if self.current_max_cex_length > 1:
                        line = line.format(TESTED_OPCODE_NO2=self.current_opcode)
                    else:
                        #7'b0000011, 7'b0100011,
                        all_opcodes = "7'b0110011, 7'b0010011, 7'b1100011, 7'b1101111,  7'b1100111, 7'b0110111, 7'b0010111,7'b0001111, 7'b1110011"
                        line = line.format(TESTED_OPCODE_NO2=all_opcodes)
                if "MAX_TRACE_LENGTH" in line:
                    line = line.format(MAX_TRACE_LENGTH=self.max_trace_length)
                output_lines.append(line)
                if line.strip() == start_marker:
                    in_block = True
                    # Insert abstraction lines
                    if self.memory_size == 64:
                        # For 64-bit memory: 2 instructions per memory location
                        for idx in range(0, self.current_max_cex_length):
                            idx_ref = (self.sodor_offset + idx + self.first_symbolic_instruction_idx) // 2
                            memory_signal = self.memory_signal_ref_format_string_jg.format(idx=idx_ref)
                            memory_signal = memory_signal_to_jg_transformation(memory_signal)
                            bitselect = "[31:0]" if (idx % 2 == 0) else "[63:32]"
                            output_lines.append(f"abstract -init_value {{{memory_signal}{bitselect}}}\n")
                            idx_dut = (self.dut_offset + idx + self.first_symbolic_instruction_idx) // 2
                            memory_signal_dut = self.memory_signal_dut_format_string_jg.format(idx=idx_dut)
                            memory_signal_dut = memory_signal_to_jg_transformation(memory_signal_dut)
                            output_lines.append(f"abstract -init_value {{{memory_signal_dut}{bitselect}}}\n")
                    elif self.memory_size == 32:
                        # For 32-bit memory: 1 instruction per memory location
                        for idx in range(0, self.current_max_cex_length):
                            idx_ref = self.sodor_offset + idx + self.first_symbolic_instruction_idx
                            memory_signal = self.memory_signal_ref_format_string_jg.format(idx=idx_ref)
                            memory_signal = memory_signal_to_jg_transformation(memory_signal)
                            output_lines.append(f"abstract -init_value {{{memory_signal}}}\n")
                            idx_dut = self.dut_offset + idx + self.first_symbolic_instruction_idx
                            memory_signal_dut = self.memory_signal_dut_format_string_jg.format(idx=idx_dut)
                            memory_signal_dut = memory_signal_to_jg_transformation(memory_signal_dut)
                            output_lines.append(f"abstract -init_value {{{memory_signal_dut}}}\n")
                    else:
                        raise Exception(f"Unsupported memory size {self.memory_size}")
                elif line.strip() == end_marker and in_block:
                    in_block = False
            f.seek(0)
            f.writelines(output_lines)
            f.truncate()
        
    def set_refcore_offset(self, sodor_offset, memory_signal_format_string_ref):
        """
        Set the sodor offset and memory signal format string.
        This is used to extract the instructions from the VCD file.
        """
        self.sodor_offset = sodor_offset 
        self.memory_signal_ref_format_string_jg = memory_signal_format_string_ref
        
    def set_dut_offset(self, dut_offset, memory_signal_format_string_dut):
        self.dut_offset = dut_offset
        self.memory_signal_dut_format_string_jg = memory_signal_format_string_dut
    
    def set_memory_size(self, memory_size):
        self.memory_size = memory_size 
    
    def set_first_symbolic_instruction_idx(self, first_symbolic_instruction_idx):
        self.first_symbolic_instruction_idx = first_symbolic_instruction_idx
    
    def set_start_address(self, start_address):
        self.start_address = start_address
        
    def set_bex_weight(self, bex_weight):
        self.bex_weight = bex_weight
        #rust_finder_library.ffi_set_bex_multiplier(bex_weight)
    
    def set_predicate_cost(self, predicate_cost):
        self.predicate_cost = predicate_cost
        #rust_finder_library.ffi_set_predicate_cost(predicate_cost)

    def _is_fixed_core_key(self, bug_key: str, checker_path: str):
        normalized_key = bug_key.replace("-", "").replace("_", "").upper()
        if normalized_key in {"ALLFIX", "FIXED", "NOBUG"}:
            return True
        return os.path.normpath(checker_path) == os.path.normpath(self.checker_path)

    def classify_cex_bug_triggers(self, cex_path: str):
        if not self.bug_verification_testbenches:
            return [], []

        underspecification_testbenches = []
        bug_keys = []
        for bug_key, checker_path in self.bug_verification_testbenches.items():
            result = analyzer.check_commit_log(checker_path, cex_path, None)
            if result is None:
                logger.warning(f"Bug verification checker failed for key {bug_key} at path {checker_path} on cex {cex_path}")
                continue
            if result.Kind == analyzer.CheckerResultKind.CEX:
                underspecification_testbenches.append(bug_key)
                if not self._is_fixed_core_key(bug_key, checker_path):
                    bug_keys.append(bug_key)
        return bug_keys, underspecification_testbenches

    def set_allowed_signals(self):
        this_dir  = str(Path(os.path.dirname(self.correctness_tcl_path)).parent)
        self.jg_connector_obj.sync_to_jg_server(this_dir, None)
        dump_signals_script = self.jg_configs.get("server_dump_signals_tcl_script_path")
        print("dump_signals_script", dump_signals_script)
        if dump_signals_script is not None:
            logger.info(f"Dumping signals with {dump_signals_script}")
            returncode, stdout, stderr = self.jg_connector_obj.run_jg(dump_signals_script, None)
            stdout = stdout.decode('utf-8')
            if returncode != 0:
                raise Exception(f"Failed to dump signals with {dump_signals_script} with return code {returncode} stdout {stdout} stderr {stderr}")
            #print("Parsing stdout",stdout)
            signals_string = stdout.split("get_design_info -verbosity silent -list signal")[1].split("\n")[1].strip()
            signals_list = re.findall(r'\{[^}]*\}|\S+', signals_string)
            signals_list = [signal.strip('{}') for signal in signals_list]
            self.allowed_signals = signals_list
            #logger.info(f"Allowed signals: {self.allowed_signals}")
            #print(f"Allowed signals: {self.allowed_signals}")
    
    def set_conditional_signals_to_condition_mapping(self, cond_mapping_path):
        if not os.path.exists(cond_mapping_path):
            raise Exception(f"conditional_signals_to_condition_mapping_path {cond_mapping_path} does not exist")
        with open(cond_mapping_path, "r") as f:
            self.conditional_signals_to_condition_mapping = json.load(f)
        

    def run_cex_generator(self, temp_bin_path):
        logger.info("Cleaning up mutations, waveforms, and seeds directories")
        #shutil.rmtree("mutations", ignore_errors=True)
        #shutil.rmtree("waveforms", ignore_errors=True)
        #shutil.rmtree("seeds", ignore_errors=True)
        #os.makedirs("mutations", exist_ok=True)
        #os.makedirs("waveforms", exist_ok=True)
        os.makedirs(constants.SEEDS_DIR, exist_ok=True)
        #subprocess.run(["rm", "-rf", "output/cexs/*"])
        if self.cex_generator_instance is None:
            raise Exception("cex_generator_instance is None, cannot run cex generator")
        self.cex_generator_instance.set_seed_invariants(self.seed_invariants)
        return self.cex_generator_instance.run_cex_generator(temp_bin_path)
        #logging.info(f'Running {" ".join(["./scripts/run_cex_generator.sh", "--target", temp_bin_path, "--output", output_dir_path, "--testbench", checker_path])}')
        #result = subprocess.run(["./scripts/run_cex_generator.sh", "--target", temp_bin_path, "--output", output_dir_path, "--testbench", checker_path])
        #if result.returncode != 0:
        #    print("run_cex_generator.sh failed with the following output:")
        #    print(result.stdout.decode() if result.stdout else "No stdout")
        #    print("And the following error:")
        #    print(result.stderr.decode() if result.stderr else "No stderr")
        #    print("", end="", flush=True)
        #    raise Exception(f"run_cex_generator.sh failed with return code {result.returncode}")
    # if result.returncode != 0:
    #     raise Exception(f"run_cex_generator.sh failed with return code {result.returncode}")
    
    def run_insight(self, current_cex_path, current_logging_directory, temp_bin_path_cex, new_logging_cex_vcd_path):
        logger.info("Loading waveforms")
        all_covered = False
        ret_dict = None
        iterations_per_cex = 0
        if self.output_dir is None:
            raise Exception("output_dir is None, cannot run insight")
        # Now we run the invariant generator until we find a good invariant
        # i.e., one that covers all the CEXs
        while not all_covered:
            instructions = cex_generator.disassemble_and_extract(current_cex_path)
            self.event_logger.log_event(event_type=EventType.running_insight_on_cex, event_details=instructions)
            # print(f"Running cex generator on current CEX iteration {iterations_per_cex}", flush=True)
            # commit_check_res_second = analyzer.check_commit_log(f"example_cores/compare_to_ibex/obj_dir/Vcorrectness", current_cex, None)
            # if commit_check_res_second.Kind != analyzer.CheckerResultKind.CEX:
            #     print(f"Line 490 Mismatch between buggy ibex and other ibex. Binary path {current_cex}, check_result {commit_check_res.Kind}, check_result2 {commit_check_res_second.Kind}")
            #     injected_bug_found = True
                #raise Exception(f"Line 490 Mismatch between buggy ibex and other ibex. Binary path {current_cex}, check_result {commit_check_res.Kind}, check_result2 {commit_check_res_second.Kind}")
            # First, we run the cex generator on the current CEX
            self.event_logger.log_event(event_type=EventType.mutation_started)
            output_sets = self.run_cex_generator(current_cex_path)
            if output_sets is None:
                raise Exception("output_sets is None after running cex generator")
            self.event_logger.log_event(event_type=EventType.mutation_completed)
            output_sets_path = os.path.join(self.output_dir, "output_sets.json")
            # with open(output_sets_path, "w") as fp:
            #     json.dump(output_sets, fp, indent=4, default=str)
            logging.info(f"Running cex generator done got {len(output_sets['cex'])} CEXs and {len(output_sets['bex'])} benign waveforms")
            # if len(output_sets["cex"]) == 0:
            #     print("We are done! No more cex!", flush=True)
            #     return
            if self.allowed_signals is not None:
                output_sets["allowed_signals"] = self.allowed_signals
            if self.conditional_signals_to_condition_mapping is not None:
                output_sets["conditional_signals_to_condition_mapping"] = self.conditional_signals_to_condition_mapping
            #output_sets["original_jaspergold_waveform_path"] = new_logging_cex_vcd_path

            with open(output_sets_path, "w") as fp:
                json.dump(output_sets, fp, indent=4, default=str)
            res = check_invariant(output_sets["minimized_cex"], self.invariants_dir_path)
            if res is not None:
                logger.error(f"Found invariant that covers minimized CEX {res}")
                #raise Exception(f"Found invariant that covers minimized CEX {res}!")
                
            #print("Cross checking with 'nonbuggy' cores", flush=True)
            #for cex_item in tqdm.tqdm(output_sets["cex"]):
            #    commit_check_res_second = analyzer.check_commit_log(f"example_cores/compare_to_ibex/obj_dir/Vcorrectness", cex_item["file"], None)
            #    if commit_check_res_second.Kind != analyzer.CheckerResultKind.CEX:
            #        print(f"Line 508 Mismatch between buggy ibex and other ibex. Binary path {cex_item['file']}, check_result {commit_check_res.Kind}, check_result2 {commit_check_res_second.Kind}")
            #        raise Exception(f"Line 508 Mismatch between buggy ibex and other ibex. Binary path {cex_item['file']}, check_result {commit_check_res.Kind}, check_result2 {commit_check_res_second.Kind}")
            timestamp = datetime.datetime.now().strftime('%Y-%m-%d_%H-%M-%S')
            invariant_filepath = os.path.join(self.invariants_dir_path, f"invariant_{timestamp}_{self.current_max_cex_length}_cex_{self.cex_waveform_counter}_{iterations_per_cex}_{str(uuid.uuid4())[:6]}.json")
            self.event_logger.log_event(event_type=EventType.separator_started)
            ret_dict = run_separator(output_sets_path=output_sets_path, regex_config_path=self.regex_config_path, bex_weight=self.bex_weight, predicate_cost=self.predicate_cost)
            self.event_logger.log_event(event_type=EventType.separator_completed, event_details=invariant_filepath+"=="+ret_dict["assume_invariant"])
            logger.info(f"Found separator {ret_dict['assume_invariant']}, fulfills {ret_dict['cex_fulfilled_percentage']}% of the CEXs, for cex {current_cex_path}")
            #invariant_template = f"./example_cores/compare_to_{self.core_name}/correctness_template.tcl"
            ret_dict["input_cex"]["filepath"] = current_cex_path
            minimized_cex_logging_path = os.path.join(current_logging_directory, f"minimized_cex_{self.cex_waveform_counter}_"+os.path.basename(temp_bin_path_cex))
            shutil.copy(output_sets["minimized_cex"]["path"], minimized_cex_logging_path)
            ret_dict["input_cex"]["minimized_cex_filepath"] = minimized_cex_logging_path
            with open(invariant_filepath, "w+") as f:
                json.dump(ret_dict, f, indent=4)
            shutil.copy(invariant_filepath, os.path.join(current_logging_directory, os.path.basename(invariant_filepath)))
            self.jg_connector_obj.add_invariant(f"./example_cores/compare_to_{self.core_name}/generated_files/correctness.tcl",ret_dict["assume_invariant"], invariant_filepath=invariant_filepath)
            #add_invariant(f"./example_cores/compare_to_{self.core_name}/invariant.tcl",ret_dict["assume_invariant"])
            with open(os.path.join(current_logging_directory, "invariant_to_cex_mapping.txt"), "a+") as fp:
                copy_to_logging_path =  os.path.join(current_logging_directory, os.path.basename(temp_bin_path_cex))
                fp.write(f"{invariant_filepath};{copy_to_logging_path};{new_logging_cex_vcd_path}\n")
            all_covered = ret_dict["cex_fulfilled_percentage"] == 100
            output_set_size = len(output_sets["cex"])
            
            # Check for duplicates in output_sets["cex"]
            seen = set()
            duplicates = []
            for cex_item in output_sets["cex"]:
                # Use a tuple of sorted items for hashability if dict, or just the file path if available
                key = cex_item.get("file") if isinstance(cex_item, dict) and "file" in cex_item else str(cex_item)
                if key in seen:
                    duplicates.append(key)
                else:
                    seen.add(key)
            if duplicates:
                raise Exception(f"Duplicate CEX entries found in output_sets['cex']: {duplicates}")
            
            covered_num = 0
            logging.info("Checking if the invariant covers all CEXs")
            alll_covered = False
            if not all_covered:
                # Remove from the CEX set the CEX that are covered by the invariant
                # to_remove = []
                # with Pool() as pool:
                #     results = list(tqdm.tqdm(pool.imap(check_invariant, output_sets["cex"]), total=len(output_sets["cex"])))
                #     to_remove = [r[0] for r in results if r is not None]
                #     covered_num = len(to_remove)
                to_remove_filenames = common.identify_covered_cex_items_through_ffi_call(output_sets["cex"], self.invariants_dir_path, enum_to_string=True)
                covered_num = len(output_sets["cex"]) - len(to_remove_filenames)
                #print("To keep", to_keep[0])
                #print("cex_item", output_sets["cex"][0])
                to_remove_filenames = {item["file"] for item in to_remove_filenames}
                #print("To remove filenames", to_remove_filenames)
                for cex_item in list(output_sets["cex"]):
                    if cex_item["file"] in to_remove_filenames:
                        #print("Want to filter out cex item", cex_item)
                        #print("output_sets format", output_sets["cex"][0])
                        output_sets["cex"].remove(cex_item)
                        #self.filtered_cex += 1
                        os.remove(cex_item["file"])
                        os.remove(cex_item[constants.WAVEFORM_PATH_KEY])
                #output_sets["cex"] = to_keep
                # covered_num = len(to_remove)
                # logging.info(f"Found {covered_num} CEXs that are covered by the invariant")
                # for cex_item in to_remove:
                #     #for cex_item in tqdm.tqdm(output_sets["cex"]):
                #     #commit_check_res_second = analyzer.check_commit_log(f"example_cores/compare_to_ibex/obj_dir/Vcorrectness", cex_item["file"], None)
                #     #if commit_check_res_second.Kind != analyzer.CheckerResultKind.CEX:
                #     #    print(f"Line 541 Mismatch between buggy ibex and other ibex. Binary path {cex_item['file']}, check_result {commit_check_res.Kind}, check_result2 {commit_check_res_second.Kind}")
                #     #    injected_bug_found = True
                #         #raise Exception(f"Line 541 Mismatch between buggy ibex and other ibex. Binary path {cex_item['file']}, check_result {commit_check_res.Kind}, check_result2 {commit_check_res_second.Kind}")
                #     os.remove(cex_item["file"])
                #     os.remove(cex_item[constants.WAVEFORM_PATH_KEY])
                #     len_before = len(output_sets["cex"])
                #     print("Will remove cex_item", cex_item)
                #     print("output format", output_sets["cex"][0])
                #     output_sets["cex"].remove(cex_item)
                #     assert len(output_sets["cex"]) == len_before - 1, f"Expected to remove one CEX, but got {len(output_sets['cex'])} instead of {len_before - 1}"
                    
                
                # Now we update the output_sets.json file
                logger.info(f"Removed {covered_num} CEXs from the set, {output_set_size} -> {len(output_sets['cex'])}")
                with open(output_sets_path, "w") as fp:
                    json.dump(output_sets, fp, indent=4, default=str)
                
                if len(output_sets["cex"]) == 0:
                    #raise Exception(f"No more CEXs left but not all_covered is False. This should not happen, if all_covered is False, we should have at least one CEX -- we have {ret_dict['cex_fulfilled_percentage']}% fulfilled. {ret_dict['score']}")
                    logger.error(f"No more CEXs left but not all_covered is False. This should not happen, if all_covered is False, we should have at least one CEX -- we have {ret_dict['cex_fulfilled_percentage']}% fulfilled. {ret_dict['score']} covered num is {covered_num}")
                    logger.error(f"This should not happen, if all_covered is False, we should have at least one CEX -- we have {ret_dict['cex_fulfilled_percentage']}% fulfilled")
                    all_covered = True
                    continue
                    return
                
                # Pick a random CEX from the set as the current CEX
                current_cex = random.choice(output_sets["cex"])["file"]
                logger.info(f"Next CEX is {current_cex}")
                with tempfile.NamedTemporaryFile(delete=False, suffix='.bin') as tmp_bin_file:
                    temp_bin_path_cex = tmp_bin_file.name
                    logger.debug(f"Writing to {temp_bin_path_cex}")
                    shutil.copy(current_cex, temp_bin_path_cex)
                current_cex = temp_bin_path_cex
                new_file_path =  os.path.join(current_logging_directory,f"cex_{self.cex_waveform_counter}_iteration_{iterations_per_cex}_{os.path.basename(current_cex)}")
                shutil.copy(current_cex, new_file_path)
                current_cex_path = new_file_path
                logger.debug(f"Copied next CEX to {temp_bin_path_cex}")
                clean_cexs_directory(self.logfile, self.output_dir)
                iterations_per_cex += 1
                bugs, underspecification_testbenches = self.classify_cex_bug_triggers(current_cex)
                fixed_core_underspecification = any(self._is_fixed_core_key(k, self.bug_verification_testbenches[k]) for k in underspecification_testbenches if k in self.bug_verification_testbenches)
                self.event_logger.log_event(
                    event_type=EventType.picked_new_cex_from_mutations,
                    event_details={"cex_path": current_cex_path},
                    bugs=bugs,
                    underspecification={"all_triggered": underspecification_testbenches, "fixed_core_mismatch": fixed_core_underspecification}
                )
    
    def generate_cex_assumption(self, instructions):
        if self.memory_size == 32:
            exclude_value = "0e800093"
        else:
            exclude_value = "0e8000930e800093"
        assumption = []
        for idx, val in enumerate(instructions):
            if val == exclude_value:
                break
            for idx in range(0, self.current_max_cex_length):
                if self.memory_size == 64:
                    idx_ref = (self.sodor_offset + idx + self.first_symbolic_instruction_idx) // 2
                    memory_signal = self.memory_signal_ref_format_string_jg.format(idx=idx_ref)
                    memory_signal = memory_signal_to_jg_transformation(memory_signal)
                    bitselect = "[31:0]" if (idx % 2 == 0) else "[63:32]"
                    assumption.append(f"({{{memory_signal}{bitselect}}} == 32'h{val})")
                else:
                    idx_ref = self.sodor_offset + idx + self.first_symbolic_instruction_idx
                    memory_signal = self.memory_signal_ref_format_string_jg.format(idx=idx_ref)
                    memory_signal = memory_signal_to_jg_transformation(memory_signal)
                    assumption.append(f"({{{memory_signal}}} == 32'h{val})")
                # output_lines.append(f" {{{memory_signal}{bitselect}}}\n")
                # idx_dut = (self.dut_offset + idx + self.first_symbolic_instruction_idx) // 2
                # memory_signal_dut = self.memory_signal_dut_format_string_jg.format(idx=idx_dut)
                # memory_signal_dut = memory_signal_to_jg_transformation(memory_signal_dut)
                # output_lines.append(f"abstract -init_value {{{memory_signal_dut}{bitselect}}}\n")
                # #boom_restriction = f"boom.dut.TileLink_Memory.mem[{i}] == 64'h{val}"
                # if self.memory_size == 32:
                #     right = f"32'h{val}"
                # else:
                #     right = f"64'h{val}"
                # j = i + self.sodor_offset + self.first_symbolic_instruction_idx // (2 if self.memory_size == 64 else 1)
                # if self.memory_signal_ref_format_string_jg is None:
                #     raise Exception("memory_signal_ref_format_string_jg is None, cannot generate cex assumption")
                # formatted_signal = self.memory_signal_ref_format_string_jg.format(idx=j)
                # if formatted_signal.startswith("correctness."):
                #     formatted_signal = formatted_signal[len("correctness."):]  # Remove "correctness."
                # sodor_restriction = formatted_signal + f" == {right}"
                #sodor_restriction = f"sodor_core.memory.mem_ext.Memory[{i}] == 64'h{val}" 
                #assumption.append(boom_restriction)
                # assumption.append(sodor_restriction)
        joined = ' &&\n    '.join(assumption)
        return f"assume {{!(\n    {joined}\n)}}"       
        
    def add_just_cex_assumption(self, instructions):
        # SIMPLE MODE: Just add CEX as assumption
        logger.info("Using simple CEX assumption mode (no separator)")
        assumption_string = self.generate_cex_assumption(instructions)
        self.jg_connector_obj.add_invariant(f"./example_cores/compare_to_{self.core_name}/generated_files/correctness.tcl", assumption_string)
        self.event_logger.log_event(event_type=EventType.cex_assumption_added, event_details=assumption_string)
        print(assumption_string)
        print("CEX ASSUMPTION ADDED!\n")
    
    def run_inner(self):
        self.jg_connector_obj.set_current_max_cex_length(self.current_max_cex_length)
        self.jg_connector_obj.initialize_waveform_extractor()
        self.generate_tcl_from_template(f"./example_cores/compare_to_{self.core_name}/templates/correctness_template.tcl", f"./example_cores/compare_to_{self.core_name}/templates/invariant_template.tcl")
        self.jg_connector_obj.add_invariants_to_tcl_files( self.invariants_dir_path, f"./example_cores/compare_to_{self.core_name}/generated_files/correctness.tcl", f"./example_cores/compare_to_{self.core_name}/generated_files/invariant.tcl")

        # Sync current setup to JG server
        this_dir  = str(Path(os.path.dirname(self.correctness_tcl_path)).parent)
        self.jg_connector_obj.sync_to_jg_server(this_dir, self.logfile)
        self.cex_waveform_counter = 0
        benign_waveform_counter = 0
        
        # TODO : Implement a better way to structure logging files:
        # Maybe create a new subdirectory for each cex retrieved from jaspergold, and store in the folder:
        # - the cex
        # - every cex subsequently created in the inner loop
        # - the invariant synthesized from the first cex and from all the others
        injected_bug_found = False
        while(True):
            # Run jg on the current correctness.tcl script
            #sync_to_jg_server(self.jg_configs, self.logfile)
            # Always copy the correctness.tcl script to the server before running
            # This way we ensure that any changes to the correctness.tcl script are reflected on the server
            # This changes might be missed by timestamp-dependent rsync, since some timestamps might either not get updated
            # early enough or might not be updated with a fine enough granularity.
            # For instance, zfs batches transactions through transaction groups, 
            # so not every change to a file is immediately reflected in the filesystem timestamp.
            # This happens even if we call f.close()! 
            self.jg_connector_obj.scp_specific_files_to_jg_server(f"./example_cores/compare_to_{self.core_name}/generated_files/correctness.tcl", self.jg_configs["server_correctness_tcl_script_path"])
            self.event_logger.log_event(event_type=EventType.jg_query_started)
            self.jg_connector_obj.run_jg(self.jg_configs["server_correctness_tcl_script_path"], self.logfile)

            self.event_logger.log_event(event_type=EventType.jg_query_completed)


            # Create a subdirectory in the logging folder for this cex
            current_logging_directory = os.path.join(self.logging_directory, f"cex_{self.cex_waveform_counter}")
            os.makedirs(current_logging_directory, exist_ok=True)

            # Retrieve the CEX from the server
            temp_bin_path_cex, logging_cex_vcd_path = self.jg_connector_obj.get_cex_from_server(self.logfile, current_logging_directory)
            if temp_bin_path_cex is None and logging_cex_vcd_path is not None:
                raise Exception(f"temp_bin_path_cex is None but logging_cex_vcd_path is not None")
            if temp_bin_path_cex is None:
                print(f"Did not get any new counterexamples. We are done?")
                return
            if logging_cex_vcd_path is None:
                raise Exception(f"logging_cex_vcds_path is None but temp_bin_path_cex is not None")
            new_logging_cex_vcd_path = logging_cex_vcd_path+f".cex_{self.cex_waveform_counter}.vcd"
            shutil.move(logging_cex_vcd_path, new_logging_cex_vcd_path)
            new_cex_path =  os.path.join(current_logging_directory, f"initial_cex_{self.cex_waveform_counter}_"+os.path.basename(temp_bin_path_cex))
            shutil.copy(temp_bin_path_cex,new_cex_path)
            current_cex_path = new_cex_path
            bugs, underspecification_testbenches = self.classify_cex_bug_triggers(temp_bin_path_cex)
            fixed_core_underspecification = any(self._is_fixed_core_key(k, self.bug_verification_testbenches[k]) for k in underspecification_testbenches if k in self.bug_verification_testbenches)
            self.event_logger.log_event(
                event_type=EventType.cex_found_from_jg,
                event_details={"cex_path": current_cex_path, "waveform_path": new_logging_cex_vcd_path},
                bugs=bugs,
                underspecification={"all_triggered": underspecification_testbenches, "fixed_core_mismatch": fixed_core_underspecification}
            )

            logger.debug(f"Checker is at example_cores/compare_to_{self.core_name}/obj_dir/Vcorrectness")
            vcorrectness_path = None
            if self.checker_path.endswith(".so"):
                # if path is example_cores/compare_to_kronos/build/all_fix/libcorrectness.so
                # then vcorrectness_path is example_cores/compare_to_kronos/build/all_fix/Vcorrectness
                vcorrectness_path = str(Path(self.checker_path).parent / "Vcorrectness")
            else:
                vcorrectness_path = self.checker_path
            commit_check_res = analyzer.check_commit_log(vcorrectness_path, temp_bin_path_cex, None)
            if commit_check_res is None:
                raise Exception(f"Could not check commit log for binary {temp_bin_path_cex}")
            
            if commit_check_res.Kind != analyzer.CheckerResultKind.CEX:
                raise Exception(f"Mismatch between verilator and jaspergold. Binary path {temp_bin_path_cex}, waveform {new_logging_cex_vcd_path}, check result {commit_check_res}")
            
            # Run insight
            if self.use_insight:
                self.run_insight(current_cex_path, current_logging_directory=current_logging_directory, temp_bin_path_cex=temp_bin_path_cex,new_logging_cex_vcd_path=new_logging_cex_vcd_path)
            else:
                instructions =  self.jg_connector_obj.waveform_extractor.extract_instructions(new_logging_cex_vcd_path)
                instructions = [f"{instr:08x}" for instr in instructions]
                self.add_just_cex_assumption(instructions)
                instructions_string = cex_generator.disassemble_and_extract(current_cex_path)
                self.event_logger.log_event(event_type=EventType.cex_assumption_added, event_details=instructions_string)
            self.cex_waveform_counter += 1   
    
    def run(self):
        # Clean up previous runs, compile verilog testbenches, and generate tcl files
        self.cex_generator_instance = cex_generator.CexGenerator(
            output_dir=self.output_dir,
            checker_path=self.checker_path,
            #waveform_dir=os.path.abspath("./waveforms"),
            #mutated_files_dir=os.path.abspath("./mutations"),
            first_symbolic_instruction_idx=self.first_symbolic_instruction_idx,
            start_address=self.start_address
        )
        self.csr_generator = generate_csr_separators.CSRSeparatorGenerator(
            self.checker_path,
            output_dir=self.invariants_dir_path
        )
        invalid_csrs_file = self.csr_generator.run()
        self.seed_invariants.append(invalid_csrs_file)
        self.cex_generator_instance.set_seed_invariants(self.seed_invariants)
        self.event_logger.log_event(EventType.genin_start)
        cleanup(self.logfile)
        clean_cexs_directory(self.logfile, self.output_dir)
        compile_testbenches(self.logfile)
        build_opcodes()
        #clean_invariants_directory()
        #add_seed_invariants(f"./example_cores/compare_to_{self.core_name}/correctness.tcl", f"./example_cores/compare_to_{self.core_name}/invariant.tcl")
        copy_seed_invariants(self.seed_invariants, constants.SEED_INVARIANT_PATH, self.invariants_dir_path)
        if False: #self.memory_size == 64:
            step_size_max_cex_length = 2
        elif self.memory_size in [32,64]:
            step_size_max_cex_length = 1
        else:
            raise Exception(f"Unsupported memory size {self.memory_size}")
        #300+3600+3600+7200+21600+3600 = 
        for (time_limit, cex_length, csr_only, max_trace_length) in [(300,1,True, 80),(7200,2,True, 150),(7200,1, False, 150),(7200,2, False, 180), (21600,3, False, 210),(2400,4, False, 300)]: #[(300,1)]: #, (600,2)
        #for (time_limit, cex_length, csr_only, max_trace_length) in [(300,1,False, 60),(3600,2,False, 110),(7200,3, False, 150),(6900,5, False, 110)]: #[(300,1)]: #, (600,2)
            csr_only_opcode = "7'b1110011"
            other_opcode = "7'b0110011, 7'b0010011, 7'b1100011, 7'b1101111,  7'b1100111, 7'b0110111, 7'b0010111,7'b0001111, 7'b1110011, 7'b0000011, 7'b0100011"
        # for time_limit in [3600, 1800, 3600, 7200]: #60,300
            #constants.CURRENT_TIME_LIMIT = time_limit
            logger.info(f"Set CURRENT_TIME_LIMIT to {time_limit} seconds, cex_length to {cex_length}")
            self.current_max_cex_length = cex_length#1#step_size_max_cex_length
            self.current_time_limit = time_limit
            self.event_logger.log_event(event_type=EventType.time_limit_increased, event_details=time_limit)
            self.event_logger.log_event(event_type=EventType.program_length_increased, event_details=str(self.current_max_cex_length))
            self.max_trace_length = max_trace_length
            # while self.current_max_cex_length <= constants.MAX_CEX_LENGTH:
            constants.CURRENT_CEX_LENGTH = self.current_max_cex_length
            logger.info(f"Running inner loop with CURRENT_CEX_LENGTH {self.current_max_cex_length}")
            self.cex_generator_instance.max_program_length = self.current_max_cex_length
            # if self.memory_size == 2: #64: #for boom...
            #     opcodes = ["1110011", "0110011", "0010011", "1100011",                      "0000011", "0100011", "1101111",                       "1100111", "0110111", "0010111",                       "0001111" ]
            # else:
            #     opcodes = [None]
            # for test_opcode in opcodes:
            # self.current_opcode = test_opcode
            if csr_only:
                self.current_opcode = csr_only_opcode
            else:
                self.current_opcode = other_opcode
            test_opcode = self.current_opcode
            try:
                self.run_inner()
            except Exception as e:
                logger.error(f"Exception in run_inner with CURRENT_CEX_LENGTH {self.current_max_cex_length}: {e}")
                raise e
            self.event_logger.log_event(event_type=EventType.opcode_completed, event_details=test_opcode)
            print(f"Incrementing CURRENT_CEX_LENGTH to {self.current_max_cex_length + step_size_max_cex_length}", flush=True)
            self.current_max_cex_length += step_size_max_cex_length
            

        self.logfile.close()


def main():

    # if len(sys.argv) < 2:
    #     print("Usage: python scripts/run_full_pipeline.py <path_to_config_file>")
    #     return

    # with open(sys.argv[1], "r") as f:
    #     configs = json.load(f)
    parser = argparse.ArgumentParser(description="Run the full CEX/invariant pipeline.")
    parser.add_argument("--config", required=True, help="Path to config file")
    parser.add_argument("--bex-weight", type=int, default=50, help="Weight for benign examples (default: 50)")
    parser.add_argument("--predicate-cost", type=int, default=10, help="Base cost of a predicate (cost/1000 is the multiplier, default: 10)")
    parser.add_argument("--no-insight", action="store_true", help="Use simple CEX assumptions instead of insight separator", default=False)
    parser.add_argument("--sweep", action="store_true", help="Sweep over bex-weight and predicate-cost values instead of using fixed ones", default=False)
    parser.add_argument("--continue-from", type=str, help="Continue from a previous run in the specified output directory", default=None)
    parser.add_argument("--mock-cex-path", type=str, help="(For testing) Path to a mock CEX file to use instead of running JasperGold", default=None) 
    print("Running full pipeline with args:", ' '.join(sys.argv), flush=True)
    args = parser.parse_args()
    with open(args.config, "r") as f:
        configs = json.load(f)
    bex_weight = args.bex_weight
    predicate_cost = args.predicate_cost
    use_insight = not(args.no_insight)
    if not use_insight and args.sweep:
        raise Exception("Cannot use --sweep with --no-insight, as sweeping makes sense only with insight/separator")
    if args.continue_from is not None and not args.sweep:
        raise Exception("Cannot use --continue-from without --sweep, as continuing makes sense only when sweeping")
    jg_configs = configs["jg_server_configs"]
    output_dir = configs["output_dir"]
    regex_config_path = configs["regex_config_path"]
    core_name = configs["core_name"]
    output_dir = os.path.join(output_dir, core_name)
    if use_insight:
        output_dir = os.path.join(output_dir, f"bex_weight_{bex_weight}_predicate_cost_{predicate_cost}")
    else:
        output_dir = os.path.join(output_dir, f"no_insight_output")
    #print("Output dir", output_dir, flush=True)
    os.makedirs(output_dir, exist_ok=True)
    checker_path = configs["verilator_script"]
    with open(configs["regex_config_path"], "r") as f:
        regex_config = json.load(f)
    sodor_offset = regex_config.get("sodor_offset", 32)
    memory_format_string_ref = regex_config.get("memory_format_string_ref", "correctness.sodor_core.memory.mem_ext.Memory[{idx}][63:0]")
    memory_format_string_dut = regex_config["memory_format_string_dut"]
    memory_size = regex_config.get("memory_size", 64)
    first_symbolic_instruction_idx = regex_config.get("first_symbolic_instruction_idx", 0)
    start_address = int(regex_config.get("start_address", "0x80000000"), 16)
    print("Will use sodor_offset", sodor_offset, "and memory_format_string", memory_format_string_ref, flush=True)
    inject_jg_connector_obj = None
    if args.mock_cex_path is not None:
        print(f"Using mock CEX path {args.mock_cex_path} instead of real JasperGold connection")
        inject_jg_connector_obj = MockJGConnector(jg_config=jg_configs, config=configs)
        cex_paths = os.listdir(args.mock_cex_path)
        cex_paths = [os.path.join(args.mock_cex_path, f) for f in cex_paths if f.endswith(".bin")]
        if len(cex_paths) == 0:
            raise Exception(f"No .bin files found in mock CEX path {args.mock_cex_path}")
        inject_jg_connector_obj.set_available_cex(cex_paths)
        inject_jg_connector_obj.set_current_max_cex_length(1) #dummy
        inject_jg_connector_obj.initialize_waveform_extractor()
        inject_jg_connector_obj.set_checker_path(checker_path)
        inject_jg_connector_obj.generate_cex_waveforms()
    seed_invariants = configs.get("seed_invariants", [])
    bug_verification_testbenches = configs.get("bug_verification_testbenches", {})
    if not isinstance(bug_verification_testbenches, dict):
        raise Exception("bug_verification_testbenches must be a dictionary mapping bug keys to testbench paths")

    pipeline_loop = PipelineLoop(
        jg_configs,
        regex_config_path,
        core_name,
        checker_path,
        seed_invariants=seed_invariants,
        inject_jg_connector=inject_jg_connector_obj,
        bug_verification_testbenches=bug_verification_testbenches,
    )
    pipeline_loop.set_refcore_offset(sodor_offset, memory_format_string_ref)
    pipeline_loop.set_dut_offset(sodor_offset, memory_format_string_dut)
    pipeline_loop.set_memory_size(memory_size)
    if "conditional_signals_mapping_path" in configs:
        pipeline_loop.set_conditional_signals_to_condition_mapping(configs["conditional_signals_mapping_path"])
    pipeline_loop.set_first_symbolic_instruction_idx(first_symbolic_instruction_idx)
    pipeline_loop.set_start_address(start_address)
    pipeline_loop.use_insight = use_insight

    if not args.sweep:
        print(f"Running with fixed bex_weight {bex_weight} and predicate_cost {predicate_cost} output_dir {output_dir}", flush=True)
        pipeline_loop.setup_output_directories(output_dir)
        pipeline_loop.set_bex_weight(bex_weight)
        pipeline_loop.set_predicate_cost(predicate_cost)
        pipeline_loop.run()
    else:
        print(f"Running sweep over bex_weight and predicate_cost")
        current_datetime = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
        output_dir_base = os.path.join(configs["output_dir"], core_name, f"sweep_{current_datetime}")
        start_bex = 90 #0
        start_predicate_cost = 10 #5 #1
        if args.continue_from is not None:
            if not os.path.exists(args.continue_from):
                raise Exception(f"continue_from directory {args.continue_from} does not exist")
            
            output_dir_base = args.continue_from
            #dir_name = os.path.basename(args.continue_from.strip("\\"))
            
            #match = re.match(r"sweep_bex_weight_(\d+)_predicate_cost_(\d+)", dir_name)
            #if match is None:
            #    raise Exception(f"Could not parse bex_weight and predicate_cost from directory name {dir_name}")
            #start_bex = int(match.group(1))
            #start_predicate_cost = int(match.group(2))
            start_bex, start_predicate_cost = find_last_checkpoint_in_sweep_dir(output_dir_base)
            print(f"Will start sweep from bex_weight {start_bex} and predicate_cost {start_predicate_cost}", flush=True)
            if start_bex is None:
                start_bex = 90
            if start_predicate_cost is None:
                start_predicate_cost = 2
            print(f"Continuing from previous run in {args.continue_from}, setting output_dir_base to {output_dir_base} and will skip to bex_weight/predicate_cost in {start_bex}, {start_predicate_cost}", flush=True)
            start_predicate_cost = 25
        for sweep_bex_weight in range(start_bex, 125, 5):
            for sweep_predicate_cost in range(start_predicate_cost, 50, 10):
                output_dir_sweep = os.path.join(output_dir_base, f"sweep_bex_weight_{sweep_bex_weight}_predicate_cost_{sweep_predicate_cost}")
                pipeline_loop.setup_output_directories(output_dir_sweep)
                pipeline_loop.output_dir = output_dir_sweep
                os.makedirs(output_dir_sweep, exist_ok=True)
                print(f"Running with sweep_bex_weight {sweep_bex_weight} and sweep_predicate_cost {sweep_predicate_cost}", flush=True)
                pipeline_loop.set_bex_weight(sweep_bex_weight)
                pipeline_loop.set_predicate_cost(sweep_predicate_cost)
                pipeline_loop.run()

if __name__ == "__main__":
    seed_generator.ensure_seeds_exist()
    main()
