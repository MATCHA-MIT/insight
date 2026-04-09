include build_config/config.cfg


ifndef MEMSIZE
$(error MEMSIZE is not defined. Please define MEMSIZE in the configuration.)
endif
ifndef MAX_SIM_TIME
$(error MAX_SIM_TIME is not defined. Please define MAX_SIM_TIME in the configuration.)
endif

ifndef START_INDEX_MEMORY
$(error START_INDEX_MEMORY is not defined. Please define START_INDEX_MEMORY in the configuration.)
endif

# .PHONY: print-default-instruction
# print-default-instruction:
# 	@echo DEFAULT_INSTRUCTION=$(DEFAULT_INSTRUCTION)

# Define all Kronos versions to build
VERSIONS = baseline all_fix no_k1_fix no_k2_fix no_k3_fix
KRONOS_DIRS = kronos_designs/kronos_baseline kronos_designs/kronos_all_fix kronos_designs/kronos_no_k1_fix kronos_designs/kronos_no_k2_fix kronos_designs/kronos_no_k3_fix

# c++ compilation defines (parsed from build_config/config.cfg)
DEFINES=-DCASCADE=1 -DDEFAULT_INSTRUCTION=$(DEFAULT_INSTRUCTION) -DDEBUG=0 -DPROVIDE_EXTERNAL_INIT=1 -DMEMSIZE=$(MEMSIZE) -DMAX_SIM_TIME=$(MAX_SIM_TIME) -DSTART_INDEX_MEMORY=$(START_INDEX_MEMORY)
# verilator compiler options
VERILATOR_OPTS_BASE=-cc -CFLAGS "$(DEFINES) -DVL_USER_FINISH=1 -fPIC -std=c++17" +define+MTVEC_RESET=$(MTVEC_RESET_ADDR) +define+TIMER_ADDR=$(TIMER_ADDR) +define+MEMSIZE=$(MEMSIZE) -DPRINTF_COND=0 -DSTOP_COND=0 -DASSERT_VERBOSE_COND=0 --no-assert --x-initial 0 --compiler clang --trace-fst --trace-max-array 1024 --trace-structs -j 0  -Wno-WIDTHEXPAND -Wno-PINMISSING
.EXPORT_ALL_VARIABLES:

CXX = g++
CC = gcc

# Add -trace-conditions only if USE_STANDARD_VERILATOR is NOT set
ifeq ($(USE_STANDARD_VERILATOR),)
VERILATOR_OPTS = $(VERILATOR_OPTS_BASE) -trace-conditions
else
VERILATOR_OPTS = $(VERILATOR_OPTS_BASE)
endif

ifeq ($(USE_STANDARD_VERILATOR),)
VERILATOR_PATH=$(realpath ../../tools/verilator/bin/verilator)
VERILATOR_ALREADY_BUILT=$(wildcard ../../tools/verilator/build_done)
ifeq ($(VERILATOR_ALREADY_BUILT),)
$(error Verilator not built. Please run cd ../../tools/verilator/ && ./build.sh)
endif
else
VERILATOR_PATH=verilator
endif

# Build all versions
all: $(foreach v,$(VERSIONS),build/$(v)/libcorrectness.so)

# Individual targets for convenience
baseline: build/baseline/libcorrectness.so
all_fix: build/all_fix/libcorrectness.so
no_k1_fix: build/no_k1_fix/libcorrectness.so
no_k2_fix: build/no_k2_fix/libcorrectness.so
no_k3_fix: build/no_k3_fix/libcorrectness.so

# Explicit rules for each version

# baseline
build/baseline/external_init.cpp: scripts/generate_external_init.py build_config/KRONOS.init
	mkdir -p build/baseline
	python3 scripts/generate_external_init.py build_config/KRONOS.init $@

