import subprocess
import os
import tempfile
import shutil
import sys
import copy
from pathlib import Path
subdir = Path(__file__).parent.parent
sys.path.append(str(subdir))

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))

subdir = Path(__file__).parent.parent / "formal-verif" / "invariant_generation" / "vincent_invariant_generator"
sys.path.append(str(subdir))
import json
import constants
import uuid
import datetime
import analyzer
import vcd_trace
import cffi
import logging
import random

import tqdm
import generate_csr_separators

subdir = Path(__file__).parent.parent / "mutation_engine"
sys.path.insert(0, str(subdir))
subdir = Path(__file__).parent
sys.path.insert(0, str(subdir))

from riscv_instruction_mutator import FileSource
from multiprocessing import Pool
from concurrent.futures import ThreadPoolExecutor
import cex_generator
import re
from logging_common import setup_logging
import cProfile
import pstats
import signal
import time
import argparse
import functools

logger = logging.getLogger("main")

FREEZE_INSTRUCTIONS = list(range(0,39))

def find_last_checkpoint_in_sweep_dir(sweep_dir):
    pattern = re.compile(r"sweep_bex_(\d+)_predcost_(\d+)")
    
    max_bex_weight = None
    max_predicate_cost = None
    
    for entry in os.listdir(sweep_dir):
        print(f"Checking entry {entry} in sweep dir {sweep_dir}", flush=True)
        match = pattern.match(entry)
        if match:
            bex_weight = int(match.group(1))
            predicate_cost = int(match.group(2))
            print(f"Found checkpoint: bex_weight={bex_weight}, predicate_cost={predicate_cost}", flush=True)
            if max_bex_weight is None or bex_weight > max_bex_weight:
                max_bex_weight = bex_weight
            if max_predicate_cost is None or predicate_cost > max_predicate_cost:
                max_predicate_cost = predicate_cost

    return max_bex_weight, max_predicate_cost

#
#allcex <- all found by fuzzer
# while allcex is not empty:
#     this_cex <- pick one cex from allcex
#     this_separator <- synthesize separator for this_cex using insight (fuzz this_cex then run rust code9
#     remove all cex which fulfilled this_separator and classify them as one bug class
# def clean_cexs_directory(logfile=subprocess.DEVNULL):
#     logger.info("Cleaning up cexs directory")
#     subprocess.run(["rm -rf output/cexs"], stdout=logfile, stderr=logfile, shell=True)
#     subprocess.run(["rm -f output/input_cex.bin output/minimized_cex.bin"], stdout=logfile, stderr=logfile, shell=True)

def clean_cexs_directory(logfile, output_dir):
    logger.info("Cleaning up cexs directory")
    output_dir_cex = os.path.join(output_dir, "cexs")
    output_dir_benign_examples = os.path.join(output_dir, "benign_examples")
    # subprocess.run(["rm -rf output/cexs"], stdout=logfile, stderr=logfile, shell=True)
    #subprocess.run(["rm -f input_cex.bin minimized_cex.bin"], stdout=logfile, stderr=logfile, shell=True)
    #shutil.rmtree("output/benign_examples/", ignore_errors=True)
    shutil.rmtree(output_dir_cex, ignore_errors=True)
    shutil.rmtree(output_dir_benign_examples, ignore_errors=True)
    

def clean_invariants_directory(logfile=subprocess.DEVNULL):
    logger.info("Cleaning up invariants directory")
    subprocess.run(["rm -rf " + constants.INVARIANT_PATH], stdout=logfile, stderr=logfile, shell=True)
    os.makedirs(constants.INVARIANT_PATH, exist_ok=True)
    shutil.copy("configs/global_seed_invariants/valid_only.json", os.path.join(constants.INVARIANT_PATH, "valid_only.json"))

def copy_seed_invariants(seed_invariants, to_dir):
    os.makedirs(to_dir, exist_ok=True)
    for seed_name in seed_invariants:
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

