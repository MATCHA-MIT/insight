#include <stdlib.h>
#include <iostream>
#include <sstream>
#include <string>
#include <verilated.h>
#include <chrono>
#include <iomanip>
#include <fstream>
#include <vector>
#include <cstdio>
#include <cstring>
#include "Vcorrectness.h"
#include "Vcorrectness___024root.h"
// #include <verilated_vcd_c.h>
#include "Vcorrectness__Dpi.h"
#include "verilated_fst_c.h"
#include <optional>
#include <sys/resource.h>

VL_ATTR_COLD void Vcorrectness___024root___eval_settle(Vcorrectness___024root* vlSelf);
void Vcorrectness___024root___eval_initial(Vcorrectness___024root* vlSelf);

using namespace std;

#ifndef MAX_SIM_TIME
#define MAX_SIM_TIME 10000
#endif

#ifndef MEMSIZE
#define MEMSIZE 1048576 // Memory size in bytes
#endif

#ifndef START_INDEX_MEMORY
#define START_INDEX_MEMORY 32 // Start index for user instructions in memory
#endif

#ifndef DUT_CORE_MEM_STRING
#define DUT_CORE_MEM_STRING correctness__DOT__kronos__DOT__dut__DOT__mem
#endif

#ifndef REF_CORE_MEM_STRING
#define REF_CORE_MEM_STRING correctness__DOT__sodor_core__DOT__memory__DOT__mem_ext__DOT__Memory
#endif

#ifndef ADDRESS_WIDTH
#define ADDRESS_WIDTH 32
#endif

#ifndef DEFAULT_INSTRUCTION
#define DEFAULT_INSTRUCTION 0x0e808093 // 0x0e808093 // addi x1, x1
#endif

#define MAX_NUM_INSTRUCTIONS (MEMSIZE / 4) //max num instructions can be adjusted as needed

static_assert(std::is_integral<decltype(MEMSIZE)>::value, "MEMSIZE must be an integer.");
static_assert(std::is_integral<decltype(MAX_SIM_TIME)>::value, "MAX_SIM_TIME must be an integer.");
static_assert(std::is_integral<decltype(START_INDEX_MEMORY)>::value, "START_INDEX_MEMORY must be an integer.");\
static_assert(MEMSIZE % 4 == 0, "MEMSIZE must be a multiple of 4.");
static_assert(START_INDEX_MEMORY >= 0 && START_INDEX_MEMORY < MAX_NUM_INSTRUCTIONS, "START_INDEX_MEMORY must be between 0 and MAX_NUM_INSTRUCTIONS-1.");
static_assert(ADDRESS_WIDTH == 32 || ADDRESS_WIDTH == 64, "Address width must be 32 or 64");


void print_setting_mem_debug(uint32_t idx, uint32_t val, string mem_name="mem") {
    #if defined(DEBUG) && DEBUG != 0 
        std::cout << "Setting " << mem_name << "[" << idx << "] to 0x" << std::hex << val << std::dec << std::endl; 
    #endif 
}

#define SET_DUT_MEMORY(dutp, idx, val) \
    print_setting_mem_debug((idx), (val), "DUT_CORE_MEM_STRING"); (dutp)->DUT_CORE_MEM_STRING[(idx)] = (val); 

#define GET_DUT_MEMORY(dutp, idx) \
    (dutp)->DUT_CORE_MEM_STRING[(idx)]

#define SET_REF_MEMORY(dutp, idx, val) \
    print_setting_mem_debug((idx), (val), "REF_CORE_MEM_STRING"); (dutp)->REF_CORE_MEM_STRING[(idx)] = (val);

#define GET_REF_MEMORY(dutp, idx) \
    (dutp)->REF_CORE_MEM_STRING[(idx)]
    
    
// #define SET_DUT_MEMORY_STRING(index, value) dut->DUT_CORE_MEM_STRING[index] = value

