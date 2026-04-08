#all:
#	verilator --version
#	verilator  -DSTOP_COND=0 -DASSERT_VERBOSE_COND=0 --compiler clang -CFLAGS -std=c++17 --no-stop-fail --x-initial 0 --x-assign 0 -j 0 -Wno-WIDTHEXPAND --no-assert --cc $(VERILATOR_OPTS) -y ../common/testbench -y rtl -y verilog_externalmem_original -f build_config/filelist.f -y sodor_verilog correctness.v --top-module correctness --exe verilator_sim/sim_main.cpp --build

   
include build_config/config.cfg

ifndef MEMSIZE
$(error MEMSIZE is not defined. Please define MEMSIZE in the configuration.)
endif

ifndef MAX_SIM_TIME
$(error MAX_SIM_TIME is not defined. Please define MAX_SIM_TIME in the configuration.)
endif

ifndef ADDRESS_WIDTH
$(error define ADDRESS_WIDTH!)
endif

ifndef DUT_CORE_MEM_STRING
$(error define MEM_STRING)
endif 

ifndef START_INDEX_MEMORY
$(error define START_INDEX_MEMORY)
endif  

RTL_files=rtl/*.sv #$(SODOR_FILES)
OTHER_RTL_FILES_BASE= -y ../common/testbench -y rtl -f build_config/filelist.f -Irtl sodor_verilog/*.sv
#PUT CPP DEFINES HERE -- Otherwise verilator will nto pass them correctly to the cpp testbench!
#-DCASCADE=1
DEFINES=-DBOOM=1 -DPROVIDE_EXTERNAL_INIT=1 -DCASCADE=1 -DDEBUG=0 -DMEMSIZE=$(MEMSIZE) -DMAX_SIM_TIME=$(MAX_SIM_TIME) -DADDRESS_WIDTH=$(ADDRESS_WIDTH) -DDUT_CORE_MEM_STRING=$(DUT_CORE_MEM_STRING) -DSTART_INDEX_MEMORY=$(START_INDEX_MEMORY) -DVL_USER_FINISH=1 
VERILATOR_OPTS=-cc -CFLAGS "$(DEFINES) -DVL_USER_FINISH=1 -fPIC -std=c++17" +define+MEMSIZE=$(MEMSIZE) -DPRINTF_COND=0 -DSTOP_COND=0 -DASSERT_VERBOSE_COND=0 --no-assert --x-initial 0 --compiler clang --trace-fst --trace-max-array 2048 --trace-structs -j 0  --x-assign 0 --trace-structs --trace-underscore --trace  --build -j 0 --no-assert --no-stop-fail 
DISABLED_WARNING=-Wno-PINMISSING -Wno-LATCH -Wno-WIDTHEXPAND -Wno-TIMESCALEMOD -Wno-UNOPTFLAT
#efine+SYNTHESIS=1 
#-Wno-Widthexpand
#--Wno-PINMISSING
VERILATOR_OPTS += $(DISABLED_WARNING)



# Add -trace-conditions only if USE_STANDARD_VERILATOR is NOT set
ifeq ($(USE_STANDARD_VERILATOR),)
VERILATOR_OPTS += -trace-conditions
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

.EXPORT_ALL_VARIABLES:

CXX = g++
CC = gcc

# BOOM variants (Kronos-style multi-variant build)
BOOM_VERSIONS = boom_baseline boom_all_fix boom_no_b1_fix boom_no_b2_fix boom_no_b3_fix boom_no_b4_fix boom_no_b5_fix

all: $(foreach v,$(BOOM_VERSIONS),build/$(v)/libcorrectness.so)

# -----------------------------------------------------------------------
# BOOM variants – Kronos-style targets (build/<variant>/libcorrectness.so)
# -----------------------------------------------------------------------
all_boom_variants: $(foreach v,$(BOOM_VERSIONS),build/$(v)/libcorrectness.so)

# Individual convenience targets
boom_baseline:   build/boom_baseline/libcorrectness.so
boom_all_fix:    build/boom_all_fix/libcorrectness.so
boom_no_b1_fix:  build/boom_no_b1_fix/libcorrectness.so
boom_no_b2_fix:  build/boom_no_b2_fix/libcorrectness.so
boom_no_b3_fix:  build/boom_no_b3_fix/libcorrectness.so
boom_no_b4_fix:  build/boom_no_b4_fix/libcorrectness.so
boom_no_b5_fix:  build/boom_no_b5_fix/libcorrectness.so

# boom_baseline
build/boom_baseline/external_init.cpp: scripts/generate_external_init.py build_config/BOOM.init
	mkdir -p build/boom_baseline
	python3 scripts/generate_external_init.py build_config/BOOM.init $@

build/boom_baseline/verilate.done: build/boom_baseline/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) $(OTHER_RTL_FILES_BASE) -y boom_designs/boom_baseline correctness.sv \
		--top-module correctness \
		build/boom_baseline/external_init.cpp --Mdir build/boom_baseline --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/boom_baseline/libcorrectness.so: build/boom_baseline/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/boom_baseline/Vcorrectness__ALL.a build/boom_baseline/verilated_threads.o build/boom_baseline/verilated_fst_c.o build/boom_baseline/verilated.o build/boom_baseline/sim_main.o build/boom_baseline/verilated_dpi.o build/boom_baseline/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# boom_all_fix
build/boom_all_fix/external_init.cpp: scripts/generate_external_init.py build_config/BOOM.init
	mkdir -p build/boom_all_fix
	python3 scripts/generate_external_init.py build_config/BOOM.init $@

build/boom_all_fix/verilate.done: build/boom_all_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) $(OTHER_RTL_FILES_BASE) -y boom_designs/boom_all_fix correctness.sv \
		--top-module correctness \
		build/boom_all_fix/external_init.cpp --Mdir build/boom_all_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/boom_all_fix/libcorrectness.so: build/boom_all_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/boom_all_fix/Vcorrectness__ALL.a build/boom_all_fix/verilated_threads.o build/boom_all_fix/verilated_fst_c.o build/boom_all_fix/verilated.o build/boom_all_fix/sim_main.o build/boom_all_fix/verilated_dpi.o build/boom_all_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# boom_no_b1_fix
build/boom_no_b1_fix/external_init.cpp: scripts/generate_external_init.py build_config/BOOM.init
	mkdir -p build/boom_no_b1_fix
	python3 scripts/generate_external_init.py build_config/BOOM.init $@

build/boom_no_b1_fix/verilate.done: build/boom_no_b1_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) $(OTHER_RTL_FILES_BASE) -y boom_designs/boom_no_b1_fix correctness.sv \
		--top-module correctness \
		build/boom_no_b1_fix/external_init.cpp --Mdir build/boom_no_b1_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/boom_no_b1_fix/libcorrectness.so: build/boom_no_b1_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/boom_no_b1_fix/Vcorrectness__ALL.a build/boom_no_b1_fix/verilated_threads.o build/boom_no_b1_fix/verilated_fst_c.o build/boom_no_b1_fix/verilated.o build/boom_no_b1_fix/sim_main.o build/boom_no_b1_fix/verilated_dpi.o build/boom_no_b1_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# boom_no_b2_fix
build/boom_no_b2_fix/external_init.cpp: scripts/generate_external_init.py build_config/BOOM.init
	mkdir -p build/boom_no_b2_fix
	python3 scripts/generate_external_init.py build_config/BOOM.init $@

build/boom_no_b2_fix/verilate.done: build/boom_no_b2_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) $(OTHER_RTL_FILES_BASE) -y boom_designs/boom_no_b2_fix correctness.sv \
		--top-module correctness \
		build/boom_no_b2_fix/external_init.cpp --Mdir build/boom_no_b2_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/boom_no_b2_fix/libcorrectness.so: build/boom_no_b2_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/boom_no_b2_fix/Vcorrectness__ALL.a build/boom_no_b2_fix/verilated_threads.o build/boom_no_b2_fix/verilated_fst_c.o build/boom_no_b2_fix/verilated.o build/boom_no_b2_fix/sim_main.o build/boom_no_b2_fix/verilated_dpi.o build/boom_no_b2_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# boom_no_b3_fix
build/boom_no_b3_fix/external_init.cpp: scripts/generate_external_init.py build_config/BOOM.init
	mkdir -p build/boom_no_b3_fix
	python3 scripts/generate_external_init.py build_config/BOOM.init $@

build/boom_no_b3_fix/verilate.done: build/boom_no_b3_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) $(OTHER_RTL_FILES_BASE) -y boom_designs/boom_no_b3_fix correctness.sv \
		--top-module correctness \
		build/boom_no_b3_fix/external_init.cpp --Mdir build/boom_no_b3_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/boom_no_b3_fix/libcorrectness.so: build/boom_no_b3_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/boom_no_b3_fix/Vcorrectness__ALL.a build/boom_no_b3_fix/verilated_threads.o build/boom_no_b3_fix/verilated_fst_c.o build/boom_no_b3_fix/verilated.o build/boom_no_b3_fix/sim_main.o build/boom_no_b3_fix/verilated_dpi.o build/boom_no_b3_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# boom_no_b4_fix
build/boom_no_b4_fix/external_init.cpp: scripts/generate_external_init.py build_config/BOOM.init
	mkdir -p build/boom_no_b4_fix
	python3 scripts/generate_external_init.py build_config/BOOM.init $@

build/boom_no_b4_fix/verilate.done: build/boom_no_b4_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) $(OTHER_RTL_FILES_BASE) -y boom_designs/boom_no_b4_fix correctness.sv \
		--top-module correctness \
		build/boom_no_b4_fix/external_init.cpp --Mdir build/boom_no_b4_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/boom_no_b4_fix/libcorrectness.so: build/boom_no_b4_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/boom_no_b4_fix/Vcorrectness__ALL.a build/boom_no_b4_fix/verilated_threads.o build/boom_no_b4_fix/verilated_fst_c.o build/boom_no_b4_fix/verilated.o build/boom_no_b4_fix/sim_main.o build/boom_no_b4_fix/verilated_dpi.o build/boom_no_b4_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

# boom_no_b5_fix
build/boom_no_b5_fix/external_init.cpp: scripts/generate_external_init.py build_config/BOOM.init
	mkdir -p build/boom_no_b5_fix
	python3 scripts/generate_external_init.py build_config/BOOM.init $@

build/boom_no_b5_fix/verilate.done: build/boom_no_b5_fix/external_init.cpp
	$(VERILATOR_PATH) $(VERILATOR_OPTS) $(OTHER_RTL_FILES_BASE) -y boom_designs/boom_no_b5_fix correctness.sv \
		--top-module correctness \
		build/boom_no_b5_fix/external_init.cpp --Mdir build/boom_no_b5_fix --exe ../common/verilator_sim/sim_main.cpp --build
	touch $@

build/boom_no_b5_fix/libcorrectness.so: build/boom_no_b5_fix/verilate.done
	$(CXX) -shared -o $@ \
	    -Wl,--whole-archive build/boom_no_b5_fix/Vcorrectness__ALL.a build/boom_no_b5_fix/verilated_threads.o build/boom_no_b5_fix/verilated_fst_c.o build/boom_no_b5_fix/verilated.o build/boom_no_b5_fix/sim_main.o build/boom_no_b5_fix/verilated_dpi.o build/boom_no_b5_fix/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz
	python -c 'import ctypes; lib = ctypes.CDLL("$@", mode=ctypes.RTLD_GLOBAL)'

clean:
	rm -rf build

.PHONY: clean all all_boom_variants \
        boom_baseline boom_all_fix \
        boom_no_b1_fix boom_no_b2_fix boom_no_b3_fix boom_no_b4_fix boom_no_b5_fix

