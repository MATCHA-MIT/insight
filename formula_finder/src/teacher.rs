//Struct that represents waveforms as "examples" for the separator inference
//The difference is, that for each waveform, we only store a set of relevant cycles:
//These are those cycles, that contain "information for our" problem
//The separator inference will then try to find a separator that separates the relevant states of the cex and bex waveforms
//For each cex, we know that at least one of the cycle is a "CEX state", but other cycles might not be
//For bex, all cycles are "bex states"
//How do we get this mapping? For each cex, for each cycle, we check whether 
//that state (restricted to the relevant signals) can be countered in any BEX state.
//If so, remove if. The struct are dervied from this algorithm
use std::cmp::Ordering;
use std::sync::atomic;
use std::sync::atomic::AtomicUsize;
use std::sync::RwLock;
use std::sync::Weak;
use indicatif::ParallelProgressIterator;
use indicatif::ProgressBar;
use std::sync::Arc;
use crate::allowed_signal_config;
use crate::allowed_signal_config::PredicateType;
use crate::data_types::general_data_types::BitSetWrapper;
use crate::data_types::general_data_types::DefaultScalarHasher;
use crate::data_types::general_data_types::DefaultVectorHasher; 
use crate::data_types::general_data_types::FuzzerDataPoint;
use crate::data_types::signal_filters;
use crate::predicates::PriorityScores;
use crate::data_types::general_data_types::SignalInfo;
use crate::data_types::general_data_types::SignalIndexSet;
use crate::constants as constants_module;
use crate::utils;
use crate::waveform;
use crate::predicates;
use std::collections::HashSet;
use std::collections::HashMap;
use rayon::prelude::*;
use itertools::Itertools;
use crate::data_types;
use indicatif::ProgressIterator;
use crate::cycle_types::{CycleCount, CycleCountConversion};
use dashmap::{DashMap, DashSet};
use rand::Rng;
use log;
 use regex::Regex;


#[derive(Debug, Clone)]
pub struct Teacher {
    pub cex_traces: Vec<Arc<CexTrace>>, //List of list of states, where state is _maybe_ a "positive example"
    pub bex_traces: Vec<Arc<BexTrace>>, //List of list of states, where state is a "negative example"
    pub states: HashMap<u64, Arc<StateMapping>>, //Map from sample_id to Sample
    pub bex_samples: Vec<Arc<BexSample>>, //List of states, where state is a "negative example"
    pub values_per_signal: Option<HashMap<u64, HashSet<i64>>>,
    pub name_to_id: Arc<HashMap<ustr::Ustr, u64>>,
    pub id_to_signal_info: Arc<HashMap<u64, SignalInfo>>,
}

#[derive(Debug, Clone, Default)]
pub struct CexTrace {
    pub contained_samples: Vec<ContainedState>, //sample_id
    pub from_path: ustr::Ustr,
    pub file_source: Option<data_types::general_data_types::WaveFormSource>,
    pub mismatch_cycle_dut_core: Option<u64>,
    pub mismatch_cycle_ref_core: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct BexTrace {
    pub contained_samples: Vec<ContainedState>, //sample_id
    pub from_path: ustr::Ustr,
    pub file_source: Option<data_types::general_data_types::WaveFormSource>,
    pub weight: f64
}

#[derive(Debug, Clone, Default)]
pub struct BexSample {
    pub from_path: ustr::Ustr,
    pub and_cycle: u64,
    pub file_source: Option<data_types::general_data_types::WaveFormSource>,
    pub occurrence_count: u64, //Number of times this bex state was seen in bex waveforms,
    pub state_id: u64, //This is the state id in the bex waveforms,
    pub state_pointer: Weak<StateMapping>,
    pub paths_and_cycles: Option<Vec<(ustr::Ustr, u64)>>, //This is a list of paths and cycles where this sample was seen
}

impl BexSample {
    pub fn must_not_cover(&self) -> bool {
        self.file_source == Some(data_types::general_data_types::WaveFormSource::MustFulfill)
    }
}

#[derive(Debug, Clone, Default)]
pub struct StateMapping {
    pub string_to_id: Arc<HashMap<ustr::Ustr, u64>>,
    pub signal_values: Vec<i64>,
    pub signal_id_to_index: Arc<HashMap<u64, usize, DefaultScalarHasher>>,
    pub state_id: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ContainedState {
    pub state_id: u64, 
    pub and_cycle: u64,
    pub state_pointer: Weak<StateMapping>,
}

pub fn collect_potential_values(cex_traces: &Vec<Arc<CexTrace>>, bex_samples: &Vec<Arc<BexSample>>) -> HashMap<u64, HashSet<i64>> {
    let mut values_per_signal: HashMap<u64, HashSet<i64>> = HashMap::new();
    let mut bex_value_per_signal: HashMap<u64, HashSet<i64>> = HashMap::new();
    for cex in cex_traces.iter() {
        for contained_state in cex.contained_samples.iter() {
            let state = contained_state.state_pointer.upgrade().unwrap();
            for (signal_idx, &idx) in state.signal_id_to_index.iter() {
                let value = state.signal_values[idx];
                let set = values_per_signal.entry(*signal_idx).or_insert(HashSet::new());
                set.insert(value);
            }
        }
    }
    for sample in bex_samples.iter() {
        let state = sample.state_pointer.upgrade().unwrap();
        if sample.file_source != Some(data_types::general_data_types::WaveFormSource::Mutations) {
            continue;
        }
        for (signal_idx, &idx) in state.signal_id_to_index.iter() {
            let value = state.signal_values[idx];
            let set = values_per_signal.entry(*signal_idx).or_insert(HashSet::new());
            set.insert(value);
            let set: &mut HashSet<i64> = bex_value_per_signal.entry(*signal_idx).or_insert(HashSet::new());
            set.insert(value);
        }
    }
    values_per_signal

    //for (signal_name, values) in potential_values.iter() {
    //    println!("Signal {} has potential values {:?}", signal_name, values);
    //}
}

pub fn fill_in_indexes_for_predicate(predicate: &mut predicates::BasePredicate, name_to_id: &HashMap<ustr::Ustr, u64>) -> Result<(), ()> {
    match predicate.base_formula {
        predicates::BaseFormula::TwoSignalEqual(ref mut inner_formula) =>  {
            let signal1_idx = name_to_id.get(&ustr::ustr(&inner_formula.signal_name1));
            let signal2_idx = name_to_id.get(&ustr::ustr(&inner_formula.signal_name2));
            if let Some(index) = signal1_idx {
                inner_formula.signal_idx1 = Some(index.clone().into());
            } else {
                return Err(());
            }
            if let Some(index) = signal2_idx {
                inner_formula.signal_idx2 = Some(index.clone().into());
            } else {
                return Err(());
            }
            Ok(())

        },
        predicates::BaseFormula::SignalToConst(ref mut inner_formula) => {
            if let Some(index) = name_to_id.get(&ustr::ustr(&inner_formula.signal_name)) {
                inner_formula.signal_idx = Some(index.clone().into());
                Ok(())
            } else {
                Err(())
            }
        },
        predicates::BaseFormula::ValueNotIn(ref mut inner_formula)
        | predicates::BaseFormula::ValueIn(ref mut inner_formula) => {
            if let Some(index) = name_to_id.get(&ustr::ustr(&inner_formula.signal_name)) {
                inner_formula.signal_idx = Some(index.clone().into());
                Ok(())
            } else {
                Err(())
            }
        },
    }
}

pub fn fill_in_indexes_for_formula_and_waveform(invariant: &mut predicates::SeparatorFormula, waveform: &waveform::WaveForm) -> Result<(), ()> {
    let name_to_id: HashMap<ustr::Ustr, u64> = waveform.name_to_code.iter()
    .map(|(key, value)| (key.clone(), (*value).into()))
    .collect();
    let res = fill_in_indexes_for_formula(invariant, &name_to_id);
    if res.is_err() {
        panic!("Waveform {} could not fill in indexes error message {:?}", waveform.path, res.err());
    }
    // for predicate in invariant.predicate_set.predicates.iter_mut() {
    //     if fill_in_indexes_for_predicate(predicate, &name_to_id).is_err() {
    //         // println!("Could not find signal {:?} in name to id {:?}", predicate.get_signal_names(), name_to_id);
    //         return Err(());
    //     }
    // }
    Ok(())
}


pub fn fill_in_indexes_for_formula(formula: &mut predicates::SeparatorFormula, name_to_id: &HashMap<ustr::Ustr, u64>) -> Result<(), String> {
    match formula {
        &mut predicates::SeparatorFormula::Invariant(ref mut invariant) => {
            if fill_in_indexes_for_invariant(invariant, &name_to_id).is_err() {
                return Err(format!("Could not fill in indexes for invariant {}", invariant));
            }
        },
        &mut predicates::SeparatorFormula::InvariantDisjunction(ref mut disjunction) => {
            for invariant in disjunction.disjunctions.iter_mut() {
                if fill_in_indexes_for_invariant(invariant, &name_to_id).is_err() {
                    return Err(format!("Could not fill in indexes for invariant {} in disjunction", invariant));
                }
            }
        },

    }
    Ok(())
}

pub fn fill_in_indexes_for_invariant(invariant: &mut predicates::Invariant, name_to_id: &HashMap<ustr::Ustr, u64>) -> Result<(), String> {
    for predicate in invariant.predicate_set.predicates.iter_mut() {
        if fill_in_indexes_for_predicate(predicate, name_to_id).is_err() {
            return Err(format!("Could not find signal {:?} in name to id {:?}", predicate.get_signal_names(), name_to_id));
        }
    }
    Ok(())
}

impl Teacher {

    pub fn get_total_num_bex(&self) -> usize {
        // 
        // bex_samples_sum
        if constants_module::SOLVE_STATES_INSTEAD_OF_TRACES {
            let bex_samples_sum = self.bex_samples.iter().map(|x| x.occurrence_count as usize).sum::<usize>();
            bex_samples_sum
        } else {
            self.bex_traces.len()
        }
    }

    pub fn get_total_bex_weight(&self) -> f64 {
        if constants_module::SOLVE_STATES_INSTEAD_OF_TRACES {
            let bex_weight_sum = self.bex_samples.iter().map(|x| (x.occurrence_count as f64)).sum::<f64>();
            bex_weight_sum
        } else {
            let bex_weight_sum = self.bex_traces.iter().map(|x| x.weight).sum::<f64>();
            bex_weight_sum
        }
    }