struct ExecutionResult {
    u_int8_t execution_finished;
    u_int8_t correct;
    uint32_t mismatch_index;
    uint32_t mismatch_instruction_idx;
    uint32_t mismatch_cycle_dut;
    uint32_t mismatch_cycle_ref;
    uint64_t *constants;
};

void vl_finish(const char* filename, int linenum, const char* hier) VL_MT_UNSAFE {
    // hier is unused in the default implementation.
    (void)hier;
    //VL_PRINTF(  // Not VL_PRINTF_MT, already on main thread
    //    "- %s:%d: Verilog $finish Vincent\n", filename, linenum);
    Verilated::threadContextp()->gotFinish(true);
}


void set_hex_value(std::string hexStr, uint32_t *mem_value){
    uint64_t value;
    std::stringstream ss;
    if (hexStr.length() < 8) {
        hexStr.insert(0, 8 - hexStr.length(), '0');
    }

    ss << std::hex << hexStr;
    ss >> value;

    *mem_value = value;
    //std::cout << "32-bit value at " << std::hex << mem_value << ": " << std::hex << *mem_value << std::endl;
}

void set_memory_at_index(Vcorrectness___024root *dut, u_int32_t index, u_int32_t value){
    //If 64-bit memory, set two 32-bit values
    #if defined(DEBUG) && DEBUG != 0 
        std::cout << "Filling memory at index " << index << " with NOP" << "max num" << MAX_NUM_INSTRUCTIONS << "memsize " << MEMSIZE << std::endl;
    #endif
    if (ADDRESS_WIDTH == 64) {
        // std::cout << "setting value " << std::hex << value << " at " << std::hex << index << std::endl;
    	if (index%2 == 0) {
                u_int32_t new_index = index / 2;
        	    uint64_t old_value = GET_DUT_MEMORY(dut,new_index);
                uint64_t new_value = (old_value & 0xFFFFFFFF00000000) | value;
                SET_DUT_MEMORY(dut, new_index, new_value);
                uint64_t sodor_old_value = GET_REF_MEMORY(dut, new_index);
                SET_REF_MEMORY(dut, new_index, (sodor_old_value & 0xFFFFFFFF00000000) | value);
    	} else {
                u_int32_t new_index = (index-1) / 2;
                uint64_t old_value = GET_DUT_MEMORY(dut, new_index);
                uint64_t new_value = (old_value & 0x00000000FFFFFFFF) | ((uint64_t)value << 32);
        	SET_DUT_MEMORY(dut, new_index, new_value);
                uint64_t sodor_old_value = GET_REF_MEMORY(dut, new_index);
        	SET_REF_MEMORY(dut, new_index, (sodor_old_value & 0x00000000FFFFFFFF) | ((uint64_t)value << 32));
   }
    } else {
        SET_DUT_MEMORY(dut,index,value);
        SET_REF_MEMORY(dut, index, value);
    }
}


//Use an additional external init
#if defined(PROVIDE_EXTERNAL_INIT) && PROVIDE_EXTERNAL_INIT != 0
extern "C" void provide_additional_init_dut (Vcorrectness___024root *dut);
#else
#endif


