use crate::jg_formatter;
use std::collections::HashSet;
use std::sync::Arc;
use rayon::prelude::*;
use crate::data_types::general_data_types::DefaultScalarHasher;

#[derive(Debug, Clone)]
pub struct JGFilterInfo {
    jg_signals: HashSet<Arc<str>>,
    jg_signals_transformed: HashSet<Arc<str>>,
}

impl JGFilterInfo {
    pub fn new(jg_signals: HashSet<Arc<str>>) -> Self {
        let jg_signals_transformed: HashSet<Arc<str>> = jg_signals
            .par_iter()
            .map(|s| jg_formatter::transform_jg_to_verilator_like(s))
            .map(Arc::from)
            .collect();
        JGFilterInfo {
            jg_signals,
            jg_signals_transformed,
        }
    }
    pub fn len(&self) -> usize {
        self.jg_signals.len()
    }
}

#[derive(Debug, Clone)]
pub enum SignalFilter {
    RegexFilter(Vec<regex::Regex>),
    StrictSignalIdxFilter(HashSet<u64, DefaultScalarHasher>),
    LooseSignalIdxFilter(HashSet<u64, DefaultScalarHasher>),
    JGFilter(JGFilterInfo),
}

impl SignalFilter {
    pub fn check_signal(&self, signal_name: &str, signal_idx: &u64) -> bool {
        match self {
            SignalFilter::RegexFilter(regexes) => {
                for regex in regexes {
                    if regex.is_match(signal_name) {
                        return true;
                    }
                }
                false
            }
            SignalFilter::StrictSignalIdxFilter(idx_set) => idx_set.contains(signal_idx),
            SignalFilter::LooseSignalIdxFilter(idx_set) => idx_set.contains(signal_idx),
            SignalFilter::JGFilter(jg_signals) => {
                // println!("Checking signal {} against JG signals {:?}", signal_name, jg_signals.len());
                if jg_formatter::matches_any_jg_signal(&Arc::from(signal_name), &jg_signals.jg_signals, Some(&jg_signals.jg_signals_transformed)).is_some() {
                    return true;
                } else {
                    // println!("Signal {} not found in JG signals", signal_name);
                    // println!("Signal {} not found in JG signals {:?}", signal_name, jg_signals);
                    return false;
                }
            }
        }
    }
    pub fn estimate_size(&self) -> usize {
        match self {
            SignalFilter::RegexFilter(regexes) => regexes.len(),
            SignalFilter::StrictSignalIdxFilter(idx_set) => idx_set.len(),
            SignalFilter::LooseSignalIdxFilter(idx_set) => idx_set.len(),
            SignalFilter::JGFilter(jg_signals) => jg_signals.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SignalFilters {
    pub filters: Vec<SignalFilter>,
}

impl SignalFilters {
    pub fn new() -> Self {
        SignalFilters {
            filters: Vec::new(),
        }
    }

    pub fn add_filter(&mut self, filter: SignalFilter) {
        self.filters.push(filter);
    }

    pub fn check_signal_any_filter(&self, signal_name: &str, signal_idx: &u64) -> bool {
        //Does signal match any filter?
        for filter in &self.filters {
            if filter.check_signal(signal_name, signal_idx) {
                return true;
            }
        }
        false
    }

    pub fn check_signal_all_filters(&self, signal_name: &str, signal_idx: &u64) -> bool {
        //Does signal match all filters?
        for filter in &self.filters {
            if !filter.check_signal(signal_name, signal_idx) {
                return false;
            }
        }
        true
    }

    pub fn estimate_max_length_matching_signals(&self) -> usize {
        //Estimate the maximum length of matching signals
        //Which is the smallest lengths of all filters
        let mut max_length = 0;
        for filter in &self.filters {
            max_length = std::cmp::min(max_length, filter.estimate_size());
        }
        max_length
    }
}
