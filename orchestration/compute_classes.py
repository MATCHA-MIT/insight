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

subdir = Path(__file__).parent.parent / "mutation_engine"
sys.path.insert(0, str(subdir))
subdir = Path(__file__).parent
sys.path.insert(0, str(subdir))

from riscv_instruction_mutator import FileSource
from multiprocessing import Pool
import cex_generator
import re
from logging_common import setup_logging
import cProfile
import pstats
import signal
import time
import argparse

ffi = cffi.FFI()
rust_finder_library = ffi.dlopen(constants.INVARIANT_FINDER_LIBRARY_PATH)
ffi.cdef('bool ffi_check_on_waveform(char *waveform_path, char *invariant_json_path, char *clock_signal);')
ffi.cdef('const char *ffi_find_invariant(const char *output_sets_path, const char *regex_config_path, uint64_t bex_weight, uint64_t predicate_cost);')
ffi.cdef("void ffi_free_library_string(const char *ptr);")
ffi.cdef('char *check_cex_items_all_invariants(const char *cex_items, const char *invariant_dict_json);')

logger = logging.getLogger("main")

CHECKER_FORMAT = "example_cores/compare_to_kronos_cascade/build/no_{}_fix/libcorrectness.so"

def check_cex_false_positive(args):
    checker = args["checker"]
    cex = args["cex"]
    waveform_path = args.get("waveform", None)
    commit_check_res = analyzer.check_commit_log(checker, cex, waveform_path)
    if commit_check_res.Kind != analyzer.CheckerResultKind.CEX:
        # tqdm.tqdm.write(f"{cex} is a false positive, skipping")
        return (cex, "BEX")
    else:
        return (cex, "CEX")
    