bool initialize_dut (Vcorrectness___024root *dut, vector<uint32_t> instructions, bool no_stdout = false){
    //correctness__DOT__sodor_core__DOT__memory__DOT__mem_ext__DOT__Memory
    #if defined(PROVIDE_EXTERNAL_INIT) && PROVIDE_EXTERNAL_INIT != 0
    	//std::cout << "Using external init function to initialize DUT memory" << std::endl;
        provide_additional_init_dut(dut);
    #endif
    int num_instr = instructions.size();
    if (num_instr > (MAX_NUM_INSTRUCTIONS - START_INDEX_MEMORY*2)) {
        if (!(no_stdout)) {
            std::cerr << "Error: Number of instructions (" << num_instr << ") exceeds available memory space (" << (MAX_NUM_INSTRUCTIONS - START_INDEX_MEMORY*2) << ")." << std::endl;
            std::cerr << "I will only be able to load " << (MAX_NUM_INSTRUCTIONS - START_INDEX_MEMORY*2) << " instructions." << std::endl;
        }
        //return false;
    }
    num_instr = std::min(num_instr, (MAX_NUM_INSTRUCTIONS - START_INDEX_MEMORY*2));
    int i = 0;
    // #ifndef CASCADE
    //     #if defined(DEBUG) && DEBUG != 0 
    //         std::cout << "Initializing memory... with fixed instructions" << std::endl;
    //     #endif
    //     for (i = 0; i < START_INDEX_MEMORY; i++) {
    //         //dut->correctness__DOT__kronos__DOT__dut__DOT__mem[i] = 0x0E800093; // add x0,x0,x0
    //         //SET_DUT_MEMORY(dut,i,0x0E800093);
    //         //dut->correctness__DOT__sodor_core__DOT__memory__DOT__mem_ext__DOT__Memory[i] = 0x0E800093;
    //         set_memory_at_index(dut, i,DEFAULT_INSTRUCTION); // 0x0E800093);
    //     }

    //     //--- Initialization sequence ---
    //     //lui   x1,0x80000 or lui x1, 16
    //     #if ADDRESS_WIDTH == 64
    //         set_memory_at_index(dut, i, 0x000100b7);
    //     #else
    //         set_memory_at_index(dut, i, 0x800000b7);
    //     #endif
    //     i++;

    //     // csrw  mtvec,x1
    //     set_memory_at_index(dut,i,0x30509073);
    //     i++;

    //     set_memory_at_index(dut,i,0xfff00293); // addi x5, x0, -1
    //     i++;
    //     set_memory_at_index(dut,i,0x3b029073); // csrrw x0, pmpaddr0, x5
    //     i++;
    //     set_memory_at_index(dut,i,0x3a029073); // csrrw x0, pmpcfg0, x5
    //     i++;
    //     set_memory_at_index(dut,i,0x34109073); // csrrw x0, mepc, x1
    //     i++;
    //     // set_memory_at_index(dut,i,0x34109073); 
    //     // i++;
    // #endif

    // --- User instructions ---
    for(int j = 0; j < num_instr; j++) {
        // #if defined(DEBUG) && DEBUG != 0
        // std::cout << "Filling memory at index " << i << " with instruction " << std::hex << instructions[j] << std::endl;
        // #endif
        set_memory_at_index(dut, i, instructions[j]);
        i++;
    }

    // --- Fill the rest with nops ---
    for(; i < MAX_NUM_INSTRUCTIONS-1; i++) {
        set_memory_at_index(dut, i, DEFAULT_INSTRUCTION); //0x0E800093);
    }
    set_memory_at_index(dut, i, 0x0000006f); // endless loop
    return true;
}

vector<uint32_t> parse_instructions(char* filename){
    // Open file as binary
    ifstream file(filename, ios::binary | ios::ate);

    if (!file.is_open()){
        if (!getenv("DISABLE_PRINTING")) {
            cout << "Could not open file " << filename << endl;
        }
        exit(1);
    }
    // std::cout << "Parsing instructions from file: " << filename << std::endl;

    vector<uint32_t> instructions;
    std::streamsize file_size = file.tellg();
    if (file_size > 0) {
        instructions.reserve(static_cast<size_t>(file_size / sizeof(uint32_t)));
    }
    file.seekg(0, ios::beg);

    uint32_t instruction;
    while(file.read(reinterpret_cast<char*>(&instruction), sizeof(uint32_t))){
        instructions.push_back(instruction);
    }

    file.close();
    return instructions;
}

