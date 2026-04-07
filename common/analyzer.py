import bisect
from concurrent.futures import ThreadPoolExecutor, as_completed
import contextlib
import ctypes
import json
import subprocess
import enum
import collections
import sys
import tempfile
import pathlib
import tempfile
import threading
import cffi
import tqdm
import constants as constants_module
import os
import binascii
import logging
import typing

logger = logging.getLogger("analyzer")

subdir = pathlib.Path(__file__).parent / "formal-verif" / "invariant_generation" / "vincent_invariant_generator"
sys.path.append(str(subdir))

subdir = pathlib.Path(__file__).parent / "riscv_instruction_mutator_pkg" 
sys.path.insert(0, str(subdir))

from riscv_instruction_mutator import program
import time
from functools import lru_cache


ffi = cffi.FFI()
rust_finder_library = ffi.dlopen(constants_module.INVARIANT_FINDER_LIBRARY_PATH)
ffi.cdef('bool ffi_check_on_waveform(char *waveform_path, char *invariant_json_path, char *clock_signal);')
ffi.cdef('int64_t check_any_invariant_fulfilled_on_waveform(char *waveform_path, char *invariant_directory, char *clock_signal, bool short_circuit);')
ffi.cdef('void ffi_free_library_string(const char* s);')
ffi.cdef("int64_t check_any_invariant_fulfilled_on_waveform_from_json_string(char *waveform_path, char *invariant_json_string, char *clock_signal, bool short_circuit);")
ffi.cdef("""
    struct ExecutionResult {
        uint8_t execution_finished;
        uint8_t correct;
        uint32_t mismatch_index;
        uint32_t mismatch_instruction_idx;
        uint32_t mismatch_cycle_dut;
        uint32_t mismatch_cycle_ref;
        uint64_t *constants;
    };
    struct ExecutionResult *run_simulation(int argc, char** argv, char** envp);
""")
ffi.cdef("""void free_result_struct(struct ExecutionResult* result);""")
ffi.cdef("""
    const char *get_invariant_objects(const char *invariants_directory);
""")

REF_COMMIT_LOG_MARKER = "REF commit log"
DUT_COMMIT_LOG_MARKER = "DUT commit log"

class CheckerResultKind(enum.Enum):
    CEX = 1
    BENIGN = 2
    INVALID = 3
    FILTERED = 4
    FULFILLS_INVARIANT = 5

DifferenceLocation = collections.namedtuple("DifferenceLocation", ["instruction_number", "instruction_idx", "refcore_cycle", "dut_cycle"])

"""
Kind: The result of the comparision, cex, benign, invalid 
difference_location: Of type DifferenceLocation, failing instruction number and respective cycles
Constant: List of integer constants found in the commit log
"""
CommitLogCheckerResult = collections.namedtuple("CheckerResult", ["Kind","difference_location", "constants"])

AnalyzerResult = collections.namedtuple("AnalyzerResult", ["kind", "minimized_example", "bug_type", "difference_location", "interesting_instructions", "constants"])

ExecutionResult = collections.namedtuple(
    "ExecutionResult",
    [
        "execution_finished",
        "correct",
        "mismatch_index",
        "mismatch_instruction_idx",
        "mismatch_cycle_dut",
        "mismatch_cycle_ref",
        "constants",
    ],
)

# Cache for loaded Verilator libraries (keyed by path)
_verilator_lib_cache: dict[str, typing.Any] = {}
_verilator_run_fn_cache: dict[str, typing.Any] = {}
_verilator_lib_lock = threading.Lock()

def _get_verilator_library(verilator_path: str):
    lib = _verilator_lib_cache.get(verilator_path)
    if lib is not None:
        return lib
    with _verilator_lib_lock:
        lib = _verilator_lib_cache.get(verilator_path)
        if lib is None:
            start = time.time()
            lib = ffi.dlopen(verilator_path)
            end = time.time()
            if os.getenv("VERILATOR_DIAG"):
                print(f"dlopen({verilator_path}) time: {end - start} seconds", flush=True)
            _verilator_lib_cache[verilator_path] = lib
        return lib

def _get_run_simulation_fn(verilator_path: str):
    fn = _verilator_run_fn_cache.get(verilator_path)
    if fn is not None:
        return fn
    lib = _get_verilator_library(verilator_path)
    with _verilator_lib_lock:
        fn = _verilator_run_fn_cache.get(verilator_path)
        if fn is None:
            start = time.time()
            fn = getattr(lib, "run_simulation")
            end = time.time()
            if os.getenv("VERILATOR_DIAG"):
                print(f"getattr(run_simulation) time: {end - start} seconds", flush=True)
            _verilator_run_fn_cache[verilator_path] = fn
        return fn

@lru_cache(maxsize=4096)
def _cstr_cached(s: str):
    # Cache cffi char[] for repeated strings (e.g., +no_stdout, stable file paths)
    return ffi.new("char[]", s.encode("utf-8"))



def call_execution_result_c_function(
    verilator_path: str, verilator_args: typing.List[str],
    extract_constants: bool = False
    ) -> ExecutionResult:
    """
    Calls a C function via cffi interface that returns an ExecutionResult struct
    and extracts the constants as a dictionary.

    Args:
        c_function_name: The name of the C function to call.
        verilator_args: List of arguments to pass to the Verilator simulation.
    Returns:
        A dictionary containing the constants and other fields from the ExecutionResult struct.
    """
    # Prepare arguments for the C function
    # print("Now calling C function", "run_simulation", "in", verilator_path, "with args", verilator_args, flush=True)
    
    verilator_args.append("+no_stdout")  # Disable stdout from Verilator simulation
    verilator_args.append("+quiet")  # Disable stdout from Verilator simulation
    argc = len(verilator_args)
    argv_cstrs = [_cstr_cached(arg) for arg in verilator_args]
    argv = ffi.new("char * []", argv_cstrs)
    envp = ffi.NULL

    # print("Now calling C function", "run_simulation", "in", verilator_path, "with args", verilator_args, flush=True)

    run_fn = _get_run_simulation_fn(verilator_path)
    start_time = time.time()
    result = run_fn(argc, argv, envp)    # Call the C function
    end_time = time.time()
    #print(f"Verilator simulation time: {end_time - start_time} seconds", flush=True)
    if result == ffi.NULL:
        return None #raise RuntimeError("run_simulation function returned NULL {} {}".format(verilator_path, verilator_args))

    # Extract constants
    constants = []
    if extract_constants is True:
        if result.constants != ffi.NULL:
            i = 0
            while result.constants[i] != 0:  # Assuming the array is null-terminated
                constants.append(result.constants[i])
                i += 1
    
    # Convert the result to a dictionary
    # print("Result", int(result.correct))

    res = ExecutionResult(
        execution_finished=result.execution_finished,
        correct=result.correct,
        mismatch_index=result.mismatch_index,
        mismatch_instruction_idx=result.mismatch_instruction_idx,
        mismatch_cycle_dut=result.mismatch_cycle_dut,
        mismatch_cycle_ref=result.mismatch_cycle_ref,
        constants=constants,
    )
    getattr(_get_verilator_library(verilator_path), "free_result_struct")(result)
    return res

def ffi_check_invariant_satisfaction(waveform_path, invariant_file_path):
    waveform_path_bytes = waveform_path.encode() if isinstance(waveform_path, str) else waveform_path
    invariant_file_path_bytes = invariant_file_path.encode() if isinstance(invariant_file_path, str) else invariant_file_path
    clk_signal_bytes = constants_module.CLK_SIGNAL.encode()
    return rust_finder_library.ffi_check_on_waveform(waveform_path_bytes, invariant_file_path_bytes, clk_signal_bytes)

def get_invariant_objects(invariants_path: str):
    invariant_dir_bytes = invariants_path.encode() if isinstance(invariants_path, str) else invariants_path
    result_ptr = rust_finder_library.get_invariant_objects(invariant_dir_bytes)
    return result_ptr