    pub fn break_down_covered_states(&self, covered_states: &BitSetWrapper) -> (BitSetWrapper, BitSetWrapper) {
        let mut cex_states = BitSetWrapper::new();
        let mut allowed_bex_states = BitSetWrapper::new();
        for cex_trace in self.cex_traces.iter() {
            for contained_state in cex_trace.contained_samples.iter() {
                let state_id = contained_state.state_id;
                if covered_states.contains(&(state_id as u32)) {
                    cex_states.add(state_id as u32);
                }
            }
        }
        for bex_sample in self.bex_samples.iter() {
            let state_id = bex_sample.state_id;
            if !(covered_states.contains(&(state_id as u32))) {
                allowed_bex_states.add(state_id as u32);
            }
        }
        (cex_states, allowed_bex_states)
    }

    pub fn set_new_original_cex(&mut self, already_covered_cex_traces: &HashSet<u64>){
        let mut pick_from = Vec::new();
        let mut old_original_cex_id = None;
        for (id, cex_trace) in self.cex_traces.iter().enumerate() {
            if already_covered_cex_traces.contains(&(id as u64)) {
                if cex_trace.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                    old_original_cex_id = Some(id);
                }
                continue;
            }
            if cex_trace.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                panic!("Original cex was not already covered?");
            }
            pick_from.push(id);
        }
        if pick_from.len() == 0 {
            panic!("No more cex to pick from");
        }
        let mut rng = rand::rng();
        let pick_id = pick_from[rng.random_range(0..pick_from.len())];
        //Make_mut is clone on write
        let new_cex_trace = Arc::make_mut(&mut self.cex_traces[pick_id]);
        new_cex_trace.file_source = Some(data_types::general_data_types::WaveFormSource::OriginalCex);
        self.cex_traces[pick_id] = Arc::new(new_cex_trace.clone());
        let modified_old_original_cex = Arc::make_mut(&mut self.cex_traces[old_original_cex_id.unwrap()]);
        modified_old_original_cex.file_source = Some(data_types::general_data_types::WaveFormSource::Mutations);
        self.cex_traces[old_original_cex_id.unwrap()] = Arc::new(modified_old_original_cex.clone());
        if old_original_cex_id == Some(pick_id) {
            panic!("Old and new original cex are the same in set_new_original_cex?");
        }
        self.cex_traces.swap(0, pick_id);
    }
    pub fn get_covered_cex_traces_from_covered_states(&self, covered_states: &BitSetWrapper) -> HashSet<u64> {
        let mut covered_cex_ids: HashSet<u64> = HashSet::new();
        for (id, cex_trace) in self.cex_traces.iter().enumerate() {
            if cex_trace.is_covered_by_blocked_states(covered_states) {
                covered_cex_ids.insert(id as u64);
            }
        }
        covered_cex_ids
    }

    pub fn get_score_from_covered_states(&self, covered_states: &BitSetWrapper) -> PriorityScores {
        let mut final_score = PriorityScores::default();
        let mut num_fulfilled_cex = 0;
        let mut num_allowed_bex = 0;
        let mut num_total_bex = 0;
        for cex_trace in self.cex_traces.iter() {
            if cex_trace.is_covered_by_blocked_states(covered_states) {
                num_fulfilled_cex += 1;
            } else if cex_trace.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                final_score.cex_only_score = predicates::ScoreResult::Unsat;
                final_score.cex_and_bex_score = predicates::ScoreResult::Unsat;
                final_score.cex_with_bex_allow_score = predicates::ScoreResult::Unsat;
                return final_score;
            }
        }
        for bex_trace in self.bex_traces.iter() {
            let is_covered = bex_trace.is_covered_by_blocked_states(covered_states);
            if !is_covered {
                num_allowed_bex += 1;
            }
            num_total_bex += 1;
        }
        // for bex_sample in self.bex_samples.iter() {
        //     let bex_sample_id = bex_sample.state_id;