# def check_invariant(cex):
#     # start_time = time.time()
#     ffi_invariant_vec_ptr = ffi.cast("void *", cex["invariant_dict_ptr"])
#     ret_idx = analyzer.waveform_fulfills_any_invariant_from_list_return_idx(cex[constants.WAVEFORM_PATH_KEY], ffi_invariant_vec_ptr)
#     if ret_idx == -1:
#         return None
#     else:
#         fulfilled_invariant = cex["invariant_dict"][ret_idx]
#         #print("Vincent CEX {cex['file']} is covered by invariant {fulfilled_invariant}", flush=True)
#         return (cex["file"], fulfilled_invariant)
#     #potential_invariants = analyzer.waveform_fullfills_any_invariant(cex[constants.WAVEFORM_PATH_KEY])
#     # end_time = time.time()
#     # print(f"Vincent Time taken to check invariant satisfaction: {end_time - start_time} seconds", flush=True)
#     #potential_invariants = analyzer.waveform_fullfills_any_invariant(cex[constants.WAVEFORM_PATH_KEY])
#     #if potential_invariants is not None:
#     #    return (cex["file"], potential_invariants)
#     #return None


def compile_rust_ffi_library():
    subprocess.run(["cargo", "build", "--release"], cwd="./formula_finder/", stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    #shutil.copy("./formal-verif/invariant_generation/invariant_finder_rust/target/release/libinvariant_finder_rust.so", constants.INVARIANT_FINDER_LIBRARY_PATH)
compile_rust_ffi_library()

ffi = cffi.FFI()
rust_finder_library = ffi.dlopen(constants.INVARIANT_FINDER_LIBRARY_PATH)
ffi.cdef('bool ffi_check_on_waveform(char *waveform_path, char *invariant_json_path, char *clock_signal);')
ffi.cdef('const char *ffi_find_invariant(const char *output_sets_path, const char *regex_config_path, uint64_t bex_weight, uint64_t predicate_cost);')
ffi.cdef("void ffi_free_library_string(const char *ptr);")
ffi.cdef("void *ffi_set_bex_multiplier(uint64_t multiplier);")
ffi.cdef('char *check_cex_items_all_invariants(const char *cex_items, const char *invariant_dict_json);')

def check_one_invariant(cex, invariant_json_path, clock_signal="TOP.correctness.clk"):
    result = rust_finder_library.ffi_check_on_waveform(
        cex.encode('utf-8'),
        str(invariant_json_path).encode('utf-8'),
        clock_signal.encode('utf-8')
    )
    return result


def check_all_cex_items(cex_items, ffi_invariant_dict_ptr):
    cex_items_for_rust = []
    for cex_item in cex_items:
        cex_items_for_rust.append({
            "file": cex_item,
            "waveform_path": cex_item + ".vcd"
        })
    cex_items_json = json.dumps(cex_items_for_rust)
    invariant_check_results_json = rust_finder_library.check_cex_items_all_invariants(cex_items_json.encode("utf-8"), ffi_invariant_dict_ptr)
    invariant_check_results = json.loads(ffi.string(invariant_check_results_json).decode("utf-8"))
    print(f"Got invariant check results for {len(invariant_check_results)} cex items", flush=True)
    rust_finder_library.ffi_free_library_string(ffi_invariant_dict_ptr)
    rust_finder_library.ffi_free_library_string(invariant_check_results_json)
    bug_classes = {}
    for item in invariant_check_results:
        #print(item)
        cex_item = item[0]
        invariants_idx = item[1]
        for idx in invariants_idx:
            if idx not in bug_classes:
                bug_classes[idx] = []
            bug_classes[idx].append(cex_item)
    return bug_classes



def run_separator(output_sets_path, regex_config_path, bex_multiplier, predicate_base_cost):
    """
    regex_config_path: A json file that specifies 
    the prioritizes of the signals. We process in stages. For each stage,
    give a name and a list of regexes that a signal can fulfill.
    """
    #rust_finder_library.ffi_set_bex_multiplier(constants.BEX_MULTIPLIER)
    if type(output_sets_path) == str:
        logger.debug("I need to convert output_sets_path to string to call ffi")
        output_sets_path = output_sets_path.encode()
    if type(regex_config_path) == str:
        logger.debug("I need to convert regex_config_path to string to call ffi")
        regex_config_path = regex_config_path.encode()
    ret_string = ffi.NULL
    try:
        ret_string = rust_finder_library.ffi_find_invariant(output_sets_path, regex_config_path, bex_multiplier, predicate_base_cost)
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

def run_cex_generator(cex_path, output_dir, checker_path, seed_invariants):
    logger.info("Cleaning up mutations and waveforms directories")
    # shutil.rmtree("mutations", ignore_errors=True)
    # shutil.rmtree("waveforms", ignore_errors=True)

    # #os.makedirs("mutations", exist_ok=True)
    # #os.makedirs("waveforms", exist_ok=True)
    # output_dir = os.path.join(os.path.abspath("output"), "kronos_deduplication")
    cex_generator_instance = cex_generator.CexGenerator(
        output_dir=output_dir,
        checker_path=checker_path,
        #waveform_dir=os.path.abspath("waveforms"),
        #mutated_files_dir=os.path.abspath("mutations"),
        log_level="INFO",
        first_symbolic_instruction_idx=0,
        max_program_length=20_000,
        #additional_seed_dirs=["dedup_seeds/"]
    )
    #cex_generator_instance.set_freeze_instructions(FREEZE_INSTRUCTIONS)
    cex_generator_instance.set_seed_invariants(seed_invariants)
    return cex_generator_instance.run_cex_generator(cex_path, minimize=True)

def check_cex_false_positive(checker_path, cex):
    commit_check_res = analyzer.check_commit_log(checker_path, cex, cex + ".vcd")
    if commit_check_res.Kind != analyzer.CheckerResultKind.CEX:
        tqdm.tqdm.write(f"{cex} is a false positive, skipping")
        return (cex, "BEX")
    else:
        return (cex, "CEX")
    
def generate_frozen_instruction_benign_example(all_cexs, output_dir, checker_path):
    result_list = []
    def process_cex(cex):
        freezed_instruction_bin = os.path.join(output_dir, f"freeze_instructions_{os.path.basename(cex)}.bin")
        input_bin = cex
        with open(freezed_instruction_bin, "wb") as f_out:
            with open(input_bin, "rb") as f_in:
                f_out.write(f_in.read()[FREEZE_INSTRUCTIONS[0]*4:FREEZE_INSTRUCTIONS[-1]*4 +4])
        ret = analyzer.check_commit_log(checker_path, freezed_instruction_bin, freezed_instruction_bin + ".fst")
        if ret.Kind != analyzer.CheckerResultKind.BENIGN:
            raise Exception(f"Could not generate frozen instruction benign example for CEX {cex}, something is wrong!")
        return (freezed_instruction_bin, freezed_instruction_bin + ".fst")
    with ThreadPoolExecutor() as executor:
        result_list = list(tqdm.tqdm(executor.map(process_cex, all_cexs[:80]), total=len(all_cexs[:80]), desc="Generating frozen instruction benign examples"))
        # result_list = list(pool.map(process_cex, all_cexs))
    return result_list
        

def inner_loop(all_cexs, output_dir, bex_multiplier, predicate_base_cost, benign_example_list, checker_path, conditional_signals_mapping_path, regex_config_path, seed_invariants):
    invariant_dir = os.path.join(output_dir, constants.INVARIANT_PATH)
    if not os.path.exists(invariant_dir):
        os.makedirs(invariant_dir, exist_ok=True)
    ffi_invariant_vec_ptr = analyzer.get_invariant_objects(invariant_dir)
    print("Got invariant objects pointer", ffi_invariant_vec_ptr, flush=True)
    ffi_invariant_vec_ptr_as_int = int(ffi.cast("uintptr_t", ffi_invariant_vec_ptr))
    print("Got invariant objects pointer as int", ffi_invariant_vec_ptr_as_int, flush=True)
    #ffi_invariant_vec_ptr = ffi.cast("void *", ffi_invariant_vec_ptr_as_int)
    #print("Got invariant objects pointer", ffi_invariant_vec_ptr, flush=True)
    ffi_invariant_vec = json.loads(ffi.string(ffi_invariant_vec_ptr).decode("utf-8"))
    print(f"Got {len(ffi_invariant_vec)} invariant objects", flush=True)
    with open(conditional_signals_mapping_path, "r") as f:
        cond_mapping = json.load(f)
    
    
    #exit(0)
    
    # Check initial invariant satisfaction:
    # check_invariant_args = [{"file": cex, "waveform_path": cex + ".vcd", "invariant_dict": ffi_invariant_vec, "invariant_dict_ptr": ffi_invariant_vec_ptr_as_int} for cex in all_cexs]
    # invariant_dict = {}
    # with Pool() as pool:
    #     results = list(tqdm.tqdm(pool.imap(check_invariant, check_invariant_args), total=len(check_invariant_args), desc="Initial invariant check"))
    #     to_remove = [r[0] for r in results if r is not None]
    #     for r in results:
    #         if r is not None:
    #             #print("CEX", r[0], "is covered by invariant", r[1], flush=True)
    #             if r[1]["path"] not in invariant_dict:
    #                 invariant_dict[r[1]["path"]] = 1
    #             else:
    #                 invariant_dict[r[1]["path"]] += 1
    # rust_finder_library.ffi_free_library_string(ffi_invariant_vec_ptr)
    
    # for inv, count in invariant_dict.items():q
    #     logger.info(f"Vincent Invariant {inv} covers {count} CEXs")
    bug_classes = check_all_cex_items(all_cexs, ffi_invariant_vec_ptr)
    with open(os.path.join(output_dir, "current_bug_classes.json"), "w") as fp:
        json.dump(bug_classes, fp, indent=4)
    to_remove = bug_classes.values()
    to_remove = set([item["file"] for sublist in to_remove for item in sublist])
    # print("all_cex", all_cexs)
    new_bexs = []
    for cex_item in all_cexs.copy():
        if cex_item in to_remove:
            all_cexs.remove(cex_item)
            new_bexs.append(cex_item)

    logger.info(f"Vincent Found {len(to_remove)} CEXs already covered by existing invariants, removing them from the list. Left with {len(all_cexs)} CEXs to process.")

    if not all_cexs:
        logger.info("No CEXs to process after initial invariant check. Exiting.")
        return

    logger.info(f"Checking all_cex, first cex to check is {all_cexs[0]}")
    bug_classes = {}
    #with open("bug_classes.json", "w") as f:
    #    json.dump(bug_classes, f, indent=4)
    #    logger.info("Initialized bug_classes.json")

    cex_waveform_counter = 0


    while all_cexs:
        this_cex = all_cexs[0]
        
        ret_dict = None
        iterations_per_cex = 0

        logger.info(f"Running cex generator on current cex iteration {iterations_per_cex} for {this_cex}")
        output_sets = run_cex_generator(this_cex, output_dir, checker_path,seed_invariants)
        output_sets_path = os.path.join(output_dir, "output_sets.json")
        output_sets["conditional_signals_to_condition_mapping"] = cond_mapping

        for new_bex in new_bexs: #TODO: Remove the continue if you the bex to be appended. But then code will be very slwo!
            continue 
            output_sets["bex"].append(
                {
                    "file": new_bex,
                    "waveform_path": new_bex + ".vcd",
                    "program_distance": 0,
                    "file_source": "FileSource.MustFulfill"
                }
            )
        # for remaining_cex in all_cexs:
        #     if remaining_cex == this_cex:
        #         continue
        #     output_sets["cex"].append(
        #         {
        #             "file": remaining_cex,
        #             "waveform_path": remaining_cex + ".vcd",
        #             "program_distance": 0,
        #             "file_source": "FileSource.Mutations"
        #         }
        #     )
        
        # Also: Extract freeze_instructions from cex_generator, run it, get waveform, and add to output_sets
        freezed_instruction_bin = os.path.join(output_dir, "freeze_instructions.bin")
        input_bin = this_cex
        with open (freezed_instruction_bin, "wb") as f_out:
            with open (input_bin, "rb") as f_in:
                f_out.write(f_in.read()[FREEZE_INSTRUCTIONS[0]*4:FREEZE_INSTRUCTIONS[-1]*4 +4])
        ret = analyzer.check_commit_log(checker_path, freezed_instruction_bin, freezed_instruction_bin + ".fst")
        if ret.Kind != analyzer.CheckerResultKind.CEX:
            logger.info(f"Generated frozen instruction benign example for CEX {this_cex}, adding to output_sets")
            output_sets["bex"].append(
                {
                    "file": freezed_instruction_bin,
                    "waveform_path": freezed_instruction_bin + ".fst",
                    "program_distance": 0,
                    "file_source": "FileSource.MustFulfill"
                }
            )
        else:
            raise Exception(f"Could not generate frozen instruction benign example for CEX {this_cex}, something is wrong!")
        # output_sets["bex"].append(
        #     {
        #         "file": "seeds/must_fulfill_waveforms/must_fulfill.bin",
        #         "waveform_path": "seeds/must_fulfill_waveforms/must_fulfill.fst",
        #         "program_distance": 0,
        #         "file_source": "FileSource.MustFulfill"
        #     }
        #)
        # for benign_example in benign_example_list[:80]:
        #     output_sets["bex"].append(
        #         {
        #             "file": benign_example[0],
        #             "waveform_path": benign_example[1],
        #             "program_distance": 0,
        #             "file_source": "FileSource.MustFulfill"
        #         }
        #     )
        with open(output_sets_path, "w") as f:
            json.dump(output_sets, f, indent=4, default=str)
        #logger.info(f"Saved output_sets.json with {len(out_dict['cex'])} CEXs and {len(out_dict['bex'])} benign waveforms")
        #with open(output_sets_path, "r") as f:
        #    output_sets = json.load(f)
        #logger.info(f"Got {len(output_sets['cex'])} CEXs and {len(output_sets['bex'])} benign waveforms")
        if len(output_sets["cex"]) == 0:
            raise Exception(f"No CEXs found in output_sets for this_cex {this_cex}, cannot proceed. Should we not have at least one?")
            return
        
        ret_dict = run_separator(
            output_sets_path=output_sets_path,
            regex_config_path=regex_config_path,
            bex_multiplier=bex_multiplier,
            predicate_base_cost=predicate_base_cost
        )
        logger.info(f"Found separator: {ret_dict['assume_invariant']}, fulfills {ret_dict['cex_fulfilled_percentage']}% of the CEXs, for CEX {this_cex}")
        minimized_cex_text = ret_dict.get("input_cex")
        minimized_cex_text = minimized_cex_text["minimized_instructions"] if minimized_cex_text is not None else None
        if minimized_cex_text is not None and len(minimized_cex_text) > 5:
            minimized_cex_text = minimized_cex_text[-5:]
        ret_dict["input_cex"] = {
            "filepath": this_cex,
            "minimized_instructions": minimized_cex_text
        }
        invariant_filepath = os.path.join(invariant_dir, f"invariant_cex_{cex_waveform_counter}_{str(uuid.uuid4())[:6]}.json")
        with open(invariant_filepath, "w") as f:
            json.dump(ret_dict, f, indent=4)

        output_set_size = len(output_sets["cex"])
        covered_num = 0
        res = check_one_invariant(this_cex + ".vcd", invariant_filepath)
        if res:
            #covered_num += 1
            logger.info(f"Vincent Found that the synthesized invariant indeed covers the CEX {this_cex}")
        else:
            raise Exception(f"Vincent Synthesized invariant does NOT cover the CEX {this_cex}, something is wrong!")
        
        

        clean_cexs_directory(logfile=subprocess.DEVNULL, output_dir=output_dir)
        # At this point, we have a separator that covers all CEXs generated from this_cex
        # We should now identify all CEXs in all_cexs that are covered by this separator
        # and classify them as one bug class

        # Check invariant satisfaction for every CEX in all_cexs
        logger.info(f"Checking invariant satisfaction for all CEXs in all_cexs")
        ffi_invariant_vec_ptr = analyzer.get_invariant_objects(invariant_dir)
        print("Got invariant objects pointer", ffi_invariant_vec_ptr, flush=True)
        ffi_invariant_vec_ptr_as_int = int(ffi.cast("uintptr_t", ffi_invariant_vec_ptr))
        print("Got invariant objects pointer as int", ffi_invariant_vec_ptr_as_int, flush=True)
        #ffi_invariant_vec_ptr = ffi.cast("void *", ffi_invariant_vec_ptr_as_int)
        #print("Got invariant objects pointer", ffi_invariant_vec_ptr, flush=True)
        ffi_invariant_vec = json.loads(ffi.string(ffi_invariant_vec_ptr).decode("utf-8"))
        # # print(f"Got {len(ffi_invariant_vec)} invariant objects", flush=True)
        # # check_invariant_args = [{"file": cex, "waveform_path": cex + ".vcd", "invariant_dict": ffi_invariant_vec, "invariant_dict_ptr": ffi_invariant_vec_ptr_as_int} for cex in all_cexs]
        # # with Pool() as pool:
        # #     results = list(tqdm.tqdm(pool.imap(check_invariant, check_invariant_args), total=len(check_invariant_args)))
        # #     to_remove = [r[0] for r in results if r is not None]
        # #     covered_num = len(to_remove)
        # #     logger.info(f"Vincent Found {covered_num} CEXs covered by the invariant {ret_dict['assume_invariant']}")
        # rust_finder_library.ffi_free_library_string(ffi_invariant_vec_ptr)
        # bug_classes[ret_dict["assume_invariant"]] = []
        # for cex_item in to_remove:
        #     new_bexs.append(cex_item)
        #     all_cexs.remove(cex_item)
        #     bug_classes[ret_dict["assume_invariant"]].append(cex_item)
        bug_classes = check_all_cex_items(all_cexs, ffi_invariant_vec_ptr)
        ret_values = list(bug_classes.values())
        #print("all cex is", all_cexs[0])
        #print("ret values is", ret_values, flush=True)
        to_remove = set()
        for ret_list in ret_values:
            for entry in ret_list:
                #print("Revoming entries", entry)
                #print("Revoming ret_list", ret_list, flush=True)
                to_remove.add(entry["file"])
        before_len = len(all_cexs)
        for cex_item in all_cexs.copy():
            if cex_item in to_remove:
                new_bexs.append(cex_item)
                all_cexs.remove(cex_item)
                # if ret_dict["assume_invariant"] not in bug_classes:
                #     bug_classes[ret_dict["assume_invariant"]] = []
                # bug_classes[ret_dict["assume_invariant"]].append(cex_item)
        print("\n Removed entries, left with", len(all_cexs), "cexs")
        with open(f"{output_dir}/bug_classes.json", "w") as f:
            json.dump(bug_classes, f, indent=4)
            logger.info(f"Saved bug classes to {output_dir}/bug_classes.json with {len(bug_classes)} classes.")

        cex_waveform_counter += 1

    logger.info(f"All CEXs processed.")
    

def main():
    parser = argparse.ArgumentParser(description="Deduplicate CEXs and classify bug classes.")
    parser.add_argument("--config", required=True, help="Path to config file")
    parser.add_argument("--cexs-dir", required=False, help="Path to the directory containing CEX files (optional, derived from config if not provided)")
    parser.add_argument("--continue-from", type=str, help="Continue from a previous run in the specified output directory", default=None)
    args = parser.parse_args()

    with open(args.config, "r") as f:
        configs = json.load(f)
    
    output_dir_root = configs["output_dir"]
    core_name = configs["core_name"]
    checker_path = configs["verilator_script"] 
    regex_config_path = configs["regex_config_path"]
    conditional_signals_mapping_path = configs.get("conditional_signals_mapping_path")
    seed_invariants = configs.get("seed_invariants", [])

    output_dir = os.path.join(output_dir_root, core_name, "deduplication")
    
    cexs_dir = args.cexs_dir
    if cexs_dir is None:
        if os.path.exists(output_dir):
            possible_cexs_dirs = []
            for d in os.listdir(output_dir):
                potential_cexs = os.path.join(output_dir, d, "cexs")
                if os.path.isdir(potential_cexs):
                    possible_cexs_dirs.append(potential_cexs)
            
            if len(possible_cexs_dirs) == 1:
                cexs_dir = possible_cexs_dirs[0]
                logger.info(f"Auto-detected CEX directory: {cexs_dir}")
            elif len(possible_cexs_dirs) > 1:
                cexs_dir = max(possible_cexs_dirs, key=os.path.getmtime)
                logger.info(f"Multiple runs found. Using the most recent CEX directory: {cexs_dir}")

        if cexs_dir is None:
            raise Exception(f"Could not automatically determine CEX directory in {output_dir}. Please specify --cexs-dir.")

    if not os.path.exists(cexs_dir):
         raise Exception(f"CEX directory {cexs_dir} does not exist")
    
    dedup_classification_path = os.path.join(cexs_dir, "dedup-classification.json")
    if os.path.exists(dedup_classification_path):
        # If it exists, load it and save the ones with "CEX" as value in all_cexs
        with open(dedup_classification_path, "r") as f:
            dedup_classification = json.load(f)
            all_cexs = [cex for cex, value in dedup_classification.items() if value == "CEX"]
    else:
        dedup_classification = {}
        # First of all, rule out false positives
        # All cexs are in binary format
        all_cexs = []
        for root, dirs, files in os.walk(cexs_dir):
            for file in files:
                if file.endswith(".bin"):
                    file_path = os.path.join(root, file)
                    all_cexs.append(file_path)

        if not all_cexs:
            print("No cex files found in the specified directory.")
            sys.exit(1)
        logger.info(f"Found {len(all_cexs)} cex files in {cexs_dir}")

        with Pool() as pool:
             results = list(tqdm.tqdm(pool.imap(functools.partial(check_cex_false_positive, checker_path), all_cexs), total=len(all_cexs), desc="False positive check"))

        false_positives = set()
        for cex, result in results:
            dedup_classification[cex] = result
            if result == "BEX":
                false_positives.add(cex)

        with open(dedup_classification_path, "w") as f:
            json.dump(dedup_classification, f, indent=4)
            logger.info("Saved dedup_classification.json with initial CEX classification.")
        all_cexs = [cex for cex in all_cexs if cex not in false_positives]

    logger.info(f"After ruling out false positives, {len(all_cexs)} cex files remain.")
    all_cexs = [os.path.join(cexs_dir, os.path.basename(cex)) for cex in all_cexs]
    current_datime = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
    #sweep_dir = f"evaluation_data/kronos/deduplication/bex_multiplier_sweep/sweep_{current_datime}"
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
            start_bex = 10
        if start_predicate_cost is None:
            start_predicate_cost = 0
        print(f"Continuing from previous run in {args.continue_from}, setting output_dir_base to {output_dir_base} and will skip to bex_weight/predicate_cost in {start_bex}, {start_predicate_cost}", flush=True)
        sweep_dir_base = output_dir_base
    else:
        sweep_dir_base = f"{output_dir}/deduplication_{current_datime}"
        start_bex = 1
        start_predicate_cost = 2
        
    os.makedirs(sweep_dir_base, exist_ok=True)
    logger.info(f"Created sweep base directory at {sweep_dir_base}")
    os.makedirs(os.path.join(sweep_dir_base, "frozen_benign_examples"), exist_ok=True)
    benign_example_list = generate_frozen_instruction_benign_example(all_cexs, os.path.join(sweep_dir_base, "frozen_benign_examples"), checker_path)
    #benign_example_list = []
    # for f in os.listdir(os.path.join("seeds/must_fulfill_waveforms/")):
    #     if f.endswith(".fst"):
    #         benign_example_list.append((
    #             os.path.join("seeds/must_fulfill_waveforms/", f.strip(".fst")+".bin"),
    #             os.path.join("seeds/must_fulfill_waveforms/", f )
    #         ))
        
    #for bex_multiplier in [0,1,2,5,10,15,20,25,50,100,250,500,1000,1500,2000,3000]:
    for bex_multiplier in [1, 25, 50, 75, 100]:
        for predicate_base_cost in [50]:
            if bex_multiplier < start_bex:
                continue
            if predicate_base_cost < start_predicate_cost:
                continue
            if bex_multiplier < start_bex and predicate_base_cost < start_predicate_cost:
                logger.info(f"Skipping bex_multiplier {bex_multiplier} as it is less than start_bex {start_bex} and predicate_base_cost {predicate_base_cost} as it is less than start_predicate_cost {start_predicate_cost}")
                continue
            
            sweep_dir = os.path.join(sweep_dir_base, f"sweep_bex_{bex_multiplier}_predcost_{predicate_base_cost}")
            os.makedirs(sweep_dir, exist_ok=True)
            logger.info(f"Starting inner loop with BEX multiplier {bex_multiplier} and predicate base cost {predicate_base_cost}. Output dir: {sweep_dir}")
            # Reset invariants directory
            logger.info(f"Setting invariants directory for next BEX multiplier {bex_multiplier}.")
            invariants_dir = os.path.join(sweep_dir, constants.INVARIANT_PATH)
            os.makedirs(invariants_dir, exist_ok=True)
            csr_generator = generate_csr_separators.CSRSeparatorGenerator(
                checker_path,
                output_dir=invariants_dir
            )
            invalid_csrs_file = csr_generator.run()
            sweep_seed_invariants = list(seed_invariants)
            sweep_seed_invariants.append(invalid_csrs_file)
            copy_seed_invariants(sweep_seed_invariants,  invariants_dir)
            logger.info(f"Done resetting invariants directory for next BEX multiplier {bex_multiplier}.")
            inner_loop(copy.deepcopy(all_cexs), sweep_dir, bex_multiplier, predicate_base_cost, benign_example_list, checker_path, conditional_signals_mapping_path, regex_config_path,seed_invariants)
            logger.info(f"Completed inner loop with BEX multiplier {bex_multiplier}.")
        
if __name__ == "__main__":
    setup_logging(logging.INFO)
    # def handle_exit(signum, frame):
    #     profiler.disable()
    #     with open("profile_results.txt", "w") as f:
    #         stats = pstats.Stats(profiler, stream=f)
    #         stats.strip_dirs()
    #         stats.sort_stats("cumulative")
    #         stats.print_stats()
    #     sys.exit(1)

    # profiler = cProfile.Profile()
    # profiler.enable()

    # signal.signal(signal.SIGINT, handle_exit)
    # signal.signal(signal.SIGTERM, handle_exit)

    # try:
    main()
    # finally:
    #     profiler.disable()
    #     with open("profile_results.txt", "w") as f:
    #         stats = pstats.Stats(profiler, stream=f)
    #         stats.strip_dirs()
    #         stats.sort_stats("cumulative")
    #         stats.print_stats()