uint64_t *get_constants(Vcorrectness___024root *dut){
    uint32_t last_commit_sodor = dut->correctness__DOT__correctness_inst__DOT__commit_counter_ref;
    uint32_t last_commit_dut = dut->correctness__DOT__correctness_inst__DOT__commit_counter_dut;
    uint32_t max_commit = std::max(last_commit_sodor, last_commit_dut);
    uint64_t *constants = (uint64_t*) calloc(max_commit, sizeof(uint64_t));
    for(int i=0;i<max_commit;++i){
        //constants[i] = dut->correctness__DOT__commit_log_sodor[i].__PVT__commit_data;
        constants[i] = dut->correctness__DOT__correctness_inst__DOT__dut_constants[i];
    }
    return constants;
}

extern "C" void free_result_struct(struct ExecutionResult* result){
    if (result){
        if (result->constants){
            free(result->constants);
            result->constants = NULL;
        }
        free(result);
    }
}

#if defined(BOOM) && BOOM != 0
void set_tlb_entries_for_boom(Vcorrectness___024root *dut){
    // Set TLB entries for BOOM core if needed
    // This is a placeholder function and should be implemented as per requirements
    // Set the TLB entry data for BOOM's frontend TLB
    // Setting sectored_entries_0_0_data_T[41:0] to 0x80
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_0_data_T = 0x80ULL;
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_1_data_T = 0x80ULL;
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_2_data_T = 0x80ULL;
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_3_data_T = 0x80ULL;
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_4_data_T = 0x80ULL;
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_5_data_T = 0x80ULL;
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_6_data_T = 0x80ULL;
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_7_data_T = 0x80ULL;
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__frontend__DOT__tlb__DOT___sectored_entries_0_8_data_T = 0x80ULL;
    // // Set the TLB entry data for BOOM's LSU DTLB
    // dut->correctness__DOT__boom__DOT__dut__DOT__BoomTile__DOT__lsu__DOT__dtlb__DOT___sectored_entries_0_data_T = 0xbe;
}
#endif

extern "C" ExecutionResult* run_simulation_inner(int argc, char** argv, char** envp);

extern "C" ExecutionResult* run_simulation(int argc, char** argv, char** envp) {
    auto start_time = std::chrono::high_resolution_clock::now();
    ExecutionResult* res = run_simulation_inner(argc, argv, envp);
    auto end_time = std::chrono::high_resolution_clock::now();
    std::chrono::duration<double> elapsed = end_time - start_time;
    //if (!getenv("DISABLE_PRINTING")) {
    //    std::cerr << "Total simulation time: " << elapsed.count() << " seconds" << std::endl;
    //}
    return res;
}

