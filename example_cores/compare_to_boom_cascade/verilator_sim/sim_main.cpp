#include <stdlib.h>
#include <iostream>
#include <sstream>
#include <string>
#include <verilated.h>
#include <iomanip>
#include <fstream>
#include <vector>
#include <optional>
#include <cstdio>
#include <verilated_vcd_c.h>
#include "../obj_dir/Vcorrectness.h"
#include "../obj_dir/Vcorrectness___024root.h"
#include "Vcorrectness__Dpi.h"

using namespace std;

#define MAX_SIM_TIME 300

#define BOOM_OFFSET 0x0
//#define SODOR_OFFSET 16
#define SODOR_OFFSET 0x0

vluint64_t sim_time = 0;

/*
void set_hex_value(std::string hexStr, uint64_t *mem_value){
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
*/

void set_hex_value(std::string hexStr, Vcorrectness___024root *dut, u_int32_t instruction_offset){
    //Sets the hex value in dut for both boom and sodor, and automatically handles that instructino_offset will
    //be increased in 32-bit increments, but memory will be increased in 64-bit increments
    
    uint64_t value;
    std::stringstream ss;
    if (hexStr.length() < 8) {
        hexStr.insert(0, 8 - hexStr.length(), '0');
    }
    ss << std::hex << hexStr;   
    ss >> value;
    std::cout << "Setting sodor mem[" << instruction_offset + SODOR_OFFSET << "] to " << hexStr << std::endl;
    int mem_offset = instruction_offset / 2; //(0,1) maps to 0, (2,3) maps to 1, (4,5) maps to 2 and so on
    int in_mem_offset = instruction_offset % 2; //0 for the first instruction, zero for the second
    int boom_mem_offset = (mem_offset + BOOM_OFFSET); //0x10000 is the start of the boom memory
    int sodor_mem_offset = (mem_offset + SODOR_OFFSET); //0x80 is the start of the sodor memory
    std::cout << "Setting boom mem[" << boom_mem_offset << "] to " << hexStr << std::endl; 
    if (in_mem_offset == 0) {
        uint64_t old_value = dut->correctness__DOT__boom__DOT__dut__DOT__TileLink_Memory__DOT__mem[boom_mem_offset];
        dut->correctness__DOT__boom__DOT__dut__DOT__TileLink_Memory__DOT__mem[boom_mem_offset] = (old_value & 0xFFFFFFFF00000000) | value; 
        old_value = dut->correctness__DOT__sodor_core__DOT__memory__DOT__mem_ext__DOT__Memory[sodor_mem_offset];
        dut->correctness__DOT__sodor_core__DOT__memory__DOT__mem_ext__DOT__Memory[sodor_mem_offset] = (old_value & 0xFFFFFFFF00000000) | value;
    } else {
        uint64_t old_value = dut->correctness__DOT__boom__DOT__dut__DOT__TileLink_Memory__DOT__mem[boom_mem_offset];
        dut->correctness__DOT__boom__DOT__dut__DOT__TileLink_Memory__DOT__mem[boom_mem_offset] = (old_value & 0x00000000FFFFFFFF) | (value << 32);
        old_value = dut->correctness__DOT__sodor_core__DOT__memory__DOT__mem_ext__DOT__Memory[sodor_mem_offset];
        dut->correctness__DOT__sodor_core__DOT__memory__DOT__mem_ext__DOT__Memory[sodor_mem_offset] = (old_value & 0x00000000FFFFFFFF) | (value << 32);
    }
    

    //std::cout << "32-bit value at " << std::hex << mem_value << ": " << std::hex << *mem_value << std::endl;
}

