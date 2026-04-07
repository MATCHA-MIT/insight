use std::sync::atomic::{self, AtomicU16, AtomicUsize};

pub const STOP_AT_PERCENT_FULFILLED: i32 = 0;
pub static REQUIRED_BEX_FULFILLED: AtomicU16 = AtomicU16::new(100);
pub const MAX_INSTRUCTION_CYCLE_LENGTH: u64 = 30; //30; //5; //30; //boom core..
pub const MAX_INSTRUCTION_LIFETIME_REFCORE: u64 = 3; //Bump to 2 to account for stall.
pub const NUM_THREADS: usize = 50;
pub const OPCODE_CSR: i64 = 0x73;
pub const OPCODE_RTYPE: i64 = 0x33;
pub const OPCODE_ITYPE_ARITHMETIC: i64 = 0x13;
pub const OPCODE_STYPE: i64 = 0x23;
pub const OPCODE_BTYPE: i64 = 0x63;
pub const MAX_NUM_BEX: usize = 15000;

// New benchmarking controls
pub static ATOMIC_MAX_NUM_BEX: AtomicUsize = AtomicUsize::new(15000);
pub static ATOMIC_MAX_NUM_CEX: AtomicUsize = AtomicUsize::new(usize::MAX);

// Benchmarking outputs
pub static BENCHMARK_NUM_CEX: AtomicUsize = AtomicUsize::new(0);
pub static BENCHMARK_NUM_BEX: AtomicUsize = AtomicUsize::new(0);
pub static BENCHMARK_ILP_STATES: AtomicUsize = AtomicUsize::new(0);
pub static BENCHMARK_NUM_COLLECTED_PREDICATES: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy)]
pub struct PredicateTakeLimits {
    pub control_predicates: usize,
    pub regular_predicates: usize,
    pub not_equal_predicates: usize,
    pub signal_equal_predicates: usize,
}

pub struct PredicateTakeLimitsAtomic {
    control_predicates: AtomicUsize,
    regular_predicates: AtomicUsize,
    not_equal_predicates: AtomicUsize,
    signal_equal_predicates: AtomicUsize,
}

impl PredicateTakeLimitsAtomic {
    pub const fn new(
        control_predicates: usize,
        regular_predicates: usize,
        not_equal_predicates: usize,
        signal_equal_predicates: usize,
    ) -> Self {
        Self {
            control_predicates: AtomicUsize::new(control_predicates),
            regular_predicates: AtomicUsize::new(regular_predicates),
            not_equal_predicates: AtomicUsize::new(not_equal_predicates),
            signal_equal_predicates: AtomicUsize::new(signal_equal_predicates),
        }
    }

    pub fn load(&self, ordering: atomic::Ordering) -> PredicateTakeLimits {
        PredicateTakeLimits {
            control_predicates: self.control_predicates.load(ordering),
            regular_predicates: self.regular_predicates.load(ordering),
            not_equal_predicates: self.not_equal_predicates.load(ordering),
            signal_equal_predicates: self.signal_equal_predicates.load(ordering),
        }
    }

    pub fn store_control_predicates(&self, value: usize, ordering: atomic::Ordering) {
        self.control_predicates.store(value, ordering);
    }

    pub fn store_regular_predicates(&self, value: usize, ordering: atomic::Ordering) {
        self.regular_predicates.store(value, ordering);
    }

    pub fn store_not_equal_predicates(&self, value: usize, ordering: atomic::Ordering) {
        self.not_equal_predicates.store(value, ordering);
    }

    pub fn store_signal_equal_predicates(&self, value: usize, ordering: atomic::Ordering) {
        self.signal_equal_predicates.store(value, ordering);
    }
}

pub static PREDICATE_TAKE_LIMITS: PredicateTakeLimitsAtomic =
    PredicateTakeLimitsAtomic::new(50, 50, 30, 50);
//We don't use this anymore pub const TAKE_NUM_PREDICATES_FROM_TEACHER: usize = 1200;
pub const MAX_GINI_IMPURITY: f64 = 0.5;
pub const FORCE_BEX_CYCLES_TO_NON_COVERED: bool = true;
pub const PREDICATES_GENERATE_BEX_PREDICATES_ONLY_FROM_CEX_CYCLES: bool = true;
pub const SOLVE_STATES_INSTEAD_OF_TRACES: bool = false;
pub const HEURISTIC_MERGE_STATES_OPTIMIZATION: bool = false;

pub static POTENTIAL_MATCH_SIGNALS: &[&str] = &[
    "rs",
    "rd",
    "opcode",
    "wb",
    "funct3",
    "funct7",
    "hz",
    "hazard",
    "hazr",
    "imm_i",
    "immediate",
];

pub const MAX_CONTROL_SIGNAL_LENGTH: usize = 7;
pub const INCORRECTNESS_SIGNAL: &'static str = "TOP.correctness.correct";
pub const COUNTER_SIGNAL: &'static str = "TOP.correctness.correctness_inst.counter";
pub const MISMATCH_CYCLE_REF_CORE_SIGNAL: &'static str = "TOP.correctness.mismatch_cycle_ref_core";
pub const MISMATCH_CYCLE_DUT_CORE_SIGNAL: &'static str = "TOP.correctness.mismatch_cycle_dut_core";
pub const DUT_STALL_SIGNAL: &str = "TOP.correctness.correctness_inst.dut_has_stalled";
pub const REFCORE_STALL_SIGNAL: &str = "TOP.correctness.correctness_inst.ref_has_stalled";

pub fn min_num_bex_fulfilled(total_num_bex: usize) -> usize {
    let bex_threshold = (total_num_bex as f64
        * (REQUIRED_BEX_FULFILLED.load(atomic::Ordering::Relaxed) as f64 / 100.0) as f64)
        .round() as i32;
    bex_threshold as usize
}
pub fn min_num_cex_fulfilled(total_num_cex: usize) -> usize {
    let cex_threshold =
        (total_num_cex as f64 * (STOP_AT_PERCENT_FULFILLED as f64 / 100.0) as f64).round() as i32;
    cex_threshold as usize
}

pub fn get_cex_and_bex_score_threshold(current_best_cex_and_bex_score: i64) -> i64 {
    (current_best_cex_and_bex_score * 20) / 100
}

pub fn get_cex_fulfilled_threshold(current_best_cex_and_bex_score: i64) -> i64 {
    (current_best_cex_and_bex_score * 110) / 100
}