        //     if covered_states.contains(&(bex_sample_id as u32)) == false {
        //         num_allowed_bex += bex_sample.occurrence_count as usize;
        //     } else {
        //         if bex_sample.must_not_cover() {
        //             final_score.cex_only_score = predicates::ScoreResult::Sat(num_fulfilled_cex as i64);
        //             final_score.cex_and_bex_score = predicates::ScoreResult::Unsat;
        //             final_score.cex_with_bex_allow_score = predicates::ScoreResult::Unsat;
        //             return final_score;
        //         }
        //     }
        //     num_total_bex += bex_sample.occurrence_count as usize;
        // }
        final_score.cex_only_score = predicates::ScoreResult::Sat(num_fulfilled_cex as i64);
        final_score.cex_and_bex_score = predicates::ScoreResult::Sat(num_fulfilled_cex as i64 + num_allowed_bex as i64);
        let bex_threshold = (num_total_bex as f64 * (constants_module::REQUIRED_BEX_FULFILLED.load(atomic::Ordering::Relaxed) as f64 / 100.0) as f64).round() as i32;
        //println!("Bex threshold {:?} allowed bex len {:?} num bex {:?}", bex_threshold, allowed_bex.len(), self.bex_samples.len());
        if num_allowed_bex >= (bex_threshold as usize) {
            final_score.cex_with_bex_allow_score = predicates::ScoreResult::Sat(num_fulfilled_cex as i64);
        } else {
            final_score.cex_with_bex_allow_score = predicates::ScoreResult::Unsat;
        }
        //println!("Final score {:?}", final_score);
        final_score
    }


    pub fn get_original_cex_sample_id(&self) -> usize {
        for (id, cex) in self.cex_traces.iter().enumerate() {
            if cex.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                return id;
            }
        }
        panic!("No originalcex present?");
    }

    pub fn collect_predicates(
        &self,
        signal_list: Option<&Vec<Arc<str>>>,
        for_core: &data_types::general_data_types::CoreType,
        generation_config: &allowed_signal_config::PredicateGenerationConfig,
    ) -> Vec<predicates::BasePredicateCandidate> {
        let mut predicates: DashSet<predicates::BasePredicateCandidate> = DashSet::new();
        // let signal_names = match signal_list {
        //     Some(list) => list.iter().map(|s| s.as_ref()).collect::<Vec<&str>>(),
        //     None => self.name_to_id.keys().map(|s| s.as_ref()).collect::<Vec<&str>>(),
        // };
        // let sodor_count = signal_names.iter().filter(|s| s.to_lowercase().contains("sodor")).count();
        // let boom_count = signal_names.iter().filter(|s| s.to_lowercase().contains("boom")).count();
        // let kronos_count = signal_names.iter().filter(|s| s.to_lowercase().contains("kronos")).count();
        // let generation_config = if sodor_count > boom_count && sodor_count > kronos_count {
        //     println!("Using Sodor predicate generation config");
        //     allowed_signal_config::get_sodor_predicate_generation_config()
        // } else if boom_count > sodor_count && boom_count > kronos_count {
        //     println!("Using Boom predicate generation config");
        //     allowed_signal_config::get_boom_predicate_generation_config()
        // } else {
        //     println!("Using default predicate generation config");
        //     allowed_signal_config::get_default_predicate_generation_config()
        // };
        let cex_values_per_signal_and_sample: DashMap<(usize, usize), HashMap<u64, HashSet<i64>>> = DashMap::new();
        let cex_signal_per_value_and_sample: DashMap<(usize, usize), HashMap<i64, BitSetWrapper>> = DashMap::new();
        let bex_signal_per_value_and_sample: DashMap<usize, HashMap<i64, BitSetWrapper>> = DashMap::new();
        let seen_bex_values_per_signal: DashMap<u64, HashSet<i64>> = DashMap::new();
        let already_seen_bex: DashMap<u64, HashSet<i64>> = DashMap::new();
        
        // NEW: Track cycle information for each (signal_idx, value) pair
        // let cex_cycle_per_signal_value: DashMap<(u64, i64), HashSet<usize>> = DashMap::new();

        let mut cex_min_cycle = 0;
        let mut cex_max_cycle = u64::MAX;

        let this_signal_idx_list: HashSet<u64, data_types::general_data_types::DefaultScalarHasher> = match signal_list {
            Some(list) => list
                .iter()
                .map(|x| self.name_to_id[&ustr::existing_ustr(x.as_ref()).unwrap()])
                .collect(),
            None => self.name_to_id.values().cloned().collect(),
        };

        let id_to_info = &self.id_to_signal_info;

        // ---- CEX processing (unchanged, still builds bitsets) ----
            // ---- CEX traces ----
        log::info!("Collecting per-signal values from CEX traces...");
        self.cex_traces.iter().progress().for_each(|cex_trace| {
            if cex_trace.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                let (start_from, until) = cex_trace
                    .get_relevant_cycles(for_core)
                    .unwrap_or_else(|| {
                        let min = cex_trace.contained_samples.iter().map(|x| x.and_cycle).min().unwrap();
                        let max = cex_trace.contained_samples.iter().map(|x| x.and_cycle).max().unwrap();
                        (min, max)
                    });
                cex_min_cycle = start_from;
                cex_max_cycle = until;
                log::debug!("Collecting predicates from CEX trace {:?} from cycle {} to {}", cex_trace.from_path, start_from, until);
                for sample in &cex_trace.contained_samples {
                    if sample.and_cycle < start_from || sample.and_cycle > until {
                        continue;
                    }

                    let mut cex_values_per_signal: HashMap<u64, HashSet<i64>> = HashMap::new();
                    let mut cex_signal_per_value: HashMap<i64, BitSetWrapper> = HashMap::new();

                    let this_state = sample.state_pointer.upgrade().unwrap();
                    for signal_idx in &this_signal_idx_list {
                        let signal_info = &id_to_info[signal_idx];
                        let signal_types = &signal_info.signal_types;
                        // if signal_info.any_alias_contains("dded__Vcond_tern_verilog_externalmem_original_RocketALU_sv_52", false) {
                        //     println!("Debugging signal info {:?} Has value {:?}", signal_info, this_state.get_signal_value_from_index(signal_idx));

                        // }
                        // For CEX, we might generate Equal/NotEqual predicates later
                        // Apply filters for those predicate types
                        let pred_types_to_check = vec![PredicateType::Equal, PredicateType::NotEqual, PredicateType::GreaterEqual, PredicateType::SmallerEqual, PredicateType::TwoSignalEqual];
                        let mut should_include = false;
                        for pred_type in &pred_types_to_check {
                            if generation_config.allows_predicate_type_set(signal_types, pred_type) {
                                should_include = true;
                                break;
                            }
                        }
                        if !should_include {
                            // if signal_info.any_alias_contains("dded__Vcond_tern_verilog_externalmem_original_RocketALU_sv_52", false) {
                            //     println!("Debugging signal info {:?} signal not allowed {:?}", signal_info, this_state.get_signal_value_from_index(signal_idx));
                            // }
                            continue;
                        }
                        let value = this_state.get_signal_value_from_index(signal_idx);
                        

                        cex_values_per_signal.entry(*signal_idx).or_default().insert(value);
                        // cex_cycle_per_signal_value
                        // .entry((*signal_idx, value))
                        // .or_insert_with(HashSet::new)
                        // .insert(sample.and_cycle as usize);

                        if generation_config.allows_predicate_type_set(signal_types, &PredicateType::TwoSignalEqual) {
                            cex_signal_per_value
                                .entry(value)
                                .or_insert_with(BitSetWrapper::new)
                                .add(*signal_idx as u32);
                        }
                    }
                    cex_values_per_signal_and_sample.insert((sample.state_id as usize, sample.and_cycle as usize), cex_values_per_signal);
                    cex_signal_per_value_and_sample.insert((sample.state_id as usize, sample.and_cycle as usize), cex_signal_per_value);
                }
            }
        });

        // pre-build an index for faster lookups:
        let cex_values_index: HashMap<u64, HashSet<i64>> = {
            let mut index = HashMap::new();
            for entry in cex_values_per_signal_and_sample.iter() {
                for (sig_idx, values) in entry.value() {
                    index.entry(*sig_idx)
                        .or_insert_with(HashSet::new)
                        .extend(values);
                }
            }
            index
        };

        let progress_bar = ProgressBar::new(self.bex_samples.len() as u64);
        // ---- BEX samples ----
        log::info!("Collecting per-signal predicates from BEX states...");
        self.bex_samples.iter().filter(|sample| sample.file_source == Some(data_types::general_data_types::WaveFormSource::Mutations)).par_bridge().for_each(|sample| {
            if sample.file_source != Some(data_types::general_data_types::WaveFormSource::Mutations) {
                return;
            }
            if constants_module::PREDICATES_GENERATE_BEX_PREDICATES_ONLY_FROM_CEX_CYCLES && (sample.and_cycle < cex_min_cycle || sample.and_cycle > cex_max_cycle) {
                return;
            }

            let mut bex_signal_per_value: HashMap<i64, BitSetWrapper> = HashMap::new();
            let state = sample.state_pointer.upgrade().unwrap();

            for signal_idx in &this_signal_idx_list {
                let signal_info = &id_to_info[signal_idx];
                let signal_types = &signal_info.signal_types;
                let value = state.get_signal_value_from_index(signal_idx);
                
                // Check if we should generate Equal predicates for this (signal_type, value)
                let mut skip_value = true;
                for potential_predicate in [PredicateType::Equal, PredicateType::NotEqual, PredicateType::GreaterEqual, PredicateType::SmallerEqual].iter() {
                    if generation_config.allows_predicate_type_set(signal_types, potential_predicate) {
                        skip_value = false;
                        break;
                    }
                }
                if !skip_value {
                    seen_bex_values_per_signal
                        .entry(*signal_idx)
                        .or_insert_with(HashSet::new)
                        .insert(value);
                    if !(generation_config.allows_value_type_set(signal_types, &PredicateType::NotEqual, *signal_idx, value, signal_info)) {
                        continue;
                    }
                    let mut entry = already_seen_bex.entry(*signal_idx).or_default();
                    if entry.insert(value) {
                        let res_cex = cex_values_index
                            .get(signal_idx)
                            .map_or(false, |set| set.contains(&value));
                        if !res_cex {
                            let predicate = predicates::BasePredicate::new_from_info_and_value(
                                signal_info,
                                predicates::Operator::NotEqual,
                                value,
                            );
                            predicates.insert(predicates::BasePredicateCandidate {
                                base_predicate: predicate,
                                only_in_cycles: None,
                            });
                        }
                    }
                }
                if generation_config.allows_predicate_type_set(signal_types, &PredicateType::TwoSignalEqual) {
                    bex_signal_per_value
                        .entry(value)
                        .or_insert_with(BitSetWrapper::new)
                        .add(*signal_idx as u32);
                }
            }
            bex_signal_per_value_and_sample.insert(sample.state_id as usize, bex_signal_per_value);
            progress_bar.inc(1);
        });
        progress_bar.finish_and_clear();
        let progress_bar = ProgressBar::new(cex_values_per_signal_and_sample.len() as u64);
        log::info!("Collecting per-signal predicates from CEX values...");
        for cex_values_per_signal in cex_values_per_signal_and_sample.iter() {
            for signal_idx in &this_signal_idx_list {
                let signal_info = &id_to_info[signal_idx];

                if let Some(cex_set) = cex_values_per_signal.get(signal_idx) {
                    for val in cex_set {
                        let only_in_cycles = None;
                        // cex_cycle_per_signal_value
                        //     .get(&(*signal_idx, *val))
                        //     .map(|cycles| {
                        //         let mut list: Vec<usize> = cycles.iter().copied().collect();
                        //         list.sort_unstable();
                        //         list.dedup();
                        //         list
                        //     })
                        //     .filter(|list| !list.is_empty());
                        // let seen_more_than_one_bex_value  = seen_bex_values_per_signal
                        //     .get(signal_idx)
                        //     .map_or(false, |bex_set| bex_set.len() > 1);
                        // let seen_different_bex_value = match seen_more_than_one_bex_value {
                        //     true => true,
                        //     false => {
                        //         seen_bex_values_per_signal
                        //             .get(signal_idx)
                        //             .map_or(false, |bex_set| bex_set.iter().any(|bex_val| *bex_val != *val))
                        //     }
                        // };
                        if generation_config.allows_value_type_set(&signal_info.signal_types, &PredicateType::Equal, *signal_idx, *val, signal_info) {
                            let predicate = predicates::BasePredicate::new_from_info_and_value(
                                signal_info,
                                predicates::Operator::Equal,
                                *val,
                            );
                            predicates.insert(predicates::BasePredicateCandidate {
                                base_predicate: predicate,
                                only_in_cycles: only_in_cycles.clone(),
                            });
                        } else {
                            // if signal_info.any_alias_contains("dded__Vcond_tern_verilog_externalmem_original_RocketALU_sv_52", false) {
                            //     println!("Debugging signal info {:?} Equal predicate not allowed for value {:?}", signal_info, val);
                            // }
                        }

                        if generation_config.allows_value_type_set(&signal_info.signal_types, &PredicateType::GreaterEqual, *signal_idx, *val, signal_info) {
                            let ge = predicates::BasePredicate::new_from_info_and_value(
                                signal_info,
                                predicates::Operator::GreaterEqual,
                                *val,
                            );
                            println!("Adding GE predicate {:?} for signal idx {} alias {:?} value {}", ge, signal_idx, signal_info.aliases, *val);
                            predicates.insert(predicates::BasePredicateCandidate {
                                base_predicate: ge,
                                only_in_cycles: only_in_cycles.clone(),
                            });
                        } else if signal_info.signal_types.contains(&data_types::general_data_types::SignalType::Control) {
                            // println!("Skipping GE predicate for control signal idx {} alias {:?} value {}", signal_idx, signal_info.aliases, *val);
                        }
                        if generation_config.allows_value_type_set(&signal_info.signal_types, &PredicateType::SmallerEqual, *signal_idx, *val, signal_info) {
                            let le = predicates::BasePredicate::new_from_info_and_value(
                                signal_info,
                                predicates::Operator::SmallerEqual, 
                                *val,
                            );
                            println!("Adding LE predicate {:?} for signal idx {} alias {:?} value {}", le, signal_idx, signal_info.aliases, *val);
                            predicates.insert(predicates::BasePredicateCandidate {
                                base_predicate: le,
                                only_in_cycles: only_in_cycles.clone(),
                            });
                        }
                    }
                }
            }
            progress_bar.inc(1);
        }
        let progress_bar = ProgressBar::new(cex_signal_per_value_and_sample.len() as u64);
        // ---- Cross-signal equal predicates ----
        log::info!("Collecting signal equal predicates...");
        let all_signal_equal_predicates: HashSet<_> = cex_signal_per_value_and_sample
            .par_iter()
            .flat_map_iter(|entry| {
                let ((_sample_id, cycle_num), cex_signal_per_value) = entry.pair();
                let mut local_preds = HashSet::new();
                for (val, matching_signals) in cex_signal_per_value {
                    for comb in matching_signals.clone().collect().iter().combinations(2) {
                        let (s1, s2) = (*comb[0], *comb[1]);
        
                        let left_info = &id_to_info[&(s1 as u64)];
                        let right_info = &id_to_info[&(s2 as u64)];
                        if left_info.length != right_info.length {
                            continue;
                        }
                        if !(generation_config.allows_predicate_type_set(&left_info.signal_types, &PredicateType::TwoSignalEqual)){
                            continue;
                        }
                        if !(generation_config.allows_predicate_type_set(&right_info.signal_types, &PredicateType::TwoSignalEqual)){
                            continue;
                        }
                        if !(generation_config.allows_value_type_set(&left_info.signal_types, &PredicateType::TwoSignalEqual, s1 as u64, *val, left_info)) {
                            continue;
                        }
                        if !(generation_config.allows_value_type_set(&right_info.signal_types, &PredicateType::TwoSignalEqual, s2 as u64, *val, right_info)) {
                            continue;
                        }
                        if left_info.signal_types.types.intersection(&right_info.signal_types.types).count() == 0 {
                            continue;
                        }
                        let signals_differ_in_same_bex = bex_signal_per_value_and_sample
                            .iter()
                            .any(|bex_entry| {
                                let bex_signal_per_value = bex_entry.value();
                                
                                // For each value, check if exactly one of the two signals has it
                                bex_signal_per_value.iter().any(|(_bex_val, signal_set)| {
                                    let s1_has = signal_set.contains(&(s1 as u32));
                                    let s2_has = signal_set.contains(&(s2 as u32));
                                    // XOR: exactly one has this value
                                    s1_has ^ s2_has
                                })
                            });
                        if signals_differ_in_same_bex {
                            let pred = predicates::BasePredicate::new_from_two_signals(
                                Arc::from(left_info.aliases[0].as_str()),
                                Arc::from(right_info.aliases[0].as_str()),
                                Some(s1 as u64),
                                Some(s2 as u64),
                                predicates::Operator::Equal,
                            );
                            let pred = predicates::BasePredicateCandidate {
                                base_predicate: pred,
                                only_in_cycles: Some(vec![*cycle_num]),
                            };
                            // log::info!("Adding signal equal predicate {:?} for value {} seen in CEX sample {} cycle {} because signals differ in same BEX sample", pred, val, sample_id, cycle_num);
                            // log::info!("Adding signal equal predicate {} left signal info {:?} right signal info {:?}", pred, left_info, right_info);
                            local_preds.insert(pred);
                        }
                        
                    }
                }
                progress_bar.inc(1);
                local_preds
            })
            .collect::<Vec<_>>()
            .into_iter()
            .fold(
                std::collections::HashMap::<predicates::BasePredicate, Vec<usize>>::new(),
                |mut acc, cand| {
                    let bp = cand.base_predicate;
                    let cycles = cand.only_in_cycles;
                    
                    acc.entry(bp).or_insert_with(Vec::new).extend(
                        cycles.unwrap_or_default()
                    );
                    acc
                }
            )
            .into_iter()
            .map(|(base_predicate, mut cycles)| {
                cycles.sort_unstable();
                cycles.dedup();
                predicates::BasePredicateCandidate {
                    base_predicate,
                    only_in_cycles: if cycles.is_empty() { None } else { Some(cycles) },
                }
            })
            .collect::<HashSet<_>>();
	
        predicates.extend(all_signal_equal_predicates);
        log::info!("Collected a total of {} predicates", predicates.len());
        predicates.into_iter().collect()
    }

    pub fn get_signal_info_from_index(&self, signal_idx: &u64) -> Option<&data_types::general_data_types::SignalInfo> {
        self.id_to_signal_info.get(signal_idx)
    }

    pub fn get_signal_type_from_index(&self, signal_idx: &u64) -> Option<data_types::general_data_types::SignalTypesSet> {
        if let Some(signal_info) = self.id_to_signal_info.get(signal_idx) {
            Some(signal_info.signal_types.clone())
        } else {
            None
        }
    }

    pub fn get_signal_list(&self) -> Vec<Arc<str>> {
        let mut signal_list: Vec<Arc<str>> = Vec::new();
        for (name, _) in self.name_to_id.iter() {
            signal_list.push(Arc::from(name.as_str()));
        }
        signal_list
    }

    pub fn filter_unique_signals(&self, signal_list: &Vec<Arc<str>>) -> Vec<Arc<str>> {
        let mut unique_codes = std::collections::HashSet::new();
        let mut result = Vec::new();

        for signal in signal_list.iter() {
            if let Some(code) = self.name_to_id.get(&ustr::existing_ustr(signal.as_ref()).unwrap()) {
                if unique_codes.insert(code) {
                    result.push(signal.clone());
                }
            }
        }
        result
    }

    // pub fn filter_control_signals(&self, signals: &Vec<Arc<str>>) -> Vec<Arc<str>> {
    //     let filtered_signals: Vec<Arc<str>> = signals.iter().filter_map(|signal_name| {
    //         //let signal_name = signal_name_symbol.as_str();
    //         if utils::is_control_signal(signal_name, self.get_signal_length(signal_name)) {
    //             Some(signal_name.clone())
    //         } else {
    //             None
    //         }
    //     }).collect();
    //     filtered_signals
    // }


    pub fn calculate_values_per_signal(&mut self) -> &HashMap<u64, HashSet<i64>> {
        if self.values_per_signal.is_none() {
            let values_per_signal = collect_potential_values(&self.cex_traces, &self.bex_samples);
            self.values_per_signal = Some(values_per_signal);
        }
        self.values_per_signal.as_ref().unwrap()
    }

    pub fn get_values_per_signal(&self, signal_name: &Arc<str>) -> Option<&HashSet<i64>> {
        let signal_idx = self.name_to_id.get(&ustr::existing_ustr(signal_name).unwrap()).unwrap();
        self.values_per_signal.as_ref().unwrap().get(signal_idx)
    }

    pub fn get_signal_idx(&self, signal_name: &Arc<str>) -> u64 {
        let signal_idx = self.name_to_id.get(&ustr::existing_ustr(signal_name).unwrap()).unwrap();
        signal_idx.clone().into()
    }

    pub fn get_maybe_signal_idx(&self, signal_name: &Arc<str>) -> Option<u64> {
        let signal_idx = self.name_to_id.get(&ustr::existing_ustr(signal_name).unwrap());
        if signal_idx.is_none() {
            return None;
        }
        Some(signal_idx.unwrap().clone().into())
    }

    pub fn fill_in_indexes_for_predicate(&self, predicate: &mut predicates::BasePredicate) -> Result<(), ()> {
        fill_in_indexes_for_predicate(predicate, &self.name_to_id)
    }

    pub fn fill_in_indexes_for_formula(&self, formula: &mut predicates::SeparatorFormula) -> Result<(), String> {
        match formula {
            &mut predicates::SeparatorFormula::Invariant(ref mut invariant) => {
                let res = fill_in_indexes_for_invariant(invariant, &self.name_to_id);
                return res;
            },
            &mut predicates::SeparatorFormula::InvariantDisjunction(ref mut disjunction) => {
                for invariant in disjunction.disjunctions.iter_mut() {
                    let res = fill_in_indexes_for_invariant(invariant, &self.name_to_id);
                    match res {
                        Err(e) => {
                            return Err(e);
                        },
                        _ => {},
                    }
                }
            },

        }
        Ok(())
    }

    pub fn get_bex_values_per_signal(&self) -> HashMap<u64, HashSet<i64>> {
        let mut bex_value_per_signal: HashMap<u64, HashSet<i64>> = HashMap::new();
        for sample in self.bex_samples.iter() {
            if sample.file_source != Some(data_types::general_data_types::WaveFormSource::Mutations) {
                continue;
            }
            let state = sample.state_pointer.upgrade().unwrap();
            for (signal_idx, &idx) in state.signal_id_to_index.iter() {
                let value = state.signal_values[idx];
                let set: &mut HashSet<i64> = bex_value_per_signal.entry(*signal_idx).or_insert(HashSet::new());
                set.insert(value);
            }
        }
        bex_value_per_signal
    }

    pub fn calculate_gini_impurity(&self) -> f64 {
        let total_num_bex: usize = self.bex_samples.iter().map(|sample| sample.occurrence_count as usize).sum();
        
        let total_samples = self.cex_traces.len() + total_num_bex;
        if total_samples == 0 {
            return 0.0;
        }
        let cex_prob = self.cex_traces.len() as f64 / total_samples as f64;
        let bex_prob = total_num_bex as f64 / total_samples as f64;
        let cex_impurity = cex_prob * (1.0 - cex_prob);
        let bex_impurity = bex_prob * (1.0 - bex_prob);
        let total_impurity = cex_impurity + bex_impurity;
        total_impurity
    }

    pub fn get_signal_aliases(&self, signal: &str) -> Vec<Arc<str>> {
        let mut aliases: Vec<Arc<str>> = Vec::new();
        if let Some(id_code) = self.name_to_id.get(&ustr::existing_ustr(signal.as_ref()).unwrap()) {
            for (name, id) in self.name_to_id.iter() {
                if id == id_code {
                    aliases.push(Arc::from(name.as_str()));
                }
            }
        } else {
            println!("Printing all name_to_ids for debugging:");
            for name in self.name_to_id.keys() {
                println!("{}", name);
            }
            panic!("Id code of signal {} not found", signal);
        }
        aliases
    }

    pub fn signal_alias_contains_string(&self,signal_idx: u64, substring: &str) -> bool {
        for (name, id) in self.name_to_id.iter() {
            if id == &signal_idx && name.contains(substring) {
                return true;
            }
        }
        return false;
    }

    pub fn signal_alias_contains_any_substring(&self,signal_idx: u64, substrings: &[&str]) -> bool {
        for (name, id) in self.name_to_id.iter() {
            if id == &signal_idx {
                for substring in substrings.iter() {
                    if name.contains(substring) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    pub fn signal_alias_ends_with_string(&self,signal_idx: u64, substring: &str) -> bool {
        for (name, id) in self.name_to_id.iter() {
            if id == &signal_idx && name.ends_with(substring) {
                return true;
            }
        }
        return false;
    }

    pub fn get_signal_length_from_index(&self, signal_idx: &u64) -> usize {
        if let Some(signal_info) = self.id_to_signal_info.get(signal_idx) {
            return signal_info.length;
        } else {
            panic!("Signal length not found for index {}", signal_idx);
        }
    }

    pub fn get_signal_length(&self, signal: &str) -> usize {
        if let Some(id_code) = self.name_to_id.get(&ustr::existing_ustr(signal.as_ref()).unwrap()) {
            if let Some(signal_info) = self.id_to_signal_info.get(&id_code) {
                return signal_info.length;
            } else {
                panic!("Signal length not found for {} {}", signal, id_code);
            }
        } else {
            panic!("Id code of signal {} not found", signal);
        }
    }


    pub fn merge_states(&mut self, state_list: &Vec<u64>) {
        if state_list.len() < 2 {
            return;
        }
        let merge_onto = state_list[0];
        let merge_onto_bex_sample = self.bex_samples.iter().find(|s| s.state_id == merge_onto); //.unwrap().clone();
        let mut no_bex_sample_found = false;
        match merge_onto_bex_sample {
            Some(sample) => {
                let mut merge_onto_bex_sample: BexSample =  (*sample.clone()).clone();
                for state_id in state_list.iter().skip(1) {
                    self.states.remove(state_id);
                    if let Some(bex_sample) = self.bex_samples.iter().find(|s| s.state_id == *state_id) {
                        merge_onto_bex_sample.occurrence_count += bex_sample.occurrence_count;
                    }
                    self.bex_samples.retain(|s| s.state_id != *state_id);
                }
                self.bex_samples.push(Arc::new(merge_onto_bex_sample));
            },
            None => {
                //println!("No bex sample found for state id {}", merge_onto);
                no_bex_sample_found = true;
            }
        }
        let mut some_cex_sample_found = false;
        // Update the state pointer in all CEX traces
        let new_cex_trace_list = self.cex_traces.iter_mut()
            .map(|cex_trace| {
                let mutable_cex_trace = Arc::make_mut(cex_trace);
                mutable_cex_trace.contained_samples.iter_mut()
                    .for_each(|sample| {
                        if state_list.contains(&sample.state_id) {
                            some_cex_sample_found = true;
                            sample.state_id = merge_onto;
                            sample.state_pointer = Arc::downgrade(&self.states.get(&merge_onto).unwrap().clone());
                        }
                    });
                mutable_cex_trace.clone()
            })
            .collect::<Vec<_>>();
        self.cex_traces = new_cex_trace_list.into_iter().map(Arc::new).collect();
        if no_bex_sample_found && !some_cex_sample_found {
            panic!("No bex sample found for state id {} and no cex sample found either", merge_onto);
        } 
        // for cex_trace in self.cex_traces.iter_mut() {
        //     let mutable_cex_trace = Arc::get_mut(cex_trace).unwrap();
        //     for sample in mutable_cex_trace.contained_samples.iter_mut() {
        //         if state_list.contains(&sample.state_id) {
        //             sample.state_id = merge_onto;
        //             sample.state_pointer = Arc::downgrade(&self.states.get(&merge_onto).unwrap().clone());
        //         }
        //     }
        // }
    }
}


impl BexTrace {
    pub fn is_covered_by_blocked_states(&self, blocked_states: &BitSetWrapper) -> bool {
        for sample in self.contained_samples.iter() {
            if blocked_states.contains(&(sample.state_id as u32)) {
                return true;
            }
        }
        false
    }

    pub fn must_not_cover(&self) -> bool {
        return self.file_source == Some(data_types::general_data_types::WaveFormSource::MustFulfill);
    }
}

impl CexTrace {
    pub fn print(&self) {
        println!("CexTrace from: {}", self.from_path);
        println!("File source: {:?}", self.file_source);
        
        // Sort samples by and_cycle
        let mut sorted_samples = self.contained_samples.clone();
        sorted_samples.sort_by(|a, b| a.and_cycle.cmp(&b.and_cycle));
        
        for sample in sorted_samples {
            println!("  Cycle {}: ", sample.and_cycle);
            
            // Create a vector of (signal_name, signal_id, value) tuples
            let mut signal_values: Vec<(String, u64, i64)> = Vec::new();
            let state = sample.state_pointer.upgrade().unwrap();
            for (signal_id, &idx) in state.signal_id_to_index.iter() {
                let value = state.signal_values[idx];
                // Find the signal name for this ID
                if let Some((signal_name, _)) = state.string_to_id.iter().find(|(_, &id)| id == *signal_id) {
                    signal_values.push((signal_name.to_string(), *signal_id, value));
                }
            }
            
            // Sort by signal name for consistent output
            signal_values.sort_by(|a, b| a.0.cmp(&b.0));
            
            for (signal_name, signal_id, value) in signal_values {
                println!("    {} (id: {}): {}", signal_name, signal_id, value);
            }
        }
    }
}

fn get_relevant_cycles_for_core(
    mismatch_cycle_ref_core: Option<u64>,
    mismatch_cycle_dut_core: Option<u64>,
    for_core: &data_types::general_data_types::CoreType,
    max_cycle: u64,
    dut_stalled: bool,
    ref_stalled: bool,
) -> (u64, u64) {
    let start_from_cycle;
    let until_cycle;
    if *for_core == data_types::general_data_types::CoreType::RefCore {
        if mismatch_cycle_ref_core.is_none() {
            start_from_cycle = 0;
            until_cycle = max_cycle; // Will be set later
        } else {
            let mismatch_instruction_cycle = mismatch_cycle_ref_core.unwrap();
            start_from_cycle = if mismatch_instruction_cycle < (constants_module::MAX_INSTRUCTION_LIFETIME_REFCORE - 1) as u64 {
                0
            } else {
                mismatch_instruction_cycle - (constants_module::MAX_INSTRUCTION_LIFETIME_REFCORE - 1) as u64
            };
            until_cycle = if ref_stalled {
                // Extend the until cycle to include potential stalls
                // mismatch_instruction_cycle
                (mismatch_instruction_cycle + (constants_module::MAX_INSTRUCTION_LIFETIME_REFCORE -1 as u64)).max(max_cycle)
            } else {
                mismatch_instruction_cycle
            };
        }
    } else {
        if mismatch_cycle_dut_core.is_none() {
            start_from_cycle = 0;
            until_cycle = max_cycle; // Will be set later
        } else {
            let mismatch_instruction_cycle = mismatch_cycle_dut_core.unwrap();
            start_from_cycle = if mismatch_instruction_cycle < (constants_module::MAX_INSTRUCTION_CYCLE_LENGTH + 1) as u64 {
                0
            } else {
                mismatch_instruction_cycle - (constants_module::MAX_INSTRUCTION_CYCLE_LENGTH + 1) as u64
            };
            until_cycle = if dut_stalled{
                // Extend the until cycle to include potential stalls
                // mismatch_instruction_cycle
                // println!("dut stalled returning extended until cycle {} to {}", start_from_cycle, (mismatch_instruction_cycle + (constants_module::MAX_INSTRUCTION_CYCLE_LENGTH as u64 - 1u64)).max(max_cycle));
                (mismatch_instruction_cycle + (constants_module::MAX_INSTRUCTION_CYCLE_LENGTH as u64 - 1u64)).max(max_cycle)
            } else {
                // println!("dut not stalled returning until cycle {}", mismatch_instruction_cycle);
                mismatch_instruction_cycle
            };
        }
    }
    (start_from_cycle, until_cycle)
}

impl CexTrace {
    
    pub fn get_relevant_cycles(&self, _for_core: &data_types::general_data_types::CoreType) -> Option<(u64, u64)> {
        // let start_from_cycle;
        // let until_cycle;
        // if *for_core == data_types::general_data_types::CoreType::RefCore {
        //     let mismatch_instruction_cycle = self.get_mismatch_ref_core();
        //     if mismatch_instruction_cycle.is_none() {
        //         start_from_cycle = 0;
        //         until_cycle = self.contained_samples.iter().map(|x| x.and_cycle).max();
        //     } else {
        //         let mismatch_instruction_cycle = mismatch_instruction_cycle.unwrap();
        //         start_from_cycle = if mismatch_instruction_cycle < (constants_module::MAX_INSTRUCTION_LIFETIME_REFCORE-1 as u64) { 0 } else { mismatch_instruction_cycle - (constants_module::MAX_INSTRUCTION_LIFETIME_REFCORE-1 as u64) };
        //         until_cycle = Some(mismatch_instruction_cycle); //Inclusive
        //         //println!("Start from cycle {} until cycle {}", start_from_cycle, until_cycle);
        //     }
        // } else {
        //     let mismatch_instruction_cycle = self.get_first_mismatch_dut_core();
        //     if mismatch_instruction_cycle.is_none() {
        //         start_from_cycle = 0;
        //         until_cycle = self.contained_samples.iter().map(|x| x.and_cycle).max();
        //     } else {
        //         let mismatch_instruction_cycle = mismatch_instruction_cycle.unwrap();
        //         start_from_cycle = if mismatch_instruction_cycle < ((constants_module::MAX_INSTRUCTION_CYCLE_LENGTH+1) as u64) { 0 } else { mismatch_instruction_cycle - (constants_module::MAX_INSTRUCTION_CYCLE_LENGTH as u64 + 1u64) };   
        //         until_cycle = Some(mismatch_instruction_cycle);//Inclusive
        //     }
        // }
        let start_from_cycle = self.contained_samples.iter().map(|x| x.and_cycle).min().unwrap_or(0);
        let until_cycle = self.contained_samples.iter().map(|x| x.and_cycle).max();
        if until_cycle.is_none() {
            println!("No until cycle found? Samples {:?} from_path {:?}", self.contained_samples, self.from_path);
            return None;
        } else {
            return Some((start_from_cycle, until_cycle.unwrap()));
        }
        
    }

    pub fn is_covered_by_blocked_states(&self, blocked_states: &BitSetWrapper) -> bool {
        for sample in self.contained_samples.iter() {
            if blocked_states.contains(&(sample.state_id as u32)) {
                return true;
            }
        }
        false
    }

    pub fn get_intersection_with_states(&self, blocked_states: &BitSetWrapper) -> BitSetWrapper {
        let mut intersection: BitSetWrapper = BitSetWrapper::new();
        for sample in self.contained_samples.iter() {
            if blocked_states.contains(&(sample.state_id as u32)) {
                intersection.add(sample.state_id as u32);
            }
        }
        intersection
    }

    pub fn try_get_value_at_cycle(&self, cycle: u64, signal_name: &str) -> Option<i64> {
        for sample in self.contained_samples.iter().sorted_by(|a, b| a.and_cycle.cmp(&b.and_cycle)) {
            if sample.and_cycle == cycle {
                let state = sample.state_pointer.upgrade().unwrap();
                return Some(state.get_signal_value_from_string(signal_name));
            }
        }
        None
    }

    pub fn get_mismatch_ref_core(&self) -> Option<u64> {
        return self.mismatch_cycle_ref_core;
    }
    pub fn get_first_mismatch_dut_core(&self) -> Option<u64> {
        return self.mismatch_cycle_dut_core;
    }
}

impl StateMapping {

    pub fn get_signal_value_from_index(&self, signal_index: &u64) -> i64 {
        match self.signal_id_to_index.get(signal_index) {
            Some(&idx) => self.signal_values[idx],
            None => {
                panic!("Signal index {} not found in state mapping signal {:?}", signal_index, self.string_to_id.iter().find(|(_, &id)| id == *signal_index).map(|(name, _)| name.clone()));
            }
        }
    }

    pub fn get_signal_value_from_string(&self, signal_name: &str) -> i64 {
        let signal_index = self.string_to_id.get(&ustr::existing_ustr(signal_name).unwrap()).unwrap();
        self.get_signal_value_from_index(signal_index)
    }

    pub fn get_projection_from_signal_idx(&self, signal_list: &Vec<u64>) -> Vec<i64> {
        let mut ret_vec =  Vec::new();
        for signal in signal_list {
            ret_vec.push(self.get_signal_value_from_index(signal));
        }
        ret_vec
    }
    
    pub fn get_projection(&self, signal_list: &Vec<&str>) -> Vec<i64> {
        let mut ret_vec =  Vec::new();
        for signal in signal_list {
            ret_vec.push(self.get_signal_value_from_string(signal));
        }
        ret_vec
    }
}

type Key = Box<[i64]>;                // cheaper to hash/clone than Vec
#[derive(Default)]
struct StateVal {
    count: u64,
    paths: Vec<ustr::Ustr>,             // dedup + cheap clone,
    cycles: Vec<CycleCount>,             // cycles where this state was seen
    file_sources: Vec<data_types::general_data_types::WaveFormSource>, // file sources where this state was seen
}

pub fn get_signal_types(signal_alias_list: &Vec<ustr::Ustr>, signal_length: u64) -> data_types::general_data_types::SignalTypesSet {
    let mut types = data_types::general_data_types::SignalTypesSet::new();
    if signal_length <= 1{
        let clock_and_reset = ["clk", "clock", "rst", "reset", "rst_n", "reset_n"];
        if clock_and_reset.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
            return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::ClockReset);
        }
        let control_signal = ["added", "csr_written"];
        if control_signal.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
            return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Control);
        }
        // return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Control);
    }
    let compiled_register_regexes_boom: Vec<Regex> =  ["uop.*lrs.*", "uop.*ldst.*", "uop.*pdst.*", "uop.*prs.*"].iter().map(|s| Regex::new(s).unwrap()).collect();
    let compiled_index_regexes_boom: Vec<Regex> =  ["uop.*q.*idx", "uop.*rob.*idx"].iter().map(|s| Regex::new(s).unwrap()).collect();
    let compiled_csr_uop_regexes_boom: Vec<Regex> =  ["uop.*csr.*"].iter().map(|s| Regex::new(s).unwrap()).collect();
    let control_like_signals = ["opcode", "funct3", "funct7", "type", "branch", "jump", "uop", "ctrl"]; //, "we", "wen", "memread", "memwrite", "memtoreg", "alusrc", "regwrite", "aluop", "valid", "ready", "stall", "flush", "interrupt", "exception"];
    if control_like_signals.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
        if signal_length < 20 { // We do not want to restrict actual branch targets, only enums
            let is_uop_register = compiled_register_regexes_boom.iter().any(|regex| signal_alias_list.iter().any(|alias| regex.is_match(&alias.to_lowercase())));
            if is_uop_register {
                return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Register);
            }
            let is_uop_index = compiled_index_regexes_boom.iter().any(|regex| signal_alias_list.iter().any(|alias| regex.is_match(&alias.to_lowercase())));
            if is_uop_index {
                return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Counter);
            }
            let is_uop_csr = compiled_csr_uop_regexes_boom.iter().any(|regex| signal_alias_list.iter().any(|alias| regex.is_match(&alias.to_lowercase())));
            if is_uop_csr {
                return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::RegisterFileAddress).with_type(data_types::general_data_types::SignalType::Immediate);
            }
            types.insert(data_types::general_data_types::SignalType::Control);
            if signal_alias_list.iter().any(|alias| alias.to_lowercase().contains("funct7")) {
                return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Funct7);
                // types.insert(data_types::general_data_types::SignalType::Immediate);
            }
        }
        
    }
    let register_like_signals_regex = ["reg", "rs1", "rs2", "rd", "rs", "rt", "rd", "ra", "wb", "rpend"];
    let compiled_regexes: Vec<Regex> = register_like_signals_regex.iter().map(|s| Regex::new(s).unwrap()).collect();
    
    if compiled_regexes.iter().chain(compiled_register_regexes_boom.iter()).any(|regex| signal_alias_list.iter().any(|alias| regex.is_match(&alias.to_lowercase()))) {
        if signal_length == 5 {
                return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Register);
        } else {
            //Special case for BOOM uop.lrs fields, which can be of any length
            if compiled_register_regexes_boom.iter().any(|regex| signal_alias_list.iter().any(|alias| regex.is_match(&alias.to_lowercase()))) {
                return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Register);
            }
        }
        // }  else {
        //     println!("Signal length is not 5 (it is {}) for register like signal {:?})", signal_length, signal_alias_list);
        // }
    }
    // let csr_like_signals = ["imm_i","csr","mepc", "sepc", "mtvec", "stvec", "satp", "mstatus", "sstatus", "mie", "sie", "mcause", "scause", "mtval", "stval"];
    let csr_like_signals_regex = ["imm_i","csr.*addr.*","mepc", "sepc", "mtvec", "stvec", "satp", "mstatus", "sstatus", "mie", "sie", "mcause", "scause", "mtval", "stval"];
    let compiled_csr_regexes: Vec<Regex> = csr_like_signals_regex.iter().map(|s| Regex::new(s).unwrap()).collect();
    if signal_length < 32 && compiled_csr_regexes.iter().any(|regex| signal_alias_list.iter().any(|alias| regex.is_match(&alias.to_lowercase()))) {
        return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::RegisterFileAddress).with_type(data_types::general_data_types::SignalType::Immediate);
    }

    let address_like_signals = ["pc", "sp", "epc", "vaddr", "paddr", "vpn", "ppn", "asid", "tag", "index", "addr","pc_lob" ];
    if address_like_signals.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
        if signal_length > 20 {
            return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Address);
        }
    }
    
    let immediate_like_regex = [".*alu.*in1", ".*alu.*in2", ".*alu.*", ".*immediate.*"];
    let compiled_immediate_regexes: Vec<Regex> = immediate_like_regex.iter().map(|s| Regex::new(s).unwrap()).collect();
    if compiled_immediate_regexes.iter().any(|regex| signal_alias_list.iter().any(|alias| regex.is_match(&alias.to_lowercase()))) {
        return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Immediate);
    }
    let immediate_like_signals = ["imm", "immediate"];
    if immediate_like_signals.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
        return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Immediate);
    }
    let immediate_like_signals = ["imm", "immediate"];
    if immediate_like_signals.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
        return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Immediate);
    }
    let data_like_signals = ["data", "rdata ", "value", "imm", "result", "mem"];
    if data_like_signals.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
        return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Data);
    }
    let counter_like_signals = ["cycle", "time", "timer", "tick", "count", "ctr", "idx"];
    if counter_like_signals.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
        return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Counter);
    }
    let instruction_like_signals = ["instr", "instruction", "inst"];
    if signal_length == 32 && instruction_like_signals.iter().any(|s| signal_alias_list.iter().any(|alias| alias.to_lowercase().contains(s))) {
        return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Instruction);
    }
    if signal_length >= 6 && types.is_empty() {
        return data_types::general_data_types::SignalTypesSet::new_from_type(data_types::general_data_types::SignalType::Unknown); //Could be data or address, but we do not know. Unlikely that it is control.
    }
    //Vincent: I tried changing this to data, but then we miss a lot of enums, which are often control signals
    return types; //Default to Control if we do not know
}

pub fn new_from_fuzzer_data_point(cex_datapoints: &Vec<data_types::general_data_types::FuzzerDataPoint>, bex_datapoints: &Vec<data_types::general_data_types::FuzzerDataPoint>, clock_signal: &str, stage_filter: Option<&data_types::general_data_types::StageFilter>, 
prefiltered_idx_list: Option<&SignalIndexSet>,predicate_generation_config: &allowed_signal_config::PredicateGenerationConfig
) -> Teacher {
    //We would like to process each bex_datapoint in parallel. 
    //For each bex_datapoint, we want to first get the corresponding waveform
    //Then, add the bex samples to our list of samples if it does not exists yet
    //Then drop the waveform again
    
    let mut signal_to_idx_mapping:  HashMap<ustr::Ustr,u64> =HashMap::new();
    // let mut signal_length_mapping: HashMap<u64, usize> = HashMap::new();
    // let mut signal_alias_mapping: HashMap<u64, Vec<ustr::Ustr>> = HashMap::new();
    let mut id_to_signal_info: HashMap<u64, SignalInfo> = HashMap::new();
    let signal_changed= dashmap::DashSet::<u64>::new();
    let cex_traces: RwLock<Vec<Arc<CexTrace>>> = RwLock::new(Vec::with_capacity(cex_datapoints.len()));
    let bex_traces: DashMap<ustr::Ustr,BexTrace> = DashMap::new();
    let bex_samples: DashMap<u64, Arc<BexSample>> = DashMap::with_capacity(bex_datapoints.len());
    let filtered_bex_count = AtomicUsize::new(0);
    let state_id_count = AtomicUsize::new(0);
    let deduplicated_cex_states_cnt = AtomicUsize::new(0);
    let deduplicated_bex_states_cnt = AtomicUsize::new(0);
    let states_map: DashMap<Vec<i64>, u64, DefaultVectorHasher> = DashMap::with_capacity_and_hasher(1000, DefaultVectorHasher::default());
    let states_dashmap: DashMap<Key, StateVal, DefaultVectorHasher> = DashMap::with_capacity_and_hasher(1000, DefaultVectorHasher::default());
    let final_state_map: DashMap<u64, Arc<StateMapping>> = DashMap::new();
    let mut tracked_signal_idx: HashSet<u64, data_types::general_data_types::DefaultScalarHasher> = HashSet::default();
    let some_data_point = if cex_datapoints.len() > 0 {
        cex_datapoints.first().unwrap()
    } else {
        bex_datapoints.first().unwrap()
    };
    let sorted_bex_datapoints = bex_datapoints.iter().sorted_by(|a, b| {
        if a.file_source == data_types::general_data_types::WaveFormSource::Mutations && b.file_source != data_types::general_data_types::WaveFormSource::Mutations {
            Ordering::Less
        } else if a.file_source != data_types::general_data_types::WaveFormSource::Mutations && b.file_source == data_types::general_data_types::WaveFormSource::Mutations {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }).collect::<Vec<&FuzzerDataPoint>>();
    //println!("Sorted bex datapoints {:?}", sorted_bex_datapoints);
    // println!("Loading waveform from first datapoint to get signal mapping with {:?} JG signals", jg_allowed_signals.as_ref().map(|inner| inner.len()).unwrap_or(0));
    let maybe_loose_idx_filter = match prefiltered_idx_list {
        Some(set) => {
            let mut filters = signal_filters::SignalFilters::new();
            //Only loose filter, we also want to retain metadata signals
            filters.add_filter(signal_filters::SignalFilter::LooseSignalIdxFilter(set.clone()));
            println!("Using prefiltered signal idx list with {} signals", set.len());
            Some(filters)
        },
        None => None
    };
    let some_waveform = waveform::WaveForm::load_waveform_and_cycle_map_from_fuzzer_datapoint(some_data_point, clock_signal, maybe_loose_idx_filter.as_ref()).unwrap();
    println!("Loading first waveform done has {:?} signals", some_waveform.name_to_code.len());
    let include_regex = match stage_filter {
        Some(filter) => Some(filter.include.clone()),
        None => None
    };
    let exclude_regex = match stage_filter {
        Some(filter) => Some(filter.exclude.clone()),
        None => None
    };
    let mut filter_signal_idx: HashSet<u64, data_types::general_data_types::DefaultScalarHasher> = HashSet::default();
    let metadata_signal_names = vec![
        constants_module::COUNTER_SIGNAL,
        clock_signal,
        constants_module::INCORRECTNESS_SIGNAL,
        constants_module::MISMATCH_CYCLE_REF_CORE_SIGNAL,
        constants_module::MISMATCH_CYCLE_DUT_CORE_SIGNAL,
        constants_module::DUT_STALL_SIGNAL,
        constants_module::REFCORE_STALL_SIGNAL,
    ];
    for (signal_name, idx) in some_waveform.name_to_code.iter() {
        let signal_length = some_waveform.get_signal_length(signal_name.as_ref());
        if utils::signal_passes_filter(signal_name, signal_length, &include_regex, &exclude_regex) {
            signal_to_idx_mapping.insert(signal_name.clone(), (*idx).into());
            let aliases = some_waveform.get_signal_aliases_ustr_from_idx(idx);
            for alias in aliases.iter() {
                //Why do we insert all aliases here? Because if we do not do this, 
                //but insert all aliases when we create the signal info (later in this loop),
                //then we might run into the situation where a predicate is w.r.t. to a signal name \
                // that is not in the name_to_code mapping (derived from this signal_to_idx_mapping)
                signal_to_idx_mapping.insert(alias.clone(), (*idx).into());
            }
            if id_to_signal_info.get(idx).is_none() {
                let signal_length = some_waveform.get_signal_length(signal_name.as_ref());
                let aliases = some_waveform.get_signal_aliases_ustr_from_idx(idx);
                let signal_types = get_signal_types(&aliases, signal_length as u64);
                // if aliases.iter().any(|a| a.to_lowercase().contains("cs_decoder_decoded_invInputs")) {
                //     println!("Signal {} with length {} has aliases {:?} and type {:?}", signal_name, signal_length, aliases, signal_type);
                // }
                // if signal_type == data_types::general_data_types::SignalType::Unknown {
                //     log::debug!("Warning: Signal {} with length {} has unknown type, aliases {:?}. Skipping", signal_name, signal_length, aliases);
                //     continue;
                // }
                // if signal_type == data_types::general_data_types::SignalType::ClockReset {
                //     //We do not want to track clock or reset signals
                //     log::debug!("Skipping signal {} with length {} and type {:?} because we do not track clock or reset signals", signal_name, signal_length, signal_type);
                //     continue;
                // }
                // if signal_type == data_types::general_data_types::SignalType::Counter || signal_type == data_types::general_data_types::SignalType::Data {
                //     //We do not track counter or data signals for now
                //     log::debug!("Skipping signal {} with length {} and type {:?} because we do not track counter or data signals", signal_name, signal_length, signal_type);
                //     continue;
                // }
                if !(predicate_generation_config.allows_types(&signal_types)) {
                    log::debug!("Skipping signal {} (alias {:?}) with length {} and type {:?} because it is not allowed by the predicate generation config", signal_name, aliases, signal_length, signal_types);
                    continue;
                }
                    
                log::debug!("Including signal {} with length {} and type {:?} and alias {:?}", signal_name, signal_length, signal_types, aliases);
                let signal_info = SignalInfo {
                    id: (*idx).into(),
                    length: signal_length,
                    signal_types,
                    aliases: aliases.clone(),
                };
                // if signal_info.any_alias_contains("nextSmall_", false){
                    // log::debug!("Signal info for nextSmall_: {:?}", signal_info);
                // }
                // if signal_info.any_alias_contains("FpPipeline", false){
                    // log::error!("Signal info for FpPipeline: {:?}", signal_info);
                // }
                id_to_signal_info.insert((*idx).into(), signal_info);
            } else {
                //Signal info already exists, we do not need to insert it again
                if aliases.iter().any(|a| a.to_lowercase().contains("cs_decoder_decoded_invInputs")) {
                    println!("Signal {} with length {} has aliases {:?} and type {:?}", signal_name, signal_length, aliases, id_to_signal_info.get(idx).unwrap().signal_types);
                }
            }

            filter_signal_idx.insert(*idx);
            tracked_signal_idx.insert(*idx);
        } else {
            log::debug!("Excluding signal {} because it does not pass the regex filter, signal_length {:?}", signal_name, signal_length);
        }
        // else if signal_name.contains("opcode") {
        //     // println!("Excluding opcode signal {} because it does not pass the filter, signal_length {:?}", signal_name, signal_length);
        // }
        if metadata_signal_names.contains(&signal_name.as_ref()) {
            filter_signal_idx.insert(*idx);
        }

    }
    // panic!("done");
    let core_type = match stage_filter {
        Some(filter) => filter.core_type.clone(),
        None => data_types::general_data_types::CoreType::DutCore
    };
    let this_signal_idx = tracked_signal_idx.iter().map(|x| *x).collect::<Vec<u64>>();
    
    // Create mapping from signal ID to index in the dense vector
    let mut id_to_idx_map = HashMap::with_capacity_and_hasher(this_signal_idx.len(), DefaultScalarHasher::default());
    for (i, &id) in this_signal_idx.iter().enumerate() {
        id_to_idx_map.insert(id, i);
    }
    let signal_id_to_index = Arc::new(id_to_idx_map);

    let signal_to_idx_mapping = Arc::new(signal_to_idx_mapping);
    // let counter_signal_idx = signal_to_idx_mapping.get(&ustr::existing_ustr(constants_module::COUNTER_SIGNAL).unwrap()).unwrap();
    println!("Creating teacher from fuzzer data points, cex: {}, bex: {}, num signals {}", cex_datapoints.len(), bex_datapoints.len(), this_signal_idx.len());
    let signal_filter = signal_filters::SignalFilter::StrictSignalIdxFilter(filter_signal_idx.clone());
    let signal_filter: signal_filters::SignalFilters = signal_filters::SignalFilters {
        filters: vec![signal_filter],
    }; 
    if ustr::existing_ustr(constants_module::COUNTER_SIGNAL).is_none() {
        panic!("Counter signal {} not found in pre-aggregated strings example waveform path {}", constants_module::COUNTER_SIGNAL, some_waveform.path);
    }
    if some_waveform.name_to_code.get(&ustr::existing_ustr(constants_module::COUNTER_SIGNAL).unwrap()).is_none() {
        panic!("Counter signal {} not found in waveform name_to_code mapping example waveform path {}", constants_module::COUNTER_SIGNAL,   some_waveform.path);
    }
    let cycle_count_idx = some_waveform.name_to_code.get(&ustr::existing_ustr(constants_module::COUNTER_SIGNAL).unwrap()).unwrap();
    //Here, we already filtered out all signals that are not matching jg signals
    // for idx in this_signal_idx.iter() {
    //     println!("Included signal {} with names", idx);
    //     for (name, id) in some_waveform.name_to_code.iter() {
    //         if id == idx {
    //             print!("  - {}", name);
    //         }
    //     }
    //     println!("");
    // }

    let max_bex = constants_module::ATOMIC_MAX_NUM_BEX.load(atomic::Ordering::Relaxed);
    let bex_slice = if sorted_bex_datapoints.len() > max_bex {
        &sorted_bex_datapoints[0..max_bex]
    } else {
        &sorted_bex_datapoints[..]
    };

    bex_slice.par_iter().progress().for_each(|datapoint| { 
        // let start = Instant::now();
        let waveform = waveform::WaveForm::load_waveform_and_cycle_map_from_fuzzer_datapoint(datapoint, clock_signal, Some(&signal_filter)).unwrap();
        // let duration = start.elapsed();
        // println!(" Thread id {:?} Loaded bex waveform {} with {} cycles and {} signals in {:?}", std::thread::current().id(), waveform.path, waveform.num_cycles, waveform.name_to_code.len(), duration);
        // let this_bex_trace = BexTrace {
        //     from_path: waveform.path,
        //     file_source: Some(datapoint.file_source.clone()),
        //     contained_samples: Vec::new(), //Will be filled later
        // };
        for signal_idx in this_signal_idx.iter() {
            if !(waveform.signal_is_constant(signal_idx)){
                signal_changed.insert(*signal_idx);
            }
        }
        let mut state = Vec::with_capacity(this_signal_idx.len());
        let _contained_samples: Vec<ContainedState> = Vec::new();
        for cycle in 0..waveform.num_cycles {
            state.clear();
            let counter_signal_value = waveform.get_signal_value_at_cycle_from_id(cycle_count_idx, &cycle).unwrap() as u64;
            
            for signal_idx in this_signal_idx.iter() {
                // if signal_idx == counter_signal_idx {
                //     continue;
                // }
                                    //let signal_idx = *(signal_to_idx_mapping.get(&signal_name).unwrap());
                let value = waveform.get_signal_value_at_cycle_from_id(signal_idx, &cycle).unwrap();
                state.push(value);
            }
            let key: Key = state.clone().into_boxed_slice();
            states_dashmap.entry(key).and_modify(|e| {
                e.count += 1;
                e.paths.push(waveform.path);
                e.cycles.push(counter_signal_value as CycleCount);
                e.file_sources.push(datapoint.file_source.clone());
                //e.file_sources.sort_unstable();
                filtered_bex_count.fetch_add(1, atomic::Ordering::SeqCst);
            }).or_insert_with(|| {
                let paths = vec![waveform.path];
                let cycles = vec![counter_signal_value as CycleCount];
                let file_sources = vec![datapoint.file_source.clone()];
                StateVal { count: 1, paths, cycles , file_sources}
            });
        }
        drop(waveform);
    });
    println!("Finished loading bex waveforms, filtered out {} bex states that were already known", filtered_bex_count.load(atomic::Ordering::SeqCst));
    let total_states = states_dashmap.len();
    
    // Process states_dashmap directly to create final_state_map and bex traces/samples
    // Using par_bridge for parallelism without collecting into an intermediate Vec
    println!("Processing {} unique states directly from dashmap with {} signals", total_states, this_signal_idx.len());
    states_dashmap.into_iter()
        .par_bridge()
        .progress_count(total_states as u64)
        .for_each(|(state, sv)| {
            // Extract only what we need from StateVal
            let take_idx = sv.file_sources.iter().enumerate()
                .min_by_key(|&(_, source)| source)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let any_source_seed = sv.file_sources.iter().any(|x| x == &data_types::general_data_types::WaveFormSource::Seed);
            let count = if any_source_seed {
                sv.count * 10
            } else {
                sv.count
            };
            
            // Generate state_id
            let state_id = state_id_count.fetch_add(1, atomic::Ordering::SeqCst) as u64;

            let state_mapping = Arc::new(StateMapping {
                string_to_id: Arc::clone(&signal_to_idx_mapping),
                signal_id_to_index: Arc::clone(&signal_id_to_index),
                signal_values: state.to_vec(),
                state_id,
            });

            final_state_map.insert(state_id, Arc::clone(&state_mapping));
            
            // Create bex traces for all occurrences of this state
            for i in 0..sv.paths.len() {
                let this_path = sv.paths[i];
                let this_cycle = sv.cycles[i];
                let weight = if this_path.as_str().contains("ci") || this_path.as_str().contains("cai") {
                    0.025
                } else {
                    1.0
                };
                bex_traces.entry(this_path).or_insert_with(|| BexTrace {
                    from_path: this_path,
                    file_source: Some(sv.file_sources[i].clone()),
                    contained_samples: Vec::new(),
                    weight: weight
                }).contained_samples.push(ContainedState {
                    state_id,
                    and_cycle: this_cycle as u64,
                    state_pointer: Arc::downgrade(&state_mapping),
                });
            }

            // Create single bex sample using the preferred occurrence
            let bex_sample = BexSample {
                from_path: sv.paths[take_idx],
                and_cycle: sv.cycles[take_idx] as u64,
                file_source: Some(sv.file_sources[take_idx].clone()),
                occurrence_count: count as u64,
                state_id,
                state_pointer: Arc::downgrade(&state_mapping),
                paths_and_cycles: None,
            };

            bex_samples.insert(state_id, Arc::new(bex_sample));

            // Explicitly drop state and sv to free memory immediately
            drop(state);
            drop(sv);
        });
    for bex_sample in bex_samples.iter() {
        let bex_sample = bex_sample.value();
        if bex_sample.file_source == Some(data_types::general_data_types::WaveFormSource::MustFulfill) {
            // println!("Bex sample {} from path {} with and cycle {} has file source MustFulfill", bex_sample.state_id, bex_sample.from_path, bex_sample.and_cycle);
        }
    }
    println!("Filtered {:} bex states initially", filtered_bex_count.load(atomic::Ordering::SeqCst));
    
    let max_cex = constants_module::ATOMIC_MAX_NUM_CEX.load(atomic::Ordering::Relaxed);
    let mut cex_slice: Vec<&FuzzerDataPoint> = if cex_datapoints.len() > max_cex {
        cex_datapoints.iter().take(max_cex).collect()
    } else {
        cex_datapoints.iter().collect()
    };
    if let Some((orig_idx, orig_cex)) = cex_datapoints
        .iter()
        .enumerate()
        .find(|(_, datapoint)| datapoint.file_source == data_types::general_data_types::WaveFormSource::OriginalCex)
    {
        let original_in_slice = orig_idx < max_cex || cex_datapoints.len() <= max_cex;
        if !original_in_slice {
            if cex_slice.is_empty() {
                cex_slice.push(orig_cex);
            } else {
                let last_idx = cex_slice.len() - 1;
                cex_slice[last_idx] = orig_cex;
            }
        }
    } else {
        log::warn!("No OriginalCex datapoint found in CEX list");
    }

    cex_slice.par_iter().progress().for_each(|datapoint| {
        let datapoint = *datapoint;
        let waveform = waveform::WaveForm::load_waveform_and_cycle_map_from_fuzzer_datapoint(datapoint, clock_signal, Some(&signal_filter)).unwrap();
        for signal_idx in this_signal_idx.iter() {
            if !(waveform.signal_is_constant(signal_idx)){
                signal_changed.insert(*signal_idx);
            }
        }
        let mut samples_for_this_cex: Vec<ContainedState> = Vec::new(); 
        //println!("Processing cex {}", cex.path);
        let mismatch_cycle_ref = waveform.get_mismatch_ref_core();
        let mismatch_cycle_dut = waveform.get_first_mismatch_dut_core();
        let dut_stalled = waveform.was_stalled_dut();
        let ref_stalled  = waveform.was_stalled_refcore();
        // println!("Cex waveform {} mismatch_cycle_ref {:?}, mismatch_cycle_dut {:?}, dut_stalled {}, ref_stalled {}", waveform.path, mismatch_cycle_ref, mismatch_cycle_dut, dut_stalled, ref_stalled);
        let (start_cycle, until_cycle) = get_relevant_cycles_for_core(mismatch_cycle_ref, mismatch_cycle_dut, &core_type, CycleCountConversion::from_cycle_count(waveform.num_cycles), dut_stalled, ref_stalled);
        for cycle in 0..waveform.num_cycles {
            let mut state = Vec::new();
            let counter_signal_value = waveform.get_signal_value_at_cycle(constants_module::COUNTER_SIGNAL, cycle).unwrap() as u64;
            if counter_signal_value < start_cycle || counter_signal_value > until_cycle {
                //println!("Skipping cycle {} for cex {}, counter signal value {} not in range {}-{}", cycle, datapoint.path, counter_signal_value, start_cycle, until_cycle);
                continue;
            }
            for signal_idx in this_signal_idx.iter() {
                // if (signal_idx == counter_signal_idx) {
                //     continue;
                // }
                let value = waveform.get_signal_value_at_cycle_from_id(signal_idx, &cycle).unwrap();
                state.push(value);
            }

            // println!("Thread {:?} is waiting for lock on states_map", std::thread::current().id());
            //let mut state_map_lock = states_map.lock().unwrap();
            // println!("Thread {:?} got lock on states_map", std::thread::current().id());

            let key: Key = state.clone().into_boxed_slice();
            match states_map.entry(key.to_vec()) {
                dashmap::Entry::Occupied(e) => {
                    let state_id = *e.get();
                    // safe: the writer inserts into final_state_map BEFORE publishing state_id (see Vacant branch)
                    let state_arc = final_state_map
                        .get(&state_id)
                        .expect("final_state_map not published yet")
                        .clone();

                    let state_weak_arc = Arc::downgrade(&state_arc);
                    let contained_state = ContainedState {
                        state_id,
                        and_cycle: counter_signal_value,
                        state_pointer: state_weak_arc,
                    };
                    samples_for_this_cex.push(contained_state);
                    deduplicated_cex_states_cnt.fetch_add(1, atomic::Ordering::Relaxed);
                }
                dashmap::Entry::Vacant(v) => {
                    let state_id = state_id_count.fetch_add(1, atomic::Ordering::Relaxed) as u64;

                    let state_mapping = Arc::new(StateMapping {
                        string_to_id: signal_to_idx_mapping.clone(), // consider Arc to avoid heavy clones
                        signal_id_to_index: Arc::clone(&signal_id_to_index),
                        signal_values: state.clone(),
                        state_id,
                    });

                    // 1) Publish the mapping first.
                    final_state_map.insert(state_id, state_mapping.clone());

                    // 2) Only then publish the id in states_map (other threads will now always find the mapping).
                    v.insert(state_id);

                    let contained_state = ContainedState {
                        state_id,
                        and_cycle: counter_signal_value,
                        state_pointer: Arc::downgrade(&state_mapping),
                    };
                samples_for_this_cex.push(contained_state);
                }
            }
        }
        let res = CexTrace{contained_samples: samples_for_this_cex, from_path: waveform.path.clone(), file_source: Some(datapoint.file_source.clone()),
                        mismatch_cycle_ref_core: mismatch_cycle_ref, mismatch_cycle_dut_core: mismatch_cycle_dut};
        if res.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
            // println!("First incorrect cycle for original cex {:?}", res.get_first_incorrect_cycle());
            println!("First dut incorrect cycle for original cex {:?} {:?}", res.from_path, res.get_first_mismatch_dut_core());
            println!("First ref incorrect cycle for original cex {:?} {:?}", res.from_path, res.get_mismatch_ref_core());
            if res.get_first_mismatch_dut_core() == res.get_mismatch_ref_core() {
                log::warn!("Warning: Original cex {:?} has same mismatch cycle for ref and dut core {:?}", res.from_path, res.get_first_mismatch_dut_core());
            }
            

        }
        cex_traces.write().unwrap().push(Arc::new(res));
        drop(waveform);
    });
    println!("From fuzzer data point Deduplicated {} cex states, {} bex states", deduplicated_cex_states_cnt.load(atomic::Ordering::SeqCst), deduplicated_bex_states_cnt.load(atomic::Ordering::SeqCst));
    //TODO Vincent: I need to correct this!
    let signal_changed_reader = signal_changed;
    //println!("Signal changed reader {:?}", signal_changed_reader);
    let non_constant_signal_idx = signal_to_idx_mapping.values().filter(|x| {
        
        if signal_changed_reader.contains(x) {
            return true;
        } else {
            return false;
            //println!("Signal idx {} with alias {:?} is constant", x, id_to_signal_info.get(x).map(|info| info.aliases.clone()));
            //return true; //false
        }
    }).cloned().collect::<Vec<u64>>();

    println!("signal_to_idx_mapping len {}", signal_to_idx_mapping.len());
    println!("Total number of states {}", final_state_map.len());
    println!("Total number of non_constant signals {}", non_constant_signal_idx.len());
    let mut name_to_id = signal_to_idx_mapping.as_ref().clone();
    name_to_id.retain(|_, v| non_constant_signal_idx.contains(v));
    println!("Total signal idx {}, num non-constant signal idx {}", this_signal_idx.len(), non_constant_signal_idx.len());
    let name_to_id = Arc::new(name_to_id);
    let cex_samples = cex_traces.into_inner().unwrap();
    // let bex_samples = bex_samples.into_inner().unwrap();
    println!("Resulting number of bex samples {:?}", bex_samples.len());
    let bex_samples_vec = bex_samples.into_iter().map(|x| x.1).collect::<Vec<Arc<BexSample>>>();
    let final_state_map = final_state_map.into_iter().collect::<HashMap<u64, Arc<StateMapping>>>();
    let bex_traces: Vec<Arc<BexTrace>> = bex_traces.into_iter().map(|(_, v)| Arc::new(v)).collect();

    // Update benchmarking stats
    constants_module::BENCHMARK_NUM_CEX.store(cex_samples.len(), atomic::Ordering::Relaxed);
    constants_module::BENCHMARK_NUM_BEX.store(bex_traces.len(), atomic::Ordering::Relaxed);

    let teacher = Teacher {cex_traces: cex_samples, bex_traces: bex_traces, bex_samples: bex_samples_vec, name_to_id: name_to_id, id_to_signal_info: Arc::new(id_to_signal_info), values_per_signal:None, states: final_state_map};
    teacher
}


pub fn new_from_samples(
    cex_samples: Vec<Arc<CexTrace>>,
    bex_samples: Vec<Arc<BexSample>>,
    old_teacher: &Teacher,
) -> Teacher {
    return Teacher {cex_traces: cex_samples, bex_traces: old_teacher.bex_traces.clone(), bex_samples: bex_samples,
        name_to_id:old_teacher.name_to_id.clone(),
        id_to_signal_info: old_teacher.id_to_signal_info.clone(),
        states: old_teacher.states.clone(),
        values_per_signal: None,
    };
}

pub fn new_from_traces(
    cex_traces: Vec<Arc<CexTrace>>,
    bex_traces: Vec<Arc<BexTrace>>,
    old_teacher: &Teacher,
) -> Teacher {
    return Teacher {cex_traces: cex_traces, bex_traces: bex_traces, bex_samples: old_teacher.bex_samples.clone(),
        name_to_id:old_teacher.name_to_id.clone(),
        id_to_signal_info: old_teacher.id_to_signal_info.clone(),
        states: old_teacher.states.clone(),
        values_per_signal: None,
    };
}