void initialize_dut (Vcorrectness___024root *dut, vector<uint32_t> instructions){
    int start_init_at = 0;
    int num_instr = instructions.size();
    char initStrings[num_instr][9];
    
    for(int i = 0; i < num_instr; i++){
        snprintf(initStrings[i], 9, "%08x", instructions[i]);
    }
    std::cout << "Num instructions " << num_instr << std::endl;

    for(int i=0; i<32;++i){
        std::string hexStr = "0E800093";
        set_hex_value(hexStr, dut,i);
    }
    std::cout << "Filling memory with instrs" << std::endl; 
    // Initialize memory with instructions
    for(int i = 0; i < num_instr; i++) {
        std::string hexStr = initStrings[i];
        set_hex_value(hexStr, dut, start_init_at+i);
        //set_hex_value(hexStr, dut, start_init_at+i);
        //std::cout << "Setting mem[" << start_init_at +i << "] to " << hexStr << std::endl;
    }
    return;
    // Fill "vector table" with different ADDs
    for(int i = 0; i < start_init_at; i++) {
        std::string hexStr = "08400113";
        set_hex_value(hexStr, dut,i);
        set_hex_value(hexStr, dut,i);
    }

    // Fill everything after the instructions with adds
    for(int i = start_init_at+num_instr; i < start_init_at+32; i++) {
        std::string hexStr = "0E800093";
        set_hex_value(hexStr, dut,i);
        set_hex_value(hexStr, dut,i);
    }

    //Fill everything after with jumps
    for(int i=start_init_at+32;i<start_init_at+64; ++i){
        std::string hexStr = "ffdff06f";
        set_hex_value(hexStr, dut,i);
        set_hex_value(hexStr, dut,i);
    }
}

vector<uint32_t> parse_instructions(char* filename){
    // Open file as binary
    ifstream file(filename, ios::binary);

    if (!file.is_open()){
        cout << "Could not open file " << filename << endl;
        exit(1);
    }

    vector<uint32_t> instructions;

    uint32_t instruction;
    while(file.read(reinterpret_cast<char*>(&instruction), sizeof(uint32_t))){
        instructions.push_back(instruction);
    }

    file.close();
    return instructions;
}

int main(int argc, char** argv, char** envp){
    // Pass command line arguments to verilator    
    // Verilated::commandArgs(argc, argv);
    if (argc < 2){
        std::cout<<"Usage: ./Vcorrectness <filename>"<<std::endl;
        return 1;
    }

    vector<uint32_t> instructions = parse_instructions(argv[1]);

    // Instantiate the DUT
    const std::unique_ptr<VerilatedContext> contextp{new VerilatedContext};
    Vcorrectness* top = new Vcorrectness;
    Verilated::traceEverOn(true);

    VerilatedVcdC* tfp = new VerilatedVcdC;
    top->trace(tfp, 99);


    // Generate name for waveform file from argv[1]
    // Generate name for waveform file from argv[1]
    std::optional<std::string> waveform_file;
    if (argc > 2){
        waveform_file = std::optional<std::string>(argv[2]);
    } else {
        waveform_file = std::nullopt;
        //waveform_file = argv[1];
        //waveform_file = waveform_file.substr(0, waveform_file.find_last_of(".")) + ".vcd";
    }
    if (waveform_file.has_value()){
        std::cout << "Writing waveform to " << waveform_file.value() << std::endl;
        tfp->open(waveform_file.value().c_str());
    }
    

    initialize_dut(top->rootp, instructions);
    top->rst = 1;
    top->clk = 0;
    // top->eval(); 
    // if (tfp != NULL){
    //     tfp->dump(sim_time);
    // }
    // ++sim_time;
    for(int i=0;i<2;++i){
        top->clk ^= 1;
        top->eval();
        if (tfp != NULL){
            tfp->dump(sim_time);
        }
        ++sim_time;
    }
    //top->clk ^= 1;
    //top->eval();
    top->rst = 0;

    while (sim_time < MAX_SIM_TIME && !(Verilated::gotFinish())){
        top->clk ^=1;
        top->eval();
        
        if (tfp != NULL){
            tfp->dump(sim_time);
        }
        sim_time++;
        if (!(top->correct)) {
            std::cout << "Leaving because of incorrectness" << std::endl;
            break;
        }
        if (Verilated::gotFinish()){
            std::cout << "Leaving because of gotFinish" << std::endl;
            break;
        }
    }
    
    const svScope scope = svGetScopeFromName("TOP.correctness");
    assert(scope);  // Check for nullptr if scope not found
    svSetScope(scope);
    top->printCommitLog(); //DPI function defined in the testbench
    
    tfp->close();

    std::cout << "Simulation finished at time: " << sim_time << std::endl;
    delete top;
    return 0;
}