extern "C" ExecutionResult* run_simulation_inner(int argc, char** argv, char** envp){
    #if defined(DEBUG) && DEBUG != 0 
        std::cout << "Debug mode is ON" << std::endl;
        std::cout << "Memsize: " << MEMSIZE << std::endl;
        std::cout << "Max sim time: " << MAX_SIM_TIME << std::endl;
    #endif
    vluint64_t sim_time = 0;
    struct ExecutionResult* result = (struct ExecutionResult*) calloc(1, sizeof(struct ExecutionResult));
    result->execution_finished = 0;
    result->correct = 1;
    result->mismatch_index = 0;
    result->mismatch_cycle_dut = 0;
    result->mismatch_cycle_ref = 0;
    result->constants = NULL;
    if (argc < 2){
        // if (!getenv("DISABLE_PRINTING")) {
            std::cout<<"Usage: ./Vcorrectness <filename>"<<std::endl;
        // }
        return result;
    }

 
    // std::cout << "Input file: " << argv[1] << std::endl;
    // std::cout << "Next arg: " << (argc > 2 ? argv[2] : "None") << std::endl;
    // for (int i = 0; i < argc; ++i) {
    //     std::cout << "Arg " << i << ": " << argv[i] << std::endl;
    // }

    vector<uint32_t> instructions = parse_instructions(argv[1]);
    // Diagnostics: optional timing and rusage counters (maj/min faults, context switches)
    struct RUsageStats { long majflt; long minflt; long nvcsw; long nivcsw; };
    auto get_rusage_stats = []() -> RUsageStats {
        struct rusage ru;
        RUsageStats s = {0,0,0,0};
        if (getrusage(RUSAGE_SELF, &ru) == 0) {
            s.majflt = ru.ru_majflt;
            s.minflt = ru.ru_minflt;
            s.nvcsw = ru.ru_nvcsw;
            s.nivcsw = ru.ru_nivcsw;
        }
        return s;
    };
    bool diag = (getenv("VERILATOR_DIAG") != NULL);
    std::chrono::high_resolution_clock::time_point t_before_inst;
    RUsageStats r_before_inst = {0,0,0,0};
    if (diag) {
        t_before_inst = std::chrono::high_resolution_clock::now();
        r_before_inst = get_rusage_stats();
    }

    bool trace_requested = false;
    for (int argi = 1; argi < argc; ++argi) {
        const char* arg = argv[argi];
        if (arg != nullptr && std::strncmp(arg, "+waveform=", 10) == 0 && arg[10] != '\0') {
            trace_requested = true;
            break;
        }
    }

    // Instantiate the DUT
    const std::unique_ptr<VerilatedContext> contextp{new VerilatedContext};
    if (trace_requested) {
        Verilated::traceEverOn(true);
        contextp->traceEverOn(trace_requested);
    }
    //Verilated::traceEverOn(true);
    contextp->commandArgs(argc, argv);

    const std::unique_ptr<Vcorrectness> top = std::make_unique<Vcorrectness>(contextp.get());
    if (diag) {
        auto t_after_inst = std::chrono::high_resolution_clock::now();
        RUsageStats r_after_inst = get_rusage_stats();
        std::chrono::duration<double> inst_elapsed = t_after_inst - t_before_inst;
        std::cerr << "VERILATOR_DIAG: instantiation time=" << inst_elapsed.count()
                  << "s minflt_delta=" << (r_after_inst.minflt - r_before_inst.minflt)
                  << " majflt_delta=" << (r_after_inst.majflt - r_before_inst.majflt)
                  << " nvcsw_delta=" << (r_after_inst.nvcsw - r_before_inst.nvcsw)
                  << " nivcsw_delta=" << (r_after_inst.nivcsw - r_before_inst.nivcsw)
                  << std::endl;
    }
    #if defined(DEBUG) && DEBUG != 0 
        std::cout << "DUT instantiated at pointer: " << top.get() << std::endl;
    #endif
    // VerilatedVcdC* tfp = nullptr;
      // VerilatedVcdC* tfp = new VerilatedFstC; //VerilatedVcdC;
    VerilatedFstC *tfp = nullptr;   //new VerilatedFstC; // Use FST for better performance
    //top->trace(tfp, 99);
    


    // Generate name for waveform file from argv[1]
    // Generate name for waveform file from argv[1]

    const char *no_stdout_arg = contextp->commandArgsPlusMatch("no_stdout");
    bool no_stdout = (no_stdout_arg != nullptr && no_stdout_arg[0] != '\0');

    bool extract_constants = false;
    const char *extract_constants_arg = contextp->commandArgsPlusMatch("extract_constants");
    extract_constants = (extract_constants_arg != nullptr && extract_constants_arg[0] != '\0');

    bool keep_going = false;
    const char *keep_going_arg = nullptr;
    keep_going_arg = contextp->commandArgsPlusMatch("keep-going");
    if (keep_going_arg != nullptr && keep_going_arg[0] != '\0') {
        keep_going = true;
    }
    bool debug_plusarg = false;
    const char *debug_arg = contextp->commandArgsPlusMatch("debug=1");
    if (debug_arg != nullptr && debug_arg[0] != '\0') {
        debug_plusarg = true;
    }
    if (keep_going) {
        
            std::cout << "Keep going mode enabled. Simulation will continue after mismatches." << std::endl;
    }    
    // } else {
    //         std::cout << "Keep going mode disabled. Simulation will stop at first mismatch." << std::endl;
    // }
    // for (int i = 0; i < argc; ++i) {
        
    //         std::cout << "Arg " << i << ": " << argv[i] << std::endl;
        
    // }

    std::optional<std::string> waveform_file = std::nullopt;
    const char *plus_arg_waveform = contextp->commandArgsPlusMatch("waveform=");
    if (plus_arg_waveform != nullptr && plus_arg_waveform[0] != '\0') {
        waveform_file = std::make_optional<std::string>(std::string(plus_arg_waveform + 10)); // Skip "+waveform="
    }
    if (waveform_file.has_value()){
        std::chrono::high_resolution_clock::time_point t_before_wf;
        RUsageStats r_before_wf = {0,0,0,0};
        if (diag) {
            t_before_wf = std::chrono::high_resolution_clock::now();
            r_before_wf = get_rusage_stats();
        }
        tfp = new VerilatedFstC;
        top->trace(tfp, 99);
        tfp->open(waveform_file.value().c_str());
        if (diag) {
            auto t_after_wf = std::chrono::high_resolution_clock::now();
            RUsageStats r_after_wf = get_rusage_stats();
            std::chrono::duration<double> wf_elapsed = t_after_wf - t_before_wf;
            std::cerr << "VERILATOR_DIAG: waveform open time=" << wf_elapsed.count()
                      << "s minflt_delta=" << (r_after_wf.minflt - r_before_wf.minflt)
                      << " majflt_delta=" << (r_after_wf.majflt - r_before_wf.majflt)
                      << " nvcsw_delta=" << (r_after_wf.nvcsw - r_before_wf.nvcsw)
                      << " nivcsw_delta=" << (r_after_wf.nivcsw - r_before_wf.nivcsw)
                      << std::endl;
        }
    }

    std::chrono::high_resolution_clock::time_point t_before_init;
    RUsageStats r_before_init = {0,0,0,0};
    if (diag) {
        t_before_init = std::chrono::high_resolution_clock::now();
        r_before_init = get_rusage_stats();
    }

    bool success = initialize_dut(top->rootp, instructions, no_stdout);   
    
    if (diag) {
        auto t_after_init = std::chrono::high_resolution_clock::now();
        RUsageStats r_after_init = get_rusage_stats();
        std::chrono::duration<double> init_elapsed = t_after_init - t_before_init;
        std::cerr << "VERILATOR_DIAG: initialize_dut time=" << init_elapsed.count()
                  << "s minflt_delta=" << (r_after_init.minflt - r_before_init.minflt)
                  << " majflt_delta=" << (r_after_init.majflt - r_before_init.majflt)
                  << " nvcsw_delta=" << (r_after_init.nvcsw - r_before_init.nvcsw)
                  << " nivcsw_delta=" << (r_after_init.nivcsw - r_before_init.nivcsw)
                  << std::endl;
    }
    if (!success){
        // if (!getenv("DISABLE_PRINTING")) {
        std::cerr << "Failed to initialize DUT memory." << std::endl;
        // }
        if (tfp != NULL) {
            tfp->close();
        }
        return nullptr;
    }
    if (debug_plusarg) {
        std::cout << "DBG init: mem[0..3] DUT="
                  << std::hex
                  << GET_DUT_MEMORY(top->rootp, 0) << " "
                  << GET_DUT_MEMORY(top->rootp, 1) << " "
                  << GET_DUT_MEMORY(top->rootp, 2) << " "
                  << GET_DUT_MEMORY(top->rootp, 3)
                  << " REF="
                  << GET_REF_MEMORY(top->rootp, 0) << " "
                  << GET_REF_MEMORY(top->rootp, 1) << " "
                  << GET_REF_MEMORY(top->rootp, 2) << " "
                  << GET_REF_MEMORY(top->rootp, 3)
                  << std::dec << std::endl;
        if (!instructions.empty()) {
            std::cout << "DBG init: instr[0]=" << std::hex << instructions[0] << std::dec << std::endl;
        }
    }
    // Optional prefault/poke of DUT memory to reduce minor page faults during hot loop.
    if (getenv("VERILATOR_PREFAULT")) {
        const char *step_str = getenv("VERILATOR_PREFAULT_STEP");
        int step = 1;
        if (step_str) step = atoi(step_str);
        if (step <= 0) step = 1;
        if (!getenv("DISABLE_PRINTING")) {
            std::cerr << "VERILATOR_PREFAULT: touching DUT memory (step=" << step << ")" << std::endl;
        }
        for (uint32_t pi = 0; pi < (uint32_t)MAX_NUM_INSTRUCTIONS; pi += (uint32_t)step) {
            volatile uint64_t v = GET_DUT_MEMORY(top->rootp, pi);
            (void)v;
        }
        if (!getenv("DISABLE_PRINTING")) {
            std::cerr << "VERILATOR_PREFAULT: done." << std::endl;
        }
    }
    top->rst = 1;
    top->clk = 0;
    int debug_cycles_left = debug_plusarg ? 20 : 0;
    for(int i=0;i<2;++i){
        top->clk ^= 1;
        top->eval();
        #if defined(BOOM) && BOOM != 0
            set_tlb_entries_for_boom(top->rootp);
        #endif
        // Vcorrectness___024root___eval_settle(top->rootp);
        if (tfp != NULL){
            tfp->dump(sim_time);
        }
        ++sim_time;
    }
    top->rst = 0;

    std::chrono::high_resolution_clock::time_point t_before_loop;
    RUsageStats r_before_loop = {0,0,0,0};
    if (diag) {
        t_before_loop = std::chrono::high_resolution_clock::now();
        r_before_loop = get_rusage_stats();
    }

    const int DIAG_SAMPLE_INTERVAL = 1000; // report every N cycles
    vluint64_t last_sample_cycle = sim_time;
    auto last_sample_time = t_before_loop;
    RUsageStats last_sample_r = r_before_loop;

    while (sim_time < MAX_SIM_TIME && !(contextp->gotFinish())){
        #if defined(BOOM) && BOOM != 0
            set_tlb_entries_for_boom(top->rootp);
        #endif
        top->clk ^=1;
        top->eval();

        if (tfp != NULL){
            tfp->dump(sim_time);
        }

        // std::cout << "mtvec " << std::hex << top->rootp->correctness__DOT__sodor_core__DOT__core__DOT__d__DOT__csr__DOT__reg_mtvec << std::dec << std::endl;
        // if (debug_cycles_left > 0) {
        //     QData fetch = top->rootp->correctness__DOT__kronos__DOT__dut__DOT__cpu__DOT__cpu__DOT__fetch;
        //     uint32_t fetch_ir = static_cast<uint32_t>(fetch & 0xFFFFFFFFULL);
        //     uint32_t fetch_pc = static_cast<uint32_t>((fetch >> 32) & 0xFFFFFFFFULL);
        //     std::cout << "DBG cycle=" << sim_time
        //               << " rst=" << static_cast<int>(top->rst)
        //               << " clk=" << static_cast<int>(top->clk)
        //               << " instret=" << static_cast<int>(top->rootp->correctness__DOT__kronos__DOT__dut__DOT__cpu__DOT__cpu__DOT__u_ex__DOT__instret)
        //               << " ret_pc=0x" << std::hex << top->rootp->correctness__DOT__kronos__DOT__dut__DOT__cpu__DOT__cpu__DOT__u_ex__DOT__ret_pc
        //               << " fetch_pc=0x" << fetch_pc
        //               << " fetch_ir=0x" << fetch_ir << std::dec
        //               << std::endl;
        //     --debug_cycles_left;
        // }
        sim_time++;
        if (diag && (sim_time - last_sample_cycle >= (vluint64_t)DIAG_SAMPLE_INTERVAL)) {
            auto now_s = std::chrono::high_resolution_clock::now();
            RUsageStats r_now = get_rusage_stats();
            std::chrono::duration<double> sample_elapsed = now_s - last_sample_time;
            std::cerr << "VERILATOR_DIAG_SAMPLE: cycle=" << sim_time
                      << " delta_cycles=" << (sim_time - last_sample_cycle)
                      << " elapsed=" << sample_elapsed.count()
                      << "s minflt_delta=" << (r_now.minflt - last_sample_r.minflt)
                      << " majflt_delta=" << (r_now.majflt - last_sample_r.majflt)
                      << " nvcsw_delta=" << (r_now.nvcsw - last_sample_r.nvcsw)
                      << " nivcsw_delta=" << (r_now.nivcsw - last_sample_r.nivcsw)
                      << std::endl;
            last_sample_cycle = sim_time;
            last_sample_time = now_s;
            last_sample_r = r_now;
        }
        if (!(top->correct) && !keep_going){ 
        //    std::cout << "Mismatch detected at time " << sim_time << std::endl;
            break;
        } else {
            // std::cout << "At cycle " << sim_time << ", correct signal is: " << (int)(top->correct) << std::endl;
        }
        if (top->done){
            break;
        }
    }
    if (diag) {
        auto t_after_loop = std::chrono::high_resolution_clock::now();
        RUsageStats r_after_loop = get_rusage_stats();
        std::chrono::duration<double> loop_elapsed = t_after_loop - t_before_loop;
        std::cerr << "VERILATOR_DIAG: simulation loop time=" << loop_elapsed.count()
                  << "s minflt_delta=" << (r_after_loop.minflt - r_before_loop.minflt)
                  << " majflt_delta=" << (r_after_loop.majflt - r_before_loop.majflt)
                  << " nvcsw_delta=" << (r_after_loop.nvcsw - r_before_loop.nvcsw)
                  << " nivcsw_delta=" << (r_after_loop.nivcsw - r_before_loop.nivcsw)
                  << std::endl;
    }
    result->execution_finished =1;
    result->correct = top->correct;
    // printf("Final correct signal: %d\n", top->correct);
    result->mismatch_index = top->mismatch_index;
    result->mismatch_instruction_idx = 0; // Not implemented yet
    result->mismatch_cycle_dut = top->mismatch_cycle_dut_core;
    result->mismatch_cycle_ref = top->mismatch_cycle_ref_core;
    if (!(no_stdout)) {
        const svScope scope = svGetScopeFromName("TOP.correctness.correctness_inst");
        if (!scope) {
            if (!getenv("DISABLE_PRINTING")) {
                std::cerr << "Warning: DPI scope TOP.correctness.correctness_inst not found; skipping commit log." << std::endl;
            }
        } else {
            svSetScope(scope);
            top->printCommitLog(); //DPI function defined in the testbench
            top->printMismatchInfo(); //DPI function defined in the testbench
            if (!getenv("DISABLE_PRINTING")) {
                std::cout << "Simulation finished at time: " << sim_time << std::endl;
            }
        }
    }

    if (tfp != NULL) {
        tfp->close();
    }

    if (extract_constants) {
        uint64_t *constants = get_constants(top->rootp);
        result->constants = constants;
    }
    //top goes out of scope automatically
    //delete top; 
    return result;
}

int main(int argc, char** argv, char** envp){
    auto start_time = std::chrono::high_resolution_clock::now();
    void *result = run_simulation(argc, argv, envp);
    auto end_time = std::chrono::high_resolution_clock::now();
    std::chrono::duration<double> elapsed = end_time - start_time;
    std::cerr << "Total simulation time: " << elapsed.count() << " seconds" << std::endl;
    free_result_struct((struct ExecutionResult*) result);
    return 0;
}
