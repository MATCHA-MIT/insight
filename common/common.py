import json
import enum
import cffi
import constants as constants_module
import logging
import sys
import shutil
import os
from pathlib import Path


subdir = Path(__file__).parent.parent / "mutation_engine" 
sys.path.insert(0, str(subdir))

subdir = Path(__file__).parent.parent
sys.path.append(str(subdir))

subdir_common = Path(__file__).parent.parent / "common"
sys.path.append(str(subdir_common))
subdir_plotting = Path(__file__).parent.parent / "plotting"
sys.path.append(str(subdir_plotting))
subdir_orch = Path(__file__).parent.parent / "orchestration"
sys.path.append(str(subdir_orch))


from riscv_instruction_mutator import InstructionMutatorWrapper, utils, program, FileSource

import vcd_trace
import tempfile
import subprocess

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

ffi = cffi.FFI()
rust_finder_library = ffi.dlopen(constants_module.INVARIANT_FINDER_LIBRARY_PATH)
ffi.cdef('bool ffi_check_on_waveform(char *waveform_path, char *invariant_json_path, char *clock_signal);')
ffi.cdef("char *check_cex_items_against_invariants(const char *cex_items, const char *invariant_directory);")
ffi.cdef("void ffi_free_library_string(const char *ptr);")

class EnumEncoder(json.JSONEncoder):
    def default(self, obj):
        if isinstance(obj, enum.Enum):
            return f"{obj.__class__.__name__}.{obj.name}"
        return super().default(obj)

# ---- Deserialization ----
def enum_decoder(dct):
    for key, value in dct.items():
        if isinstance(value, str) and "." in value:
            enum_name, member_name = value.split(".", 1)
            # Map string to actual Enum class
            if enum_name in globals() and issubclass(globals()[enum_name], enum.Enum):
                enum_class = globals()[enum_name]
                dct[key] = enum_class[member_name]
    return dct

def identify_covered_cex_items_through_ffi_call(cex_items, invariant_directory, enum_to_string=False):
    """
    cex_items: A list of cex items to filter
    invariant_directory: The directory where the invariants are stored
    Returns a list of cex items that are not covered by any invariant
    """
    if type(cex_items) == str:
        print("I need to convert cex_items to string to call ffi")
        cex_items = cex_items.encode()
    elif type(cex_items) == list:
        print("I need to convert cex_items to string to call ffi")
        cex_items = json.dumps(cex_items, cls=EnumEncoder).encode()
    else:
        raise Exception("cex_items should be a string or a list, but got {type(cex_items)}")
    if type(invariant_directory) == str:
        print("I need to convert invariant_directory to string to call ffi")
        invariant_directory = invariant_directory.encode()
    #logger.debug(f"Calling ffi.check_cex_items_against_invariants with cex_items {cex_items} and invariant_directory {invariant_directory}")
    ret_string = ffi.NULL
    try:
        ret_string = rust_finder_library.check_cex_items_against_invariants(cex_items, invariant_directory)
        if ret_string == ffi.NULL:
            raise Exception("check_cex_items_against_invariants returned NULL, this should not happen")
        if enum_to_string:
            res_list = json.loads(ffi.string(ret_string).decode("utf-8"))
        else:
            res_list = json.loads(ffi.string(ret_string).decode("utf-8"), object_hook=enum_decoder)
        rust_finder_library.ffi_free_library_string(ret_string)
        return res_list
    except Exception as e:
        logger.error(f"Exception in filter_cex_items_through_ffi_call: {e}")
        raise e
    