def waveform_fullfills_any_invariant(waveform_path, invariants_path):
    # print("all invariants", os.listdir(constants_module.INVARIANT_PATH))
    for i, invariant_file in enumerate(os.listdir(invariants_path)):
        # print("Checking invariant", invariant_file)
        invariant_file_path = os.path.join(invariants_path, invariant_file)
        if not invariant_file_path.endswith(".json"): #Ignore .swp files
            continue
        if ffi_check_invariant_satisfaction(waveform_path, invariant_file_path):
            return invariant_file_path
    # print("Waveform does not fulfill any invariant")
    return None

def waveform_count_fulfilled_invariants_from_list(waveform_path, invariants_path):
    count = 0
    for i, invariant_file in enumerate(os.listdir(invariants_path)):
        invariant_file_path = os.path.join(invariants_path, invariant_file)
        if not invariant_file_path.endswith(".json"): #Ignore .swp files
            continue
        if check_invariant_satisfaction(waveform_path, invariant_file_path):
            count += 1
    return count

def waveform_compute_fulfilled_invariant_set(waveform_path, invariants_path):
    inv_set = []
    for i, invariant_file in enumerate(os.listdir(invariants_path)):
        invariant_file_path = os.path.join(invariants_path, invariant_file)
        if not invariant_file_path.endswith(".json"): #Ignore .swp files
            continue
        if check_invariant_satisfaction(waveform_path, invariant_file_path):
            inv_set.append(invariant_file_path)
    return inv_set

def waveform_fulfills_any_invariant_bool(waveform_path, invariants_path):
    logger.debug(f"Checking waveform for any invariant: {waveform_path}")
    res = rust_finder_library.check_any_invariant_fulfilled_on_waveform(waveform_path.encode(), invariants_path.encode(), constants_module.CLK_SIGNAL.encode(), True)
    if res == -1:
        return False
    else:
        return True
    
def waveform_fulfills_any_invariant_from_list_return_idx(waveform_path, invariant_object_str):
    logger.debug(f"Checking waveform for any invariant from list: {waveform_path}")
    res = rust_finder_library.check_any_invariant_fulfilled_on_waveform_from_json_string(waveform_path.encode(), invariant_object_str, constants_module.CLK_SIGNAL.encode(), True)
    return res

def waveform_fulfills_any_invariant_from_list(waveform_path, invariant_object_str):
    res = waveform_fulfills_any_invariant_from_list_return_idx(waveform_path, invariant_object_str)
    if res == -1:
        return False
    else:
        return True

class BugTypeResult(enum.Enum):
    EXPOSE_ONLY = 1
    SETUP_ONLY = 2
    EXPOSE_AND_SETUP = 3
    
def _apply_nops_to_bytes(orig_bytes: bytes, indices: typing.Iterable[int], nop_word: int = 0x00000013) -> bytes:
    """
    Return a mutated copy of orig_bytes with a NOP (addi x0, x0, 0) placed at the
    given instruction indices (each instruction is 4 bytes, little-endian).
    """
    if not indices:
        return orig_bytes
    ba = bytearray(orig_bytes)
    nop_bytes = nop_word.to_bytes(4, "little", signed=False)
    for idx in indices:
        off = idx * 4
        if 0 <= off <= len(ba) - 4:
            ba[off:off + 4] = nop_bytes
    return bytes(ba)


def _still_cex(checker_path: str, bin_bytes: bytes, reuse_bin_path: str, check_delete=False, invariant_ffi_ptr=None, check_mismatch_index=None) -> bool:
    """
    Write bin_bytes to reuse_bin_path and run check_commit_log. Return True if result is CEX.
    :param checker_path: Path to the testbench/checker binary to be invoked by check_commit_log.
    :param bin_bytes: The binary bytes to write to the file.
    :param reuse_bin_path: Path to the temporary binary file to write to.
    :param check_delete: Whether this is a delete operation (affects logging).
    """
    with open(reuse_bin_path, "wb") as fp:
        fp.write(bin_bytes)
    check_invariant_satisfaction = invariant_ffi_ptr is not None and invariant_ffi_ptr != ffi.NULL
    if check_invariant_satisfaction:
        with tempfile.NamedTemporaryFile(suffix=".fst", delete=True) as waveform_tmp:
            waveform_path = waveform_tmp.name
            result = check_commit_log(
                checker_path=checker_path,
                bin_file_input_path=reuse_bin_path,
                waveform_output=waveform_path,
                check_invariant_satisfaction=check_invariant_satisfaction,
                extract_constants=False,
                invariant_ffi_ptr=invariant_ffi_ptr
            )
    else:
        result = check_commit_log(
            checker_path=checker_path,
            bin_file_input_path=reuse_bin_path,
            waveform_output=None,
            check_invariant_satisfaction=check_invariant_satisfaction,
            extract_constants=False,
            invariant_ffi_ptr=invariant_ffi_ptr
        )
    #end_time = time.time()
    # print(f"check_commit_log execution time: {end_time - start_time} seconds", flush=True)
    if result is None:
        raise RuntimeError("check_commit_log returned None")
    if result.Kind == CheckerResultKind.CEX:
        if check_mismatch_index is not None:
            if result.difference_location.instruction_number != check_mismatch_index:
                # Mismatch occurs after last executed instruction, not a valid CEX
                # print(f"Rejected CEX: mismatch at instruction {result.difference_location.instruction_number}, expected {check_mismatch_index}", flush=True)
                # with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as tf:
                #     tf.write(bin_bytes)
                #     print(f"Rejected CEX binary written to {tf.name}", flush=True)
                return False
            else:
                # print(f"Accepted CEX: mismatch at expected instruction {result.difference_location.instruction_number}", flush=True)
                return True
        else:
            return True
    else:
        return False


