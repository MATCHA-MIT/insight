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
# OTHER_RTL_FILES_BASE defined in the rule below
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

# Build configurations: DIR, VERILOG_SRC, LIB_NAME
BUILD_NORMAL_DIR = obj_dir
BUILD_NORMAL_VERILOG = verilog_externalmem_original
BUILD_NORMAL_LIB = libcorrectness.so

BUILD_REFERENCE_DIR = obj_dir_reference
BUILD_REFERENCE_VERILOG = verilog_bug_free_boom
BUILD_REFERENCE_LIB = libcorrectness_reference.so

BUILD_NO_B4_DIR = obj_dir_no_b4
BUILD_NO_B4_VERILOG = verilog_boom_no_b4_bug
BUILD_NO_B4_LIB = libcorrectness_no_b4.so

BUILD_BUG2_DIR = obj_dir_bug2
BUILD_BUG2_VERILOG = verilog_externalmem_SmallBoomV3CascadeBug2Config
BUILD_BUG2_LIB = libcorrectness_bug2.so

BUILD_BUG3_DIR = obj_dir_bug3
BUILD_BUG3_VERILOG = verilog_externalmem_SmallBoomV3CascadeBug3Config
BUILD_BUG3_LIB = libcorrectness_bug3.so

BUILD_BUG4_DIR = obj_dir_bug4
BUILD_BUG4_VERILOG = verilog_externalmem_SmallBoomV3CascadeBug4Config
BUILD_BUG4_LIB = libcorrectness_bug4.so

BUILD_BUG5_DIR = obj_dir_bug5
BUILD_BUG5_VERILOG = verilog_externalmem_SmallBoomV3CascadeBug5Config
BUILD_BUG5_LIB = libcorrectness_bug5.so

BUILD_BUGRENAME_DIR = obj_dir_bugrename
BUILD_BUGRENAME_VERILOG = verilog_externalmem_SmallBoomV3CascadeBugRenameConfig
BUILD_BUGRENAME_LIB = libcorrectness_bugrename.so

BUILD_NOBUG_DIR = obj_dir_nobug
BUILD_NOBUG_VERILOG = verilog_externalmem_SmallBoomV3CascadeNoBugConfig
BUILD_NOBUG_LIB = libcorrectness_nobug.so

BUILD_ALLBUGS_DIR = obj_dir_allbugs
BUILD_ALLBUGS_VERILOG = verilog_externalmem_SmallBoomV3CascadeAllBugsConfig
BUILD_ALLBUGS_LIB = libcorrectness_allbugs.so

# List all build targets
BUILD_TARGETS = $(BUILD_BUG2_DIR)/$(BUILD_BUG2_LIB) $(BUILD_BUG3_DIR)/$(BUILD_BUG3_LIB) $(BUILD_BUG4_DIR)/$(BUILD_BUG4_LIB) $(BUILD_BUG5_DIR)/$(BUILD_BUG5_LIB) $(BUILD_BUGRENAME_DIR)/$(BUILD_BUGRENAME_LIB) $(BUILD_NOBUG_DIR)/$(BUILD_NOBUG_LIB) $(BUILD_ALLBUGS_DIR)/$(BUILD_ALLBUGS_LIB)
#$(BUILD_REFERENCE_DIR)/$(BUILD_REFERENCE_LIB) $(BUILD_NO_B4_DIR)/$(BUILD_NO_B4_LIB)

all: $(BUILD_TARGETS)