def identify_non_covered_cex_items_through_ffi_call(cex_items, invariant_directory, enum_to_string=False, look_at_invariants_only=None):
    """
    cex_items: A list of cex items to filter
    invariant_directory: The directory where the invariants are stored
    Returns a list of cex items that are not covered by any invariant
    """
    if look_at_invariants_only is not None: 
        #Create temporary directory and copy valid_only.json from invariant_directory. Then call with 
        #directory
        with tempfile.TemporaryDirectory() as temp_dir:
            #valid_only_path = Path(invariant_directory) / "valid_only.json"
            #if not valid_only_path.exists():
            #    raise Exception(f"valid_only.json does not exist in {invariant_directory}, cannot look at valid only")
            #temp_invariant_directory = Path(temp_dir)
            #shutil.copy(valid_only_path, temp_invariant_directory / "valid_only.json")
            for item in look_at_invariants_only:
                basename_of_path = os.path.basename(item)
                print(f"Copying {Path(invariant_directory) / basename_of_path} to temporary directory {temp_dir}")
                shutil.copy(Path(invariant_directory) / basename_of_path, Path(temp_dir) / basename_of_path)
            print("Filtered out invariants to look at:", look_at_invariants_only)
            covered = identify_covered_cex_items_through_ffi_call(cex_items, str(Path(temp_dir)), enum_to_string)
        
    else:
        covered = identify_covered_cex_items_through_ffi_call(cex_items, invariant_directory, enum_to_string)
    print("Covered items:", "\n".join([str(item['file']) for item in covered]))
    not_covered = []
    covered_files = set(item['file'] for item in covered)
    for item in cex_items:
        if item['file'] not in covered_files:
            not_covered.append(item)

    return not_covered
    # if type(cex_items) == str:
    #     print("I need to convert cex_items to string to call ffi")
    #     cex_items = cex_items.encode()
    # elif type(cex_items) == list:
    #     print("I need to convert cex_items to string to call ffi")
    #     cex_items = json.dumps(cex_items, cls=EnumEncoder).encode()
    # else:
    #     raise Exception("cex_items should be a string or a list, but got {type(cex_items)}")
    # if type(invariant_directory) == str:
    #     print("I need to convert invariant_directory to string to call ffi")
    #     invariant_directory = invariant_directory.encode()
    # #logger.debug(f"Calling ffi.check_cex_items_against_invariants with cex_items {cex_items} and invariant_directory {invariant_directory}")
    # ret_string = ffi.NULL
    # try:
    #     ret_string = rust_finder_library.check_cex_items_against_invariants(cex_items, invariant_directory)
    #     if ret_string == ffi.NULL:
    #         raise Exception("check_cex_items_against_invariants returned NULL, this should not happen")
    #     if enum_to_string:
    #         res_list = json.loads(ffi.string(ret_string).decode("utf-8"))
    #     else:
    #         res_list = json.loads(ffi.string(ret_string).decode("utf-8"), object_hook=enum_decoder)
    #     rust_finder_library.ffi_free_library_string(ret_string)
    #     return res_list
    # except Exception as e:
    #     logger.error(f"Exception in filter_cex_items_through_ffi_call: {e}")
    #     raise e

def write_inst_little_endian(inst: int, output_file: str):
    """Write a 32-bit instruction as 4 bytes in little-endian order."""
    with open(output_file, "ab") as f:
        f.write(inst.to_bytes(4, byteorder="little", signed=False))

