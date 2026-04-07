/// Configuration module for cycle-related types
/// 
/// This module allows configuring the integer type used for cycle counts
/// throughout the waveform processing system.

/// Type alias for cycle count. Change this to u64 if you need more cycles.
/// 
/// u32 supports up to 4,294,967,295 cycles which should be sufficient for most use cases.
/// If you need more cycles, change this to u64.
pub type CycleCount = u32;

/// Trait to allow generic conversion between different cycle count types
pub trait CycleCountConversion {
    fn to_cycle_count(self) -> CycleCount;
    fn from_cycle_count(cycle: CycleCount) -> Self;
}

impl CycleCountConversion for u64 {
    fn to_cycle_count(self) -> CycleCount {
        self.try_into().expect("Cycle count too large for CycleCount type")
    }
    
    fn from_cycle_count(cycle: CycleCount) -> Self {
        cycle.into()
    }
}

impl CycleCountConversion for u32 {
    fn to_cycle_count(self) -> CycleCount {
        self
    }
    
    fn from_cycle_count(cycle: CycleCount) -> Self {
        cycle
    }
}

impl CycleCountConversion for usize {
    fn to_cycle_count(self) -> CycleCount {
        self.try_into().expect("Cycle count too large for CycleCount type")
    }
    
    fn from_cycle_count(cycle: CycleCount) -> Self {
        cycle.try_into().expect("CycleCount too large for usize")
    }
}