# Function to get verilog source for a directory
get_verilog = $(if $(filter $(BUILD_NORMAL_DIR),$1),$(BUILD_NORMAL_VERILOG),$(if $(filter $(BUILD_REFERENCE_DIR),$1),$(BUILD_REFERENCE_VERILOG),$(if $(filter $(BUILD_NO_B4_DIR),$1),$(BUILD_NO_B4_VERILOG),$(if $(filter $(BUILD_BUG2_DIR),$1),$(BUILD_BUG2_VERILOG),$(if $(filter $(BUILD_BUG3_DIR),$1),$(BUILD_BUG3_VERILOG),$(if $(filter $(BUILD_BUG4_DIR),$1),$(BUILD_BUG4_VERILOG),$(if $(filter $(BUILD_BUG5_DIR),$1),$(BUILD_BUG5_VERILOG),$(if $(filter $(BUILD_BUGRENAME_DIR),$1),$(BUILD_BUGRENAME_VERILOG),$(if $(filter $(BUILD_NOBUG_DIR),$1),$(BUILD_NOBUG_VERILOG),$(BUILD_ALLBUGS_VERILOG))))))))))

# Pattern rule for external_init.cpp
.PRECIOUS: %/external_init.cpp
%/external_init.cpp: generate_external_init.py BOOM.init
	mkdir -p $*
	python3 generate_external_init.py BOOM.init $@

# Pattern rule for verilator build
.PRECIOUS: %/Vcorrectness__ALL.a
%/Vcorrectness__ALL.a: %/external_init.cpp
	# Do not compile SimDRAM.v (it pulls DPI symbols like memory_init/memory_tick).
	# Also drop TestHarness.sv (we build with --top-module correctness).
	# Provide a local stub for SimDRAM so elaboration succeeds without DPI.
	grep -v -E '(SimDRAM\\.v|TestHarness\\.sv)' $(call get_verilog,$*)/filelist.f > $(call get_verilog,$*)/filelist.filtered.f
	sed -i 's|^|$(call get_verilog,$*)/|' $(call get_verilog,$*)/filelist.filtered.f
	$(VERILATOR_PATH) $(VERILATOR_OPTS) \
		-y ../common/testbench -y rtl -y $(call get_verilog,$*) \
		-f $(call get_verilog,$*)/filelist.filtered.f \
		-Irtl sodor_verilog/*.sv rtl/TileLink_Memory.sv correctness.sv \
		--top-module correctness \
		--Mdir $* \
		$*/external_init.cpp \
		--exe ../common/verilator_sim/sim_main.cpp \
		 \
		--build

# Define a template for building shared libraries
define BUILD_LIB_template
$(1)/$(2): $(1)/Vcorrectness__ALL.a
	$$(CXX) -shared -o $$@ \
	    -Wl,--whole-archive $(1)/Vcorrectness__ALL.a $(1)/verilated_threads.o $(1)/verilated_fst_c.o $(1)/verilated.o $(1)/sim_main.o $(1)/verilated_dpi.o $(1)/external_init.o -Wl,--no-whole-archive \
	    -fPIC -lstdc++ -lz 
	python -c 'import ctypes; lib = ctypes.CDLL("$$@", mode=ctypes.RTLD_GLOBAL)'
endef

# Generate rules for each build target
$(eval $(call BUILD_LIB_template,$(BUILD_BUG2_DIR),$(BUILD_BUG2_LIB)))
$(eval $(call BUILD_LIB_template,$(BUILD_BUG3_DIR),$(BUILD_BUG3_LIB)))
$(eval $(call BUILD_LIB_template,$(BUILD_BUG4_DIR),$(BUILD_BUG4_LIB)))
$(eval $(call BUILD_LIB_template,$(BUILD_BUG5_DIR),$(BUILD_BUG5_LIB)))
$(eval $(call BUILD_LIB_template,$(BUILD_BUGRENAME_DIR),$(BUILD_BUGRENAME_LIB)))
$(eval $(call BUILD_LIB_template,$(BUILD_NOBUG_DIR),$(BUILD_NOBUG_LIB)))
$(eval $(call BUILD_LIB_template,$(BUILD_ALLBUGS_DIR),$(BUILD_ALLBUGS_LIB)))

clean:
	rm -rf $(BUILD_BUG2_DIR) $(BUILD_BUG3_DIR) $(BUILD_BUG4_DIR) $(BUILD_BUG5_DIR) $(BUILD_BUGRENAME_DIR) $(BUILD_NOBUG_DIR) $(BUILD_ALLBUGS_DIR)

.PHONY: clean all