def batch_nop_substitution(
    checker_path: str,
    orig_bytes: bytes,
    candidate_indices: typing.List[int],
    interesting_instructions: typing.Optional[typing.List[int]] = None,
    temp_dir: typing.Optional[str] = None,
    initial_chunk_size: typing.Optional[int] = None,
    invariant_ffi_ptr=None,
    keep_benign=False
) -> typing.Tuple[typing.List[int], bytes, typing.List[bytes]]:
    """
    Try to NOP out as many candidate instruction indices as possible while keeping the input a CEX.
    Minimizes calls to check_commit_log via greedy chunking (large chunks first, then shrink).

    Args:
        checker_path: Path to the testbench/checker binary to be invoked by check_commit_log.
        original_bin_path: Path to the original program binary.
        candidate_indices: Instruction indices (0-based) that are safe to attempt NOP.
        temp_dir: Optional directory for the temporary mutated binary file.
        initial_chunk_size: Optional starting chunk size. Defaults to len(candidate_indices).

    Returns:
        accepted_indices: The indices that were successfully NOPed while preserving CEX.
        mutated_bytes: The final mutated program bytes reflecting the accepted NOPs.
    """
    print("Starting batch nop substitution", flush=True)
    if interesting_instructions is None:
        interesting_instructions = []
    # Filter out-of-range indices
    instr_count = len(orig_bytes) // 4
    print("Batch nop substitution, sorting candidate indices", len(candidate_indices), "instr count", instr_count, flush=True)
    cand = sorted(idx for idx in set(candidate_indices) if 0 <= idx < instr_count)
    print("Sorting done", flush=True)
    if not cand:
        return [], orig_bytes

    # Prepare temp file reused across trials
    # os.makedirs(temp_dir or ".", exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".bin", delete=False) as tf:
        reuse_bin_path = tf.name
        tf.write(orig_bytes)
    tested_cache = {}  # frozenset(indices) -> bool (still CEX?)
    tested_cache_lock = threading.Lock()
    accepted: typing.Set[int] = set()
    remaining: typing.List[int] = cand.copy()
    commit_log_result = check_commit_log(
        checker_path,
        reuse_bin_path,
        None,
        extract_constants=False,
        check_invariant_satisfaction=(invariant_ffi_ptr is not None and invariant_ffi_ptr != ffi.NULL),
        invariant_ffi_ptr=invariant_ffi_ptr,
    )
    check_mismatch_index = commit_log_result.difference_location.instruction_number if commit_log_result is not None else None
    print("Check mismatch index", check_mismatch_index)
    all_benign_examples = []

    def try_set_serial(nop_set: typing.Set[int]) -> bool:
        key = frozenset(nop_set)
        hit = tested_cache.get(key)
        if hit is not None:
            return hit
        mutated = _apply_nops_to_bytes(orig_bytes, nop_set)
        ok = _still_cex(checker_path, mutated, reuse_bin_path, invariant_ffi_ptr=invariant_ffi_ptr, check_mismatch_index=check_mismatch_index)
        tested_cache[key] = ok
        return ok

    def try_set_parallel(key_fset: frozenset[int]) -> typing.Tuple[bool, typing.ByteString]:
        # Thread-safe cache check/set with a per-call temp file
        #print("Trying nop set", key_fset, flush=True)
        # with tested_cache_lock:
        #     hit = tested_cache.get(key_fset)
        # if hit is not None:
        #     return hit
        #print("Cache miss, applying nops and checking", key_fset, flush=True)
        mutated = _apply_nops_to_bytes(orig_bytes, key_fset)
        with tempfile.NamedTemporaryFile(suffix=".bin", delete=True) as tf_local:
            ok = _still_cex(checker_path, mutated, tf_local.name, invariant_ffi_ptr=invariant_ffi_ptr, check_mismatch_index=check_mismatch_index)
        # with tested_cache_lock:
        #     tested_cache[key_fset] = ok
        return (ok, mutated)

    #First: Just try to nop out everything else than interesting_instructions
    print("Trying to nop out everything except interesting instructions", flush=True)
    if len(interesting_instructions) >= 1:
        initial_nop_set = frozenset(set(remaining) - set(interesting_instructions))
        if try_set_serial(set(initial_nop_set)):
            accepted |= initial_nop_set
            remaining = [i for i in remaining if i not in accepted]
            print("Successfully nopped out everything except interesting instructions", flush=True)
    # Start with all-at-once; if that fails, shrink chunks
    chunk_size = initial_chunk_size or len(remaining)
    #chunk_size = 1
    # Greedy: accept the largest chunks that keep the CEX; shrink if none pass at current size
    print("Batch nop substitution, starting main loop", flush=True)
    while remaining and chunk_size >= 1:
        found_any = False
                # Build jobs from a snapshot of accepted to keep trials consistent this round
        snapshot_accepted = set(accepted)
        jobs: list[tuple[frozenset[int], typing.Set[int]]] = []
        print("Trying chunk size", chunk_size, "remaining len", len(remaining), "accepted_nop len", len(accepted), flush=True)
        for start in range(0, len(remaining), chunk_size):
            chunk = remaining[start:start + chunk_size]
            new_ids = set(chunk) - snapshot_accepted
            if not new_ids:
                continue
            trial_set = frozenset(snapshot_accepted | new_ids)
            jobs.append((trial_set, new_ids))

        if not jobs:
            # Nothing new to try at this chunk size
            remaining = [i for i in remaining if i not in accepted]
            if chunk_size == 1:
                break
            chunk_size = max(1, chunk_size // 2)
            continue

        # Parallelize trials unless using a single .so (likely not thread-safe)
        #max_workers = 1 if checker_path.endswith(".so") else min(8, (os.cpu_count() or 4))
        max_workers = min(8, (os.cpu_count() or 4))
        if max_workers == 1:
            # Fallback to serial with shared reuse_bin_path
            for trial_set, new_ids in jobs:
                if try_set_serial(set(trial_set)):
                    accepted |= new_ids
                    found_any = True
        else:
            # Run all trials in parallel against snapshot_accepted
            with ThreadPoolExecutor(max_workers=max_workers) as ex:
                fut_to_new_ids = {ex.submit(try_set_parallel, trial_set): new_ids for trial_set, new_ids in jobs}
                passing_chunks: list[typing.Set[int]] = []
                #pbar = tqdm.tqdm(total=len(fut_to_new_ids), desc="Trying nop chunk parallel")
                for fut in as_completed(fut_to_new_ids):
                    ok, bytes = fut.result()
                    new_ids = fut_to_new_ids[fut]
                    if ok:
                        passing_chunks.append(new_ids)
                    elif keep_benign:
                        all_benign_examples.append(bytes)
                    #pbar.update(1)
                    #pbar.set_description(f"Trying nop chunk parallel, accepted: {len(accepted)}, remaining: {len(remaining)}, passing {len(passing_chunks)}")
                    
                        

            # Incrementally validate each passing chunk against the updated accepted set
            for new_ids in tqdm.tqdm(passing_chunks, desc="Validating passing chunks"):
                trial_now = accepted | new_ids
                if try_set_serial(trial_now):
                    accepted |= new_ids
                    found_any = True
                    #break  # Accept one passing chunk at a time to keep trials consistent
        # Remove accepted from remaining
        remaining = [i for i in remaining if i not in accepted]
        if not found_any:
            if chunk_size == 1:
                break
            chunk_size = max(1, chunk_size // 2)
        # this_iter_bytes = _apply_nops_to_bytes(orig_bytes, accepted)
        # with open(reuse_bin_path, "wb") as fp:
        #     fp.write(this_iter_bytes)
        # print("Intermediate res ", check_commit_log(checker_path,
        #                     reuse_bin_path,
        #                     None,
        #                     True,
        #                     invariant_ffi_ptr=invariant_ffi_ptr))# Just to log current status
    print("Batch nop substitution done, accepted len", len(accepted), "remaining", len(remaining), flush=True)
    #print("Remaining indices not nopped:", remaining, flush=True)
    final_bytes = _apply_nops_to_bytes(orig_bytes, accepted)
    if len(accepted) == 0:
        print("No nops accepted, returning original bytes", flush=True)
        assert final_bytes == orig_bytes
    # Persist final_bytes into the reused path to help downstream flows if needed
    with open(reuse_bin_path, "wb") as fp:
        fp.write(final_bytes)
    print(f"Final mutated binary written to {reuse_bin_path}", flush=True)
    # Caller can decide what to do with final_bytes; we leave the temp file in place
    return sorted(accepted), final_bytes, all_benign_examples

def _apply_deletes_to_bytes(orig_bytes: bytes, delete_indices: typing.Iterable[int]) -> bytes:
    """
    Return a mutated copy of orig_bytes with instructions at delete_indices removed.
    Indices are 0-based, each instruction is 4 bytes, little-endian.
    """
    delete_sorted = sorted(set(i for i in delete_indices if i >= 0))
    if not delete_sorted:
        return orig_bytes
    instr_count = len(orig_bytes) // 4
    ba = bytearray()
    last = 0
    for idx in delete_sorted:
        if idx >= instr_count:
            break
        # copy block [last, idx)
        if idx > last:
            ba.extend(orig_bytes[last * 4: idx * 4])
        last = idx + 1
    # tail
    if last < instr_count:
        ba.extend(orig_bytes[last * 4: instr_count * 4])
    return bytes(ba)


def batch_delete_substitution_bytes(
    checker_path: str,
    original_bytes: bytes,
    candidate_indices: typing.List[int],
    temp_dir: typing.Optional[str] = None,
    initial_chunk_size: typing.Optional[int] = None,
    one_pass: bool = False,
    invariant_ffi_ptr=None,
) -> typing.Tuple[typing.List[int], bytes]:
    """
    Try to delete as many candidate instruction indices as possible while keeping the input a CEX.
    Uses greedy chunking (large chunks first, then shrink) to reduce calls to check_commit_log.

    Returns:
        accepted_deletes: Indices (relative to original_bytes) that were deleted.
        mutated_bytes: Final bytes with accepted deletions applied.
    """
    instr_count = len(original_bytes) // 4
    cand = sorted(idx for idx in set(candidate_indices) if 0 <= idx < instr_count)
    if not cand:
        return [], original_bytes

    with tempfile.NamedTemporaryFile(dir=temp_dir, suffix=".bin", delete=False) as tf:
        reuse_bin_path = tf.name

    tested_cache: dict[frozenset[int], bool] = {}
    tested_cache_lock = threading.Lock()
    accepted: typing.Set[int] = set()
    remaining: typing.List[int] = cand.copy()
    chunk_size = initial_chunk_size or len(remaining)

    def try_delete_set_serial(delete_set: typing.Set[int]) -> bool:
        key = frozenset(delete_set)
        with tested_cache_lock:
            hit = tested_cache.get(key)
        if hit is not None:
            return hit
        mutated = _apply_deletes_to_bytes(original_bytes, delete_set)
        ok = _still_cex(checker_path, mutated, reuse_bin_path, check_delete=True, invariant_ffi_ptr=invariant_ffi_ptr)
        with tested_cache_lock:
            tested_cache[key] = ok
        return ok

    def try_delete_set_parallel(key_fset: frozenset[int]) -> bool:
        
        # Per-task temp path to avoid reuse races
        key = frozenset(key_fset)
        with tested_cache_lock:
            hit = tested_cache.get(key)
        if hit is not None:
            return hit
        mutated = _apply_deletes_to_bytes(original_bytes, key_fset)
        with tempfile.NamedTemporaryFile(suffix=".bin", delete=True) as tf_local:
            ok = _still_cex(checker_path, mutated, tf_local.name, check_delete=True, invariant_ffi_ptr=invariant_ffi_ptr)
        with tested_cache_lock:
            tested_cache[key] = ok
        return ok


    while remaining and chunk_size >= 1:
        found_any = False
        # Build jobs from a snapshot to keep trials consistent within this round
        snapshot_accepted = set(accepted)
        jobs: list[tuple[frozenset[int], typing.Set[int]]] = []
        print("Trying to delete chunk size", chunk_size, "accepted", len(accepted), "remaining", len(remaining), flush=True)
        for start in range(0, len(remaining), chunk_size):
            chunk = remaining[start:start + chunk_size]
            new_ids = set(chunk) - snapshot_accepted

            if not new_ids:
                continue
            trial_set = frozenset(snapshot_accepted | new_ids)
            if start <= 50:
                print("Considering chunk", chunk, "new ids", new_ids, "trial set", trial_set, flush=True)
            jobs.append((trial_set, new_ids))

        if not jobs:
            remaining = [i for i in remaining if i not in accepted]
            if chunk_size == 1:
                break
            chunk_size = max(1, chunk_size // 2)
            continue

        # Parallelize deletion trials similarly to NOP trials
        max_workers = min(8, (os.cpu_count() or 4))
        passing_chunks: list[typing.Set[int]] = []
        with ThreadPoolExecutor(max_workers=max_workers) as ex:
            fut_to_new_ids = {ex.submit(try_delete_set_parallel, trial_set): new_ids for trial_set, new_ids in jobs}
            for fut in tqdm.tqdm(as_completed(fut_to_new_ids), total=len(fut_to_new_ids), desc=f"Trying delete chunks accepted: {len(accepted)}, remaining: {len(remaining)}, passing {len(passing_chunks)}"):
                ok = fut.result()
                new_ids = fut_to_new_ids[fut]
                if ok:
                    passing_chunks.append(new_ids)

        # Incrementally validate each passing chunk against the updated accepted set
        for new_ids in tqdm.tqdm(passing_chunks):
            trial_now = accepted | new_ids
            if try_delete_set_serial(trial_now):
                accepted |= new_ids
                found_any = True
                #break  # Accept one passing chunk at a time to keep trials consistent
        # Housekeeping
        remaining = [i for i in remaining if i not in accepted]
        if not found_any:
            if chunk_size == 1:
                break
            chunk_size = max(1, chunk_size // 2)
        if one_pass:
            break
    print("Batch delete substitution done, accepted len", len(accepted), "remaining", len(remaining), flush=True)
    final_bytes = _apply_deletes_to_bytes(original_bytes, accepted)
    with open(reuse_bin_path, "wb") as fp:
        fp.write(final_bytes)
    return sorted(accepted), final_bytes

def trim_unexecuted_tail_bytes(orig_bytes: bytes, last_executed_idx: int) -> typing.Tuple[bytes, typing.List[int]]:
    """
    Remove all instructions strictly after last_executed_idx.
    This is safe for counterexamples: mismatch occurs at/before last_executed_idx, tail is not executed.
    Returns (trimmed_bytes, deleted_indices).
    """
    instr_count = len(orig_bytes) // 4
    keep = max(0, min(instr_count, last_executed_idx))
    print("Keeping until instruction index", keep, "of", instr_count, flush=True)
    if keep >= instr_count:
        return orig_bytes, []
    deleted = list(range(keep, instr_count))
    return orig_bytes[:keep * 4], deleted

            
def find_executed_instructions(checker_path: str, input_file: str, first_symbolic_instruction_idx: int, start_address: int) -> list[int]:
    """Parse the output of the testbench to identify instructions that were actually executed"""
    program_length = os.path.getsize(input_file) // 4
    check_result = check_commit_log(checker_path, input_file, None, False)
    if check_result.Kind != CheckerResultKind.CEX:
        raise Exception("There is something wrong here, this should be a cex")
    expose_instruction_number = check_result.difference_location.instruction_number
    print("Expose instruction number", expose_instruction_number, flush=True)
    args = [checker_path, input_file]
    if checker_path.endswith(".so"):
        args[0] = os.path.join(os.path.dirname(checker_path), "Vcorrectness")
    #Skipping the below part, we do not need a waveform here
    # with tempfile.NamedTemporaryFile(delete=False, suffix=".fst") as waveform_tmp:
    #     args.append(waveform_tmp.name)
    #     waveform_file_path = waveform_tmp.name
    try:
        output = subprocess.check_output(args, stderr=subprocess.PIPE)
    except subprocess.CalledProcessError as e:
        output = e.output  # This captures the output even if the subprocess fails
        logger.warning(f"Testbench failed with return code {e.returncode} on path {input_file}, but we are ignorining that")
        raise e
    output = output.decode("utf-8").split("\n")
    commit_log_ref = False
    commit_log_dut = False
    stall_detected = False
    executed_instructions = set()
    #print("output", len(output), "lines")
    #print(output)
    for line in output:
        if "STALL detected" in line:
            stall_detected = True
        #continue
        # print("Parsing", line, "start_address", start_address, "first idx", first_symbolic_instruction_idx, "expose_numer", expose_instruction_number, flush=True)
        if REF_COMMIT_LOG_MARKER in line:
            commit_log_ref = True
            commit_log_dut = False#
            continue
        if DUT_COMMIT_LOG_MARKER in line:
            # print("Wef ound the DUT commit log", flush=True)
            commit_log_ref = False
            commit_log_dut = True
            continue
        if not commit_log_ref and not commit_log_dut:
            continue
        if expose_instruction_number is not None: #If it's zero if expose_instruction_number does not work..
            line_elems = [x for x in line.split(" ") if x != ""]
            # print("this line elem", line_elems, "this start address", start_address, flush=True)
            try:
                this_idx = int(line_elems[1])
            except ValueError as e:
                # print("error", line_elems, flush=True)
                raise Exception(f"erro {line_elems} {this_idx}")
                raise e
            if this_idx <= expose_instruction_number:
                #TODO Vincent: This needs to change if we have an offset that is not 0x8000000!!!
                this_commit_pc = int(line_elems[4], 16)
                # print(f"Found commit pc {hex(this_commit_pc)}")
                if this_commit_pc < start_address: #What if go above this address (e.g. because of mtvec). We ignore this, as it is not "our instruction"
                    pass #continue
                else:
                    this_value = (this_commit_pc - start_address) // 4 - first_symbolic_instruction_idx
                    # print(f"Adding executed instruction idx {hex(this_value)}")
                    executed_instructions.add((this_commit_pc - start_address) // 4 - first_symbolic_instruction_idx)
            else:
                pass
		#Do not break -- after all, we might have not passed both commit logs yet
                #break
            #print("This idx", this_idx, "expose", expose_instruction_number, REF_COMMIT_LOG_MARKER,DUT_COMMIT_LOG_MARKER, flush=True)
            if this_idx == expose_instruction_number:
                commit_log_dut = False
                commit_log_ref = False
    if len(executed_instructions) > 0 and max(executed_instructions) <= program_length:
        executed_instructions.add((max(executed_instructions)+1)) #Might always have just been a jump back to vector table
    if (len(executed_instructions) == 0) and (stall_detected or (expose_instruction_number is not None and expose_instruction_number == 0)):
        executed_instructions = {0}
    # print("Returning executed instructions", executed_instructions)
    executed_instructions = sorted([x for x in executed_instructions if x >= 0])
    return executed_instructions



def get_constant_from_verilator_output(outputs, instruction_number: typing.Optional[int]) -> list[int]:
    # Find commit log indices in a single pass
    if instruction_number is None:
        this_instruction_number = 0
    else:
        this_instruction_number = instruction_number
    sodor_commit_log_idx = next((i for i, line in enumerate(outputs) if REF_COMMIT_LOG_MARKER in line), -1)
    ibex_commit_log_idx = next((i for i, line in enumerate(outputs) if DUT_COMMIT_LOG_MARKER in line), -1)
    if sodor_commit_log_idx == -1 or ibex_commit_log_idx == -1:
        return []

    # Adjust instruction_number if needed
    if this_instruction_number == 0:
        this_instruction_number = sodor_commit_log_idx - ibex_commit_log_idx - 2

    # Pre-slice the relevant lines for both logs
    sodor_lines = outputs[sodor_commit_log_idx + 1 : sodor_commit_log_idx + this_instruction_number + 2]
    ibex_lines = outputs[ibex_commit_log_idx + 1 : ibex_commit_log_idx + this_instruction_number + 2]

    found_constants = set()
    for sodor_line, ibex_line in zip(sodor_lines, ibex_lines):
        sodor_elems = sodor_line.split()
        ibex_elems = ibex_line.split()
        if len(sodor_elems) > 3 and len(ibex_elems) > 3:
            found_constants.add(int(ibex_elems[3], 16))
            found_constants.add(int(sodor_elems[3], 16))
    return list(found_constants)

def parse_verilator_output_into_results_dict(output: typing.List[str], extract_constants: bool = False) -> ExecutionResult:
    correct = False
    benign = None
    instruction_number = None
    clock_cycle_refcore = None
    clock_cycle_dut = None
    instruction_idx = None
    # Scan output in reverse only once
    for line in reversed(output):
        if "Correct" in line:
            correct = int(line.split(" ")[1])
            benign = correct == 1
            if benign is True:
                break
            #break
        elif instruction_number is None and "Mismatch at index" in line:
            line = line.strip()
            instruction_number = int(line.split(" ")[-1])
        elif clock_cycle_refcore is None and "Mismatch cycle ref_core" in line:
            line = line.strip()
            clock_cycle_refcore = int(line.split(" ")[-1])
        elif clock_cycle_dut is None and "Mismatch cycle dut_core" in line:
            line = line.strip()
            #print("Found matching line", line, line.split(" "))
            clock_cycle_dut = int(line.split(" ")[-1])
        # Early exit if all found
        if benign is not None and instruction_number is not None and clock_cycle_refcore is not None and clock_cycle_dut is not None:
            break
    if benign is None:
        raise Exception("Could not determine if the testbench output is benign or not. {bin_file_input_path}".format(bin_file_input_path=bin_file_input_path))

    if benign is False:
        if instruction_number is not None and clock_cycle_refcore is not None and clock_cycle_dut is not None:
            pass # We actually do not need that, the rust code will get that from the waveform.
        commit_log_dut = False
        instruction_idx = 0
        for line in output:
            if REF_COMMIT_LOG_MARKER in line:
                commit_log_dut = True
                continue
            if instruction_number and commit_log_dut:
                line_elems = [x for x in line.split(" ") if x != ""]
                if int(line_elems[1]) == instruction_number:
                    #TODO Vincent: This needs to change if we have an offset that is not 0x8000000!!!
                    instruction_idx = (int("0x" + line_elems[4], 16) - 0x80000000) // 4
                    break
    if extract_constants is True:
        found_constants = get_constant_from_verilator_output(output, instruction_number)
    else:
        found_constants = None
    return ExecutionResult(
        execution_finished=1,
        correct=correct,
        mismatch_index=instruction_number if instruction_number is not None else 0,
        mismatch_instruction_idx=instruction_idx if instruction_idx is not None else 0,
        mismatch_cycle_dut=clock_cycle_dut if clock_cycle_dut is not None else 0,
        mismatch_cycle_ref=clock_cycle_refcore if clock_cycle_refcore is not None else 0,
        constants=found_constants,  
    )

def call_verilator_and_get_result_dict(checker_path, bin_file_input_path, waveform_output=None, extract_constants=False) -> ExecutionResult:
    args = [checker_path, bin_file_input_path]
    if waveform_output is not None:
        args.append("+waveform=" + waveform_output)
    if extract_constants is True:
        args.append("+extract_constants")
        # with tempfile.NamedTemporaryFile(delete=False, suffix=".fst") as waveform_tmp:
        #     args.append(waveform_tmp.name)
        #     waveform_file_path = waveform_tmp.name
    if checker_path.endswith(".so"):
        # print("Calling C function in shared library", checker_path, args, flush=True)
        result = call_execution_result_c_function(
            verilator_path=checker_path,
            verilator_args=args,
            extract_constants=extract_constants,
        )
        return result
    else:
        try:
            output = subprocess.check_output(args, stderr=subprocess.PIPE)
        except subprocess.CalledProcessError as e:
            output = e.output  # This captures the output even if the subprocess fails
            logger.warning(f"Testbench failed with return code {e.returncode} on path {bin_file_input_path}, but we are ignorining that")
            raise e
        output = output.decode("utf-8").split("\n")
        return parse_verilator_output_into_results_dict(output, extract_constants=extract_constants)

def check_commit_log(checker_path, bin_file_input_path, waveform_output=None, check_invariant_satisfaction=False,
                    extract_constants=False, invariant_ffi_ptr=None) -> typing.Optional[CommitLogCheckerResult]:
    # file_content = b""
    # instructions = []
    # with open(bin_file_input_path, "rb") as f:
    #     # Read 4 bytes at a time and interpret them as 32-bit integers
    #     while True:
    #         instruction = f.read(4)
    #         file_content += instruction
    #         if not instruction:
    #             break
    #         instructions.append("0x" + hex(int.from_bytes(instruction, byteorder="little"))[2:].zfill(8))
    #print("Checking code ", instructions[0])
    #args = [checker_path, bin_file_input_path]
    start_time = time.time()
    result_dict = call_verilator_and_get_result_dict(checker_path, bin_file_input_path, waveform_output, extract_constants=extract_constants)
    end_time = time.time()
    # print(f"Time taken to run verilator and get result dict: {end_time - start_time} seconds for {bin_file_input_path}")
    # if end_time - start_time > 1.5:
    #     tf_name = None
    #     with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as tf:
    #         tf.write(open(bin_file_input_path, "rb").read())
    #         print(f"Long execution binary written to {tf.name}", flush=True)
    #         tf_name = tf.name
    #     print(f"Long execution binary written to {tf_name}", flush=True)
    #     exit(0)
    if result_dict is None:
        return None
    benign = result_dict.correct == 1
    if benign is True:
        return CommitLogCheckerResult(CheckerResultKind.BENIGN,None,result_dict.constants)
    else:
        kind = CheckerResultKind.CEX
        if check_invariant_satisfaction is True:
            if waveform_output is None:
                print("Generating temporary waveform to check invariant satisfaction", flush=True)
                with tempfile.NamedTemporaryFile(delete=True, suffix=".fst") as waveform_tmp:
                    waveform_path = waveform_tmp.name
                    call_verilator_and_get_result_dict(checker_path, bin_file_input_path, waveform_path, extract_constants=False)
                    #start_time = time.time()
                    if invariant_ffi_ptr is not None and invariant_ffi_ptr != ffi.NULL:
                        invariant_fulfills = waveform_fulfills_any_invariant_from_list(waveform_path, invariant_ffi_ptr)
                    else:
                        invariant_fulfills = waveform_fulfills_any_invariant_bool(waveform_path)
                    #end_time = time.time()
            else:
                waveform_path = waveform_output
                #tart_time = time.time()
                if invariant_ffi_ptr is not None and invariant_ffi_ptr != ffi.NULL:
                    invariant_fulfills = waveform_fulfills_any_invariant_from_list(waveform_path, invariant_ffi_ptr)
                else:
                    invariant_fulfills = waveform_fulfills_any_invariant_bool(waveform_path)
                #end_time = time.time()
            #print(f"Vincent analyzer Time taken to check invariant satisfaction: {end_time - start_time} seconds")
            if invariant_fulfills is True:
                kind = CheckerResultKind.FULFILLS_INVARIANT
        # print("Result dict", result_dict)
        instruction_number = result_dict.mismatch_index
        instruction_idx = result_dict.mismatch_instruction_idx
        clock_cycle_refcore = result_dict.mismatch_cycle_ref
        clock_cycle_dut = result_dict.mismatch_cycle_dut
        found_constants = result_dict.constants
        return CommitLogCheckerResult(kind,DifferenceLocation(instruction_number, instruction_idx, clock_cycle_refcore, clock_cycle_dut),found_constants)
        #return CheckerResult(CheckerResultKind.CEX,DifferenceLocation(instruction_number, clock_cycle_sodor, clock_cycle_ibex),found_constants)

class BugAnalyzer:
    def __init__(self, checker_path, input_file_path, invariants_path, first_symbolic_instruction_idx=0, start_address=0x80000080):
        self.invariants_string_dict = None
        self.checker_path = checker_path
        with open(input_file_path, "rb") as f:
            self.input_file = f.read()
        self.input_file_path = input_file_path
        self.first_symbolic_instruction_idx = first_symbolic_instruction_idx
        self.start_address = start_address
        print("Start address", hex(self.start_address), flush=True)
        if invariants_path is not None:
            self.invariants_path = invariants_path
            print("Getting invariant objects from", self.invariants_path, flush=True)
            self.invariants_string_dict = get_invariant_objects(self.invariants_path)
            print("Got invariant objects", self.invariants_string_dict, flush=True)
        else:
            self.invariants_path = None
            self.invariants_string_dict = None
            print("No invariants path provided, skipping invariant checks", flush=True)
        self.freeze_instructions = []
        
    def set_freeze_instructions(self, freeze_instructions: typing.List[int]):
        self.freeze_instructions = freeze_instructions

    def __del__(self):
        if self.invariants_string_dict is not None and self.invariants_string_dict != ffi.NULL:
            rust_finder_library.ffi_free_library_string(self.invariants_string_dict)
            self.invariants_string_dict = ffi.NULL
    
    def simplify(self, checker_path, input_file):
            
        this_program: program.Program = program.Program(open(input_file, "rb").read())
        interesting_instructions = []
        executed_instructions = find_executed_instructions(checker_path, input_file, self.first_symbolic_instruction_idx, self.start_address)
        print("Executed instructions before minimization", executed_instructions,"max is", max(executed_instructions) if executed_instructions else None, flush=True)
        logger.info(f"Executed instructions (interesting instructions before minimization): {executed_instructions}")
        commit_log_result = check_commit_log(checker_path, input_file, None, extract_constants=False, check_invariant_satisfaction=False, invariant_ffi_ptr=None)
        check_original_mismatch_index = commit_log_result.difference_location.instruction_number if commit_log_result is not None else None
        print("Check mismatch index before deleting trail", check_original_mismatch_index)
        if len(executed_instructions) == 0:
            raise Exception(f"No executed instructions found, something is wrong. Offset and execution instruction memory index correct? start_addres {self.start_address} symoblic idx {self.first_symbolic_instruction_idx}")
        if any([idx < 0 for idx in executed_instructions]):
            logger.warning(f"Some executed instruction indices are negative, likely due to incorrect offset {self.first_symbolic_instruction_idx}, exec_instructions {executed_instructions}")
        if any([idx>=this_program.get_length() for idx in executed_instructions]):
            logger.warning(f"Some executed instruction indices are out of range (program length {this_program.get_length()}), likely due to incorrect offset {self.first_symbolic_instruction_idx}, exec_instructions {executed_instructions}")
        if all([idx >= this_program.get_length() for idx in executed_instructions]):
            raise Exception(f"All executed instruction indices are out of range (program length {this_program.get_length()}), likely due to incorrect offset {self.first_symbolic_instruction_idx}, exec_instructions {executed_instructions}")
        executed_instructions = [idx for idx in executed_instructions if 0 <= idx < this_program.get_length()]
        interesting_instructions = executed_instructions.copy()
        last_executed_idx = max(executed_instructions) if executed_instructions else None
        print("Last executed idx", last_executed_idx)
        print(flush=True)
        if len(self.freeze_instructions) > 0:
            if last_executed_idx is not None:
                if last_executed_idx < max(self.freeze_instructions):
                    raise Exception(f"Cannot freeze instructions beyond last executed instruction {last_executed_idx}, freeze_instructions {self.freeze_instructions}")
        print("Trying to trim unexecuted tail if any", last_executed_idx, this_program.get_length()-1, "\n", flush=True)
        if last_executed_idx is not None and last_executed_idx < this_program.get_length() - 1:
            logger.info(f"Trimming unexecuted tail after instruction {last_executed_idx}")
            trimmed_bytes, deleted_indices = trim_unexecuted_tail_bytes(this_program.get_code(), last_executed_idx)
            with tempfile.NamedTemporaryFile(suffix=".bin", delete=True) as tmp:
                #Data loads might be affected by trimming, so we need to re-check
                if _still_cex(self.checker_path, trimmed_bytes, tmp.name, invariant_ffi_ptr=self.invariants_string_dict, check_mismatch_index=check_original_mismatch_index):
                    commit_log_result = check_commit_log(checker_path, tmp.name, None, extract_constants=False, check_invariant_satisfaction=False, invariant_ffi_ptr=None)
                    check_mismatch_index_after_delete = commit_log_result.difference_location.instruction_number if commit_log_result is not None else None
                    print("Check mismatch index after deleting trail", check_mismatch_index_after_delete)
                    if check_original_mismatch_index != check_mismatch_index_after_delete:
                        logger.warning("Trimming unexecuted tail changed the mismatch location, skipping trim")
                        interesting_instructions = executed_instructions
                        #return this_program.get_code(), interesting_instructions
                        #return this_program.get_code(), interesting_instructions, []
                    else:
                        logger.info(f"Successfully trimmed unexecuted tail, removed {len(deleted_indices)} instructions")    
                        this_program = program.Program(trimmed_bytes)
                        interesting_instructions = [idx for idx in executed_instructions if idx < this_program.get_length()]
                        logger.info(f"Trimmed program length {this_program.get_length()}, deleted instructions len {len(deleted_indices)}, interesting instructions now {interesting_instructions}")
                else:
                    logger.warning("Trimming unexecuted tail changed the program behavior, skipping trim")
                    interesting_instructions = executed_instructions
        candidate_indices = list(range(0, this_program.get_length()))
        candidate_indices = [idx for idx in candidate_indices if idx not in self.freeze_instructions]
        candidate_indices = [idx for idx in candidate_indices if idx in interesting_instructions]
        accepted_nop_idx, nopped_example, all_benign_examples = batch_nop_substitution(
            checker_path=checker_path,
            orig_bytes=this_program.get_code(),
            candidate_indices=candidate_indices,
            interesting_instructions=interesting_instructions,
            temp_dir=None,
            initial_chunk_size=None,
            invariant_ffi_ptr=self.invariants_string_dict,
        )
        not_nopped_set = set(candidate_indices) - set(accepted_nop_idx)
        logger.info(f"Interesting instructions before batch NOP substitution: {interesting_instructions} not nopped set {not_nopped_set}")
        interesting_instructions= set(interesting_instructions).intersection(not_nopped_set)
        min_interesting_instructions = min(interesting_instructions) if len(interesting_instructions) > 0 else None
        if min_interesting_instructions is None:
            #logger.info("No interesting instructions left after NOP substitution, returning nopped example")
            raise Exception("No interesting instructions left after NOP substitution")
        max_interesting_instructions = max(interesting_instructions)
        interesting_instructions.add(max(min_interesting_instructions-1,0))
        interesting_instructions.add(min(max_interesting_instructions+1,this_program.get_length()-1))   
        interesting_instructions = sorted(interesting_instructions)
        interesting_instructions = [idx for idx in interesting_instructions if idx not in self.freeze_instructions]
        logger.info(f"Interesting instructions after batch NOP substitution: {interesting_instructions} not nopped set {not_nopped_set}")
        #return this_program.get_code(), interesting_instructions, all_benign_examples
        this_program = program.Program(nopped_example)
        #return 
        
        return this_program.get_code(), interesting_instructions, all_benign_examples
        # 2) Batch deletion substitution (replaces per-instruction loop)
        min_distance = interesting_instructions[0]
        if len(interesting_instructions) > 1:
            for i in range(len(interesting_instructions) - 1):
                distance = interesting_instructions[i + 1] - interesting_instructions[i]
                if min_distance is None or distance < min_distance:
                    min_distance = distance
        logger.info(f"Smallest distance between two indices in interesting_instructions: {min_distance}")
        if min_distance is None or min_distance <= 0.1 * this_program.get_length():
            logger.info("Skipping batch deletion substitution since instructions are too close together")
            return this_program.get_code(), interesting_instructions
            #min_distance = interesting_instructions[0]
        min_distance = max(1, min_distance)
        delete_candidates = accepted_nop_idx #list(range(0, this_program.get_length()))
        accepted_delete_idx, deleted_example = batch_delete_substitution_bytes(
            checker_path=checker_path,
            original_bytes=nopped_example,
            candidate_indices=delete_candidates,
            temp_dir=None,
            initial_chunk_size=min_distance,
            one_pass=True,
            invariant_ffi_ptr=self.invariants_string_dict
        )
        # Build final program after deletions
        this_program = program.Program(deleted_example)

        # Remap interesting_instructions after deletions:
        # - drop those that were deleted
        # - shift others by number of deletions before them
        del_sorted = sorted(accepted_delete_idx)
        del_set = set(del_sorted)
        remapped_interesting: list[int] = []
        for idx in interesting_instructions:
            if idx in del_set:
                continue
            # number of deletions strictly before idx
            shift = bisect.bisect_left(del_sorted, idx)
            remapped_interesting.append(idx - shift)
        interesting_instructions = remapped_interesting

        logger.info(f"Interesting instructions after batch deletion: {len(interesting_instructions)}")
        return this_program.get_code(), interesting_instructions
        # For each instruction, check if program is still CEX if we replace it with a NOP
        # If so, keep the nop, otherwise roll back to old instruction
        # And keep going until we iterated through all instructions
        # for idx in range(0, this_program.get_length()):
        #     logger.info(f"Nopping out instruction at {idx}")
        #     old_instruction = this_program.instructions[idx]
        #     this_program.substitute_with_nop_or_append(idx)
        #     with tempfile.NamedTemporaryFile(delete=True) as tmp:
        #         with open(tmp.name, "wb") as fp:
        #             fp.write(this_program.get_code())
        #         check_result = check_commit_log(checker_path, tmp.name, None, check_invariant_satisfaction=True)
        #         if check_result.Kind != CheckerResultKind.CEX:
        #             # Roll back to old instructiond
        #             this_program.instructions[idx] = old_instruction
        #             interesting_instructions.append(idx)
        #         else:
        #             logger.debug(f"Could nop out instruction at {idx} without changing program result {this_program.code_as_hexstring()[:idx*8]}")
        # return this_program.get_code(), interesting_instructions
        # # Try to delete each instruction and check if the program is still a CEX
        # for idx in sorted(range(0, this_program.get_length()), reverse=True):  # Iterate in reverse to avoid index shifting
        #     logger.info(f"Deleting instruction at {idx}")
        #     instruction_backup = this_program.instructions[idx]
        #     this_program.delete_instruction(idx)
        #     with tempfile.NamedTemporaryFile(delete=True) as tmp:
        #         with open(tmp.name, "wb") as fp:
        #             fp.write(this_program.get_code())
        #         check_result = check_commit_log(checker_path, tmp.name, None, check_invariant_satisfaction=True)
        #         if check_result.Kind != CheckerResultKind.CEX:
        #             # Restore the instruction if deleting it changes the outcome
        #             # outcome can also be: Already fulfilled invariant
        #             this_program.instructions.insert(idx, instruction_backup)
        #         else: 
        #             # Adjust all indexes in interesting_instructions after this delete
        #             interesting_instructions = [
        #                 this_idx - 1 if this_idx > idx else this_idx
        #                 for this_idx in interesting_instructions
        #             ]
        # return this_program.get_code(), interesting_instructions, all_benign_examples

    def minimize_example(self, checker_path, input_file, check_invariant_satisfaction: bool = True) -> typing.Tuple[CommitLogCheckerResult, bytes, list[int], list[bytes]]:
        # with tempfile.NamedTemporaryFile(delete=False, prefix="vincent_insight_") as tmp:
        input_file_check_result = check_commit_log(checker_path, input_file, None, extract_constants=True, check_invariant_satisfaction=check_invariant_satisfaction, invariant_ffi_ptr=self.invariants_string_dict)
        if input_file_check_result.Kind != CheckerResultKind.CEX:
            raise Exception(f"Input file is not a counterexample, exiting {input_file_check_result} {checker_path}")
            # print("Checking if waveform fulfills any invariant")
            # maybe_invariant = waveform_fullfills_any_invariant(tmp.name)
            # if maybe_invariant is not None:
            #     raise Exception(f"Input file {input_file} with waveform {tmp.name} is a counterexample but fulfills invariant {maybe_invariant}, exiting")
        expose_instruction_number = input_file_check_result.difference_location.instruction_number
        print("Simplifying example, expose instruction number", expose_instruction_number)
        print("I am minmiming")
        logger.info(f"logger Simplifying example, expose instruction number {expose_instruction_number}")
        simpler_input_program, interesting_instructions_simplified_program, all_benign_examples = self.simplify(checker_path, input_file)
        with tempfile.NamedTemporaryFile(delete=True) as tmp:
            with open(tmp.name, "wb") as fp:
                fp.write(simpler_input_program)
            check_result_simple_program = check_commit_log(checker_path, tmp.name, None, check_invariant_satisfaction=check_invariant_satisfaction, invariant_ffi_ptr=self.invariants_string_dict)
        if check_result_simple_program.Kind != CheckerResultKind.CEX:
            raise Exception(f"Simplified program is not a CEX, exiting {check_result_simple_program}")
        return check_result_simple_program, simpler_input_program, interesting_instructions_simplified_program, all_benign_examples
        
    def analyse(self, minimize=True, check_invariant_satisfaction=False, return_nopped_benign_examples=False) -> typing.Tuple[AnalyzerResult, list[bytes]]:
        print("Analyzing example")
        all_benign_examples = []
        with tempfile.NamedTemporaryFile(delete=False, prefix="vincent_insight_") as tmp:
            input_file_check_result = check_commit_log(self.checker_path, self.input_file_path, tmp.name, extract_constants=True)
            if input_file_check_result.Kind != CheckerResultKind.CEX:
                raise Exception(f"Input file is not a counterexample, exiting {input_file_check_result} {self.checker_path}")
            # print("Checking if waveform fulfills any invariant")
            logger.info(f"Input file difference location: {input_file_check_result.difference_location.instruction_number}")
            if check_invariant_satisfaction is True:
                if waveform_fulfills_any_invariant_from_list(tmp.name, self.invariants_string_dict) is True:
                    maybe_invariant = waveform_fullfills_any_invariant(tmp.name, self.invariants_path)
                    if maybe_invariant is not None:
                        raise Exception(f"Input file {self.input_file_path} with waveform {tmp.name} is a counterexample but fulfills invariant {maybe_invariant}, exiting")
                    else:
                        raise Exception(f"Mismatch between ffi calls. Rust code correct?")
                else:
                    logger.info("Input file does not fulfill any invariant, proceeding with analysis")
        if minimize is True:
            res_check, minimized_example, interesting_instructions, all_benign_examples = self.minimize_example(self.checker_path, self.input_file_path,check_invariant_satisfaction=check_invariant_satisfaction)
        else:
            logging.info("Skipping minimization as per user request")
            res_check = check_commit_log(self.checker_path, self.input_file_path, None, extract_constants=True)
            minimized_example = self.input_file
            this_program = program.Program(minimized_example)
            # interesting_instructions = list(range(0, this_program.get_length()))
            interesting_instructions = find_executed_instructions(self.checker_path, self.input_file_path, self.first_symbolic_instruction_idx, self.start_address)
            if any(idx < 0 or idx>=this_program.get_length() for idx in interesting_instructions):
                logger.warning(f"Some interesting instruction indices are negative, likely due to incorrect offset {self.first_symbolic_instruction_idx}: {interesting_instructions}")
            interesting_instructions = [idx for idx in interesting_instructions if 0 <= idx < this_program.get_length()]
        logger.info(f"Interesting instructions after minimization (or original if minimization skipped): {interesting_instructions}")
        if res_check.Kind != CheckerResultKind.CEX:
            raise Exception(f"Input file is not a counterexample, exiting {res_check}")
        #print("Minimized example is", minimized_example)
        if res_check.difference_location.instruction_number is None:
            raise Exception(f"Difference location is None, exiting {res_check} {self.input_file_path}")

        logger.info(f"Constants: {res_check.constants}")
        # if res_check.difference_location.instruction_number == 0:
        #     program_with_nop = program.Program(minimized_example)
        #     for idx in range(1, max(2,program_with_nop.get_length())):
        #         program_with_nop.substitute_with_nop_or_append(idx)
        #     with tempfile.NamedTemporaryFile(delete=True) as tmp:
        #         with open(tmp.name, "wb") as fp:
        #             fp.write(program_with_nop.get_code())
        #         res_check_nop = check_commit_log(self.checker_path, tmp.name, None, extract_constants=True)
        #     if res_check_nop.Kind == CheckerResultKind.CEX:
        #         bug_type = BugTypeResult.EXPOSE_ONLY
        #         interesting_instructions = [0]
        #         minimized_example = program_with_nop.get_code()
        #     else:
        #         bug_type = BugTypeResult.EXPOSE_AND_SETUP #That is actually a bug which depends on the instruction AFTER the exposing instruction
        #     analysis_result = AnalyzerResult(
        #         kind=res_check.Kind,
        #         minimized_example=minimized_example,
        #         bug_type=bug_type,
        #         difference_location=res_check.difference_location,
        #         interesting_instructions=interesting_instructions,
        #         constants=res_check.constants
        #     )
        #     return analysis_result, [] # minimized_example, bug_type, interesting_instructions, res_check
        analyzer_result = AnalyzerResult(
            kind=res_check.Kind,
            minimized_example=minimized_example,
            bug_type=BugTypeResult.EXPOSE_AND_SETUP,  # Will be determined later
            difference_location=res_check.difference_location,
            interesting_instructions=interesting_instructions,
            constants=res_check.constants
        )
        logger.info(f"Interesting instructions {interesting_instructions}")
        # logger.info(f"Analyzer result: {analyzer_result}, kind:  {analyzer_result.kind}")
        if return_nopped_benign_examples is True:
            return analyzer_result, all_benign_examples #minimized_example, bug_type, interesting_instructions, res_check
        else:
            return analyzer_result, []  # minimized_example, bug_type, interesting_instructions, res_check
                    
        #First: Let's find out the "bug type"
        #bug_type = None
        #if res_check.difference_location.instruction_number == 0:
        #    bug_type = CheckerResultKind.EXPOSE_ONLY
        #else:

if __name__ == "__main__":
    #print(check_commit_log(sys.argv[1],sys.argv[2], sys.argv[3]))
    # print(waveform_fullfills_any_invariant("output/cexs/waveforms/minimized_cex.vcd"))
    # print(waveform_fullfills_any_invariant("output/cexs/waveforms/minimized_cex.vcd"))
    # print(waveform_fullfills_any_invariant("output/cexs/waveforms/minimized_cex.vcd"))
    # exit(0)
    logging.basicConfig(
        level=logging.DEBUG,
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
        handlers=[
            logging.StreamHandler(sys.stdout)
        ]
    )
    logger.setLevel(logging.DEBUG)
    # first_symbolic_instruction_idx = 2
    # start_address = 0x80000080
    checker_path = sys.argv[1]
    if not os.path.exists(checker_path):
        raise Exception(f"Checker path {checker_path} does not exist")
    testcase = sys.argv[2]
    if not os.path.exists(testcase):
        raise Exception(f"Testcase path {testcase} does not exist")
    configs_path = sys.argv[3]
    if not os.path.exists(configs_path):
        raise Exception(f"Configs path {configs_path} does not exist")
    if len(sys.argv) >= 3:
        config = json.load(open(configs_path, "r"))
        print(f"Loaded config from {configs_path}")
        regex_config = json.load(open(configs_path, "r"))
        if "first_symbolic_instruction_idx" in config:
            first_symbolic_instruction_idx = config["first_symbolic_instruction_idx"]
        if "start_address" in config:
            start_address = int(config["start_address"],16)
        else: 
            start_address = 0x80000080
        print("config is", config)
        print("start address", hex(start_address))
    #output_path = config["output"]
    if len(sys.argv) >= 5:
        invariant_path = sys.argv[4]
        if not os.path.exists(invariant_path):
            raise Exception(f"Invariant path {invariant_path} does not exist")
        check_invariant_satisfaction = True
    else:
        invariant_path = None
        check_invariant_satisfaction = False
            
    logger.info(f"Starting analysis with first_symbolic_instruction_idx={first_symbolic_instruction_idx} and start_address={hex(start_address)}")
    analyser = BugAnalyzer(checker_path, testcase, invariant_path, first_symbolic_instruction_idx=first_symbolic_instruction_idx, start_address=start_address)
    res, _ = analyser.analyse(minimize=True, check_invariant_satisfaction=check_invariant_satisfaction)
    print(f"Result of analysis: {res}")
    with tempfile.NamedTemporaryFile(delete=False, prefix="minimized_example_", suffix=".bin") as tmp_file:
        tmp_file.write(res.minimized_example)
        print(f"Minimized example written to: {tmp_file.name}")
    #   print(f"{#analyser.analyse(minimize=True,check_invariant_satisfaction=True )}")