def classify_according_to_invariant_dir(invariants_dir, all_cexs):
    ffi_invariant_vec_ptr = analyzer.get_invariant_objects(invariants_dir)
    print("Got invariant objects pointer", ffi_invariant_vec_ptr, flush=True)
    ffi_invariant_vec_ptr_as_int = int(ffi.cast("uintptr_t", ffi_invariant_vec_ptr))
    print("Got invariant objects pointer as int", ffi_invariant_vec_ptr_as_int, flush=True)
    #ffi_invariant_vec_ptr = ffi.cast("void *", ffi_invariant_vec_ptr_as_int)
    #print("Got invariant objects pointer", ffi_invariant_vec_ptr, flush=True)
    ffi_invariant_vec = json.loads(ffi.string(ffi_invariant_vec_ptr).decode("utf-8"))
    cex_items = json.dumps([{"file": cex, "waveform_path": cex + ".vcd"} for cex in all_cexs])
    print(f"Checking {len(all_cexs)} cex items against {len(ffi_invariant_vec)} invariants", flush=True)
    invariant_check_results_json = rust_finder_library.check_cex_items_all_invariants(cex_items.encode("utf-8"), ffi_invariant_vec_ptr)
    invariant_check_results = json.loads(ffi.string(invariant_check_results_json).decode("utf-8"))
    print(f"Got invariant check results for {len(invariant_check_results)} cex items", flush=True)
    rust_finder_library.ffi_free_library_string(ffi_invariant_vec_ptr)
    rust_finder_library.ffi_free_library_string(invariant_check_results_json)
    bug_classes = {}
    for item in invariant_check_results:
        #print(item)
        cex_item = item[0]
        invariants_idx = item[1]
        for idx in invariants_idx:
            #print(ffi_invariant_vec[int(idx)])
            if idx not in bug_classes:
                bug_classes[idx] = []
            bug_classes[idx].append(cex_item)

    return bug_classes #invariant_check_results
    # print(f"Got {len(ffi_invariant_vec)} invariant objects", flush=True)
    # with open("formal-verif/compare_cores/src/compare_to_kronos_cascade/obj_dir_kronos_no_fix/cond_map.json", "r") as f:
    #     cond_mapping = json.load(f)
    # #exit(0)    
    # # Check initial invariant satisfaction:
    # check_invariant_args = [{"file": cex, "waveform_path": cex + ".vcd", "invariant_dict": ffi_invariant_vec, "invariant_dict_ptr": ffi_invariant_vec_ptr_as_int} for cex in all_cexs]
    # invariant_dict = {}
    # with Pool() as pool:
    #     results = list(tqdm.tqdm(pool.imap(check_invariant, check_invariant_args), total=len(check_invariant_args), desc="Initial invariant check"))
    #     #to_remove = [r[0] for r in results if r is not None]
    #     for r in results:
    #         if r is not None:
    #             #print("CEX", r[0], "is covered by invariant", r[1], flush=True)
    #             if r[1]["path"] not in invariant_dict:
    #                 invariant_dict[r[1]["path"]] = 1
    #             else:
    #                 invariant_dict[r[1]["path"]] += 1


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cexs_dir", type=str, required=True, help="Path to the directory containing the counterexamples to check")
    parser.add_argument("--class_file", type=str, required=True, help="Output path to the file containing the classes")
    parser.add_argument("--sweep_dir", type=str, help="Path to the sweep directory", default=None)

    args = parser.parse_args()

    setup_logging(logging.INFO)

    cexs_dir = args.cexs_dir
    class_file = args.class_file
    dedup_classification_path = os.path.join(cexs_dir, "dedup-classification.json")
    if os.path.exists(dedup_classification_path):
        # If it exists, load it and save the ones with "CEX" as value in all_cexs
        with open(dedup_classification_path, "r") as f:
            dedup_classification = json.load(f)
            all_cexs = [cex for cex, value in dedup_classification.items() if value == "CEX"]
    else:
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

        check_cex_false_positive_args = [{"checker": "example_cores/compare_to_kronos_cascade/build/baseline/libcorrectness.so","cex": cex, "waveform": cex + ".vcd"} for cex in all_cexs]
        #it"s important to only pass "waveform" here: Otherwise, we re-generate waveforms for all other bugs, which have different signals, and then we cannot hceck the invariants anymore.
        with Pool() as pool:
            results = list(tqdm.tqdm(pool.imap(check_cex_false_positive, check_cex_false_positive_args), total=len(all_cexs), desc="False positive check"))

        false_positives = set()
        for cex, result in results:
            if result == "BEX":
                false_positives.add(cex)

        all_cexs = [cex for cex in all_cexs if cex not in false_positives]
        
        check_cex_unfixed_args = [{"checker": "example_cores/compare_to_kronos_cascade/build/all_fix/libcorrectness.so","cex": cex} for cex in all_cexs]
        with Pool() as pool:
            results = list(tqdm.tqdm(pool.imap(check_cex_false_positive, check_cex_unfixed_args), total=len(all_cexs), desc="Unfixed check"))
        unfixed_cexs = set()
        for cex, result in results:
            if result == "CEX":
                unfixed_cexs.add(cex)
        #all_cexs = list(unfixed_cexs)
        logger.info(f"After removing false positives and fixed cex files, {len(unfixed_cexs)} cex files remain")
        #print("Frist ten remaining cex files:")
        #for cex in all_cexs[:10]:
        #    print(cex)
        #return 
            

    classes = {"k1": set(), "k4": set(), "k5": set()}
    for label in classes.keys():
        logger.info(f"Processing class {label}")
        checker = CHECKER_FORMAT.format(label)
        check_cex_false_positive_args = [{"checker": checker, "cex": cex} for cex in all_cexs]
        with Pool() as pool:
            results = list(tqdm.tqdm(pool.imap(check_cex_false_positive, check_cex_false_positive_args), total=len(all_cexs), desc=f"Class {label} false positive check"))
        for cex, result in results:
            if result == "CEX":
                classes[label].add(cex)
        # logger.info(f"Class {label} has {len(classes[label])} unique cex files")
    
    all_cexs_len = len(all_cexs)
    logger.info(f"BEFORE INTERSECTIONS")
    # Now compute intersections
    k1_set = classes["k1"]
    k4_set = classes["k4"]
    k5_set = classes["k5"]
    logger.info(f"CEXS on design without K1 fix: {len(k1_set)}")
    # logger.info(f"CEXS on design without K2 fix: {len(k2_set)}")
    logger.info(f"CEXS on design without K4 fix: {len(k4_set)}")
    logger.info(f"CEXS on design without K5 fix: {len(k5_set)}")
    
    total_unique_cexs = {v for class_set in classes.values() for v in class_set}
    logger.info(f"Total unique cex files across all classes: {len(total_unique_cexs)} out of {all_cexs_len} total cex files")
    logger.info(f"Total unique cex files from dedup {len(all_cexs)}")
    if all_cexs_len != len(total_unique_cexs):
        raise Exception(f"Not every CEX is actually covered by a class? {all_cexs_len} vs {len(total_unique_cexs)}")
    with open(class_file, "w") as f:
        json.dump({k: list(v) for k, v in classes.items()}, f, indent=4)
    
    if args.sweep_dir is not None:
        sweep_dir = args.sweep_dir
        for this_dir in os.listdir(sweep_dir):
            if not os.path.isdir(os.path.join(sweep_dir, this_dir)):
                print(f"Skipping {this_dir} as it is not a directory")
            this_invariants_dir = os.path.join(sweep_dir,this_dir, "invariants")
            if not os.path.exists(this_invariants_dir):
                print(f"Skipping {this_invariants_dir} as it does not exist")
                continue
            out_dict = classify_according_to_invariant_dir(this_invariants_dir, all_cexs)
            with open(os.path.join(sweep_dir, this_dir,"bug_classes.json"), "w") as fp:
                json.dump(out_dict,fp, indent=4)
    # 
            
            

    # logger.info(f"AFTER INTERSECTIONS")
    # # Class k4 is the intersection of all of those
    # k4_set = k1_set.intersection(k2_set).intersection(k5_set)
    # classes["k4"] = k4_set

    # # Update the classes removing the elements in k4
    # classes["k1"] = k1_set - k4_set
    # classes["k2"] = k2_set - k4_set
    # classes["k5"] = k5_set - k4_set

    # # Class k12 is the intersection of k1 and k2
    # k12_set = k1_set.intersection(k2_set) - k4_set
    # classes["k12"] = k12_set

    # k15_set = k1_set.intersection(k5_set) - k4_set
    # classes["k15"] = k15_set

    # k25_set = k2_set.intersection(k5_set) - k4_set
    # classes["k25"] = k25_set

    # classes["k1"] = classes["k1"] - k12_set - k15_set
    # classes["k2"] = classes["k2"] - k12_set - k25_set
    # classes["k5"] = classes["k5"] - k15_set - k25_set

    # logger.info(f"Class k1 has {len(classes['k1'])} unique cex files")
    # logger.info(f"Class k2 has {len(classes['k2'])} unique cex files")
    # logger.info(f"Class k4 has {len(k4_set)} unique cex files")
    # logger.info(f"Class k5 has {len(classes['k5'])} unique cex files")
    # logger.info(f"Class k12 has {len(classes['k12'])} unique cex files")
    # logger.info(f"Class k15 has {len(classes['k15'])} unique cex files")
    # logger.info(f"Class k25 has {len(classes['k25'])} unique cex files")



if __name__ == "__main__":
    main()