build/baseline/verilate.done: build/baseline/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) kronos_designs/kronos_baseline/rtl/core/kronos_types.sv -y kronos_designs/kronos_baseline/rtl/core -Irtl sodor_verilog/*.sv correctness.sv --top-module correctness \
		build/baseline/external_init.cpp --Mdir build/baseline --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/baseline/libcorrectness.so: build/baseline/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/baseline/Vcorrectness__ALL.a build/baseline/verilated_threads.o build/baseline/verilated_fst_c.o build/baseline/verilated.o build/baseline/sim_main.o build/baseline/verilated_dpi.o build/baseline/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz 
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# all_fix
build/all_fix/external_init.cpp: scripts/generate_external_init.py build_config/KRONOS.init
	mkdir -p build/all_fix
	python3 scripts/generate_external_init.py build_config/KRONOS.init $@

build/all_fix/verilate.done: build/all_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) kronos_designs/kronos_all_fix/rtl/core/kronos_types.sv -y kronos_designs/kronos_all_fix/rtl/core -Irtl sodor_verilog/*.sv correctness.sv --top-module correctness \
		build/all_fix/external_init.cpp --Mdir build/all_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/all_fix/libcorrectness.so: build/all_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/all_fix/Vcorrectness__ALL.a build/all_fix/verilated_threads.o build/all_fix/verilated_fst_c.o build/all_fix/verilated.o build/all_fix/sim_main.o build/all_fix/verilated_dpi.o build/all_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz 
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# no_k1_fix
build/no_k1_fix/external_init.cpp: scripts/generate_external_init.py build_config/KRONOS.init
	mkdir -p build/no_k1_fix
	python3 scripts/generate_external_init.py build_config/KRONOS.init $@

build/no_k1_fix/verilate.done: build/no_k1_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) kronos_designs/kronos_no_k1_fix/rtl/core/kronos_types.sv -y kronos_designs/kronos_no_k1_fix/rtl/core -Irtl sodor_verilog/*.sv correctness.sv --top-module correctness \
		build/no_k1_fix/external_init.cpp --Mdir build/no_k1_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/no_k1_fix/libcorrectness.so: build/no_k1_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/no_k1_fix/Vcorrectness__ALL.a build/no_k1_fix/verilated_threads.o build/no_k1_fix/verilated_fst_c.o build/no_k1_fix/verilated.o build/no_k1_fix/sim_main.o build/no_k1_fix/verilated_dpi.o build/no_k1_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz 
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# no_k2_fix
build/no_k2_fix/external_init.cpp: scripts/generate_external_init.py build_config/KRONOS.init
	mkdir -p build/no_k2_fix
	python3 scripts/generate_external_init.py build_config/KRONOS.init $@

build/no_k2_fix/verilate.done: build/no_k2_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) kronos_designs/kronos_no_k2_fix/rtl/core/kronos_types.sv -y kronos_designs/kronos_no_k2_fix/rtl/core -Irtl sodor_verilog/*.sv correctness.sv --top-module correctness \
		build/no_k2_fix/external_init.cpp --Mdir build/no_k2_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/no_k2_fix/libcorrectness.so: build/no_k2_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/no_k2_fix/Vcorrectness__ALL.a build/no_k2_fix/verilated_threads.o build/no_k2_fix/verilated_fst_c.o build/no_k2_fix/verilated.o build/no_k2_fix/sim_main.o build/no_k2_fix/verilated_dpi.o build/no_k2_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz 
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# no_k3_fix
build/no_k3_fix/external_init.cpp: scripts/generate_external_init.py build_config/KRONOS.init
	mkdir -p build/no_k3_fix
	python3 scripts/generate_external_init.py build_config/KRONOS.init $@

build/no_k3_fix/verilate.done: build/no_k3_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) kronos_designs/kronos_no_k3_fix/rtl/core/kronos_types.sv -y kronos_designs/kronos_no_k3_fix/rtl/core -Irtl sodor_verilog/*.sv correctness.sv --top-module correctness \
		build/no_k3_fix/external_init.cpp --Mdir build/no_k3_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/no_k3_fix/libcorrectness.so: build/no_k3_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/no_k3_fix/Vcorrectness__ALL.a build/no_k3_fix/verilated_threads.o build/no_k3_fix/verilated_fst_c.o build/no_k3_fix/verilated.o build/no_k3_fix/sim_main.o build/no_k3_fix/verilated_dpi.o build/no_k3_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz 
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'



clean:
	rm -rf build

.PHONY: clean all baseline all_fix no_k1_fix no_k2_fix no_k3_fix