class WaveformExtractor():
    def __init__(self, memory_signal_ref_format_string_jg=None,
                 memory_signal_dut_format_string_jg=None,
                 memory_size=32,
                 sodor_offset=0,
                 dut_offset=0,
                 first_symbolic_instruction_idx=0,
                 current_max_cex_length=None):
        self.memory_signal_ref_format_string_jg = memory_signal_ref_format_string_jg
        self.memory_signal_dut_format_string_jg = memory_signal_dut_format_string_jg
        self.memory_size = memory_size
        self.sodor_offset = sodor_offset
        self.dut_offset = dut_offset
        self.first_symbolic_instruction_idx = first_symbolic_instruction_idx
        self.current_max_cex_length = current_max_cex_length

    def extract_instructions(self, vcd_filepath):
        MEMORY_SIGNALS = []
        v = vcd_trace.vcdTrace(vcd_filepath, "correctness.clk")
        signals = v.vcd.references_to_ids.keys()
        if self.memory_signal_ref_format_string_jg is None:
            raise Exception("memory_signal_ref_format_string_jg is None, cannot extract instructions")
        if self.current_max_cex_length is None:
            raise Exception("current_max_cex_length is None, cannot extract instructions. Initialize it first")
        for i in range(0,64):
            MEMORY_SIGNALS.append(self.memory_signal_ref_format_string_jg.format(idx=i))
        other_core_mem_string = self.memory_signal_dut_format_string_jg
        instructions = []
        if self.memory_size == 64:
            # For 64-bit memory: 2 instructions per memory location
            # Each memory location contains 64 bits = 2 × 32-bit instructions
            for i in range(0, (self.current_max_cex_length + 1) // 2):
                memory_idx = self.sodor_offset + i + self.first_symbolic_instruction_idx // 2
                # Get the full 64-bit value from memory
                full_memory_value = int(v.get_signal_value_at_cycle_str(MEMORY_SIGNALS[memory_idx], 0))
                
                # Extract two 32-bit instructions from the 64-bit memory value
                # Lower 32 bits (instruction 0) and upper 32 bits (instruction 1)
                instruction_0 = full_memory_value & 0xFFFFFFFF
                instruction_1 = (full_memory_value >> 32) & 0xFFFFFFFF
                
                # Add instructions in the correct order
                if (i * 2) < self.current_max_cex_length:
                    instructions.append(instruction_0) #int_to_hex_with_zeros(instruction_0, 8))
                if (i * 2 + 1) < self.current_max_cex_length:
                    instructions.append(instruction_1) #int_to_hex_with_zeros(instruction_1, 8))
                
                # Verify with DUT if available
                if other_core_mem_string is not None:
                    dut_memory_idx = self.dut_offset + i + self.first_symbolic_instruction_idx // 2
                    dut_memory_value = int(v.get_signal_value_at_cycle_str(other_core_mem_string.format(idx=dut_memory_idx), 0))
                    if full_memory_value != dut_memory_value:
                        raise Exception(f"Warning: Memory values do not match between cores at memory index {i}: {hex(full_memory_value)} != {hex(dut_memory_value)}")
        else:
            # For 32-bit memory: 1 instruction per memory location
            for i in range(0, self.current_max_cex_length):
                hex_string_length = 8
                memory_idx = self.sodor_offset + i + self.first_symbolic_instruction_idx
                ref_core_instruction = int(v.get_signal_value_at_cycle_str(MEMORY_SIGNALS[memory_idx], 0)) #int_to_hex_with_zeros(int(v.get_signal_value_at_cycle_str(MEMORY_SIGNALS[memory_idx], 0)), hex_string_length)
                if other_core_mem_string is not None:
                    dut_memory_idx = self.dut_offset + i + self.first_symbolic_instruction_idx
                    other_core_instruction = int(v.get_signal_value_at_cycle_str(other_core_mem_string.format(idx=dut_memory_idx), 0)) #int_to_hex_with_zeros(int(v.get_signal_value_at_cycle_str(other_core_mem_string.format(idx=dut_memory_idx), 0)), hex_string_length)
                    if ref_core_instruction != other_core_instruction:
                       raise Exception(f"Warning: Instructions do not match between cores at index {i}: {ref_core_instruction} != {other_core_instruction}")
                instructions.append(ref_core_instruction)
        return instructions

    def extract_instructions_and_disassemble(self, vcd_filepath, save_file=True):
        instructions = self.extract_instructions(vcd_filepath)
        with tempfile.NamedTemporaryFile(delete=(not save_file), suffix='.bin') as tmp_bin_file:
            logger.debug(f"Writing to {tmp_bin_file.name}")
            temp_bin_path = tmp_bin_file.name
            for inst in instructions:
                logger.debug(f"inst_hex {hex(inst)}")
                write_inst_little_endian(inst, temp_bin_path)
            #shutil.copy(temp_bin_path, os.path.join(constants.JG_FOUND_CEXS, os.path.basename(temp_bin_path)))
            #subprocess.run(["./scripts/run_ibex.sh", temp_bin_path, os.path.abspath("/tmp/program.vcd")]) 
            subprocess.run(["./util_scripts/disassemble_objdump.sh", temp_bin_path])
        logger.debug(f"Program is in {tmp_bin_file.name}")
        return temp_bin_path if save_file else None 
    