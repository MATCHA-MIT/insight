use crate::constants;
use crate::costs;
use crate::cycle_types;
use crate::data_types;
use crate::data_types::general_data_types::StateIdType;
use crate::predicates;
use crate::teacher;
use rayon::prelude::*;
use std::collections::HashMap;
use std::collections::HashSet;
// use partitions::PartitionVec;

#[derive(Clone)]
pub struct SetCoverPredicate {
    pub id: usize,
    pub covered_states: data_types::general_data_types::BitSetWrapper,
    pub predicate: predicates::BasePredicateCandidate,
}

#[derive(Clone)]
pub struct SetCoverBEXSample {
    pub state_id: data_types::general_data_types::StateIdType,
    pub weight: u64,
    pub from_path: ustr::Ustr,
    pub must_not_cover: bool,
}

#[derive(Clone)]
pub struct SetCoverCEXSample {
    pub state_id: data_types::general_data_types::StateIdType,
    pub cycle: cycle_types::CycleCount,
}

#[derive(Clone)]
pub struct SetCoverCEXTrace {
    pub id: usize,
    pub weight: u64,
    pub states: HashMap<
        data_types::general_data_types::StateIdType,
        SetCoverCEXSample,
        data_types::general_data_types::DefaultScalarHasher,
    >,
    pub from_path: ustr::Ustr,
    pub file_source: Option<data_types::general_data_types::WaveFormSource>,
}

#[derive(Clone)]
pub struct UnweightedBEXSample {
    pub state_id: data_types::general_data_types::StateIdType,
    pub cycle: cycle_types::CycleCount,
}

#[derive(Clone)]
pub struct SetCoverBEXTrace {
    pub id: usize,
    pub weight: f64,
    pub states: HashMap<
        data_types::general_data_types::StateIdType,
        UnweightedBEXSample,
        data_types::general_data_types::DefaultScalarHasher,
    >,
    pub from_path: ustr::Ustr,
    pub file_source: Option<data_types::general_data_types::WaveFormSource>,
    pub must_not_cover: bool,
}

#[derive(Clone)]
pub struct SetCoverInstance {
    pub predicates: HashMap<usize, SetCoverPredicate>,
    pub bex_samples:
        HashMap<data_types::general_data_types::StateIdType, SetCoverBEXSample, data_types::general_data_types::DefaultScalarHasher>,
    pub cex_traces: HashMap<usize, SetCoverCEXTrace, data_types::general_data_types::DefaultScalarHasher>,
    pub bex_traces: HashMap<usize, SetCoverBEXTrace, data_types::general_data_types::DefaultScalarHasher>,
    pub all_states: HashSet<data_types::general_data_types::StateIdType, data_types::general_data_types::DefaultScalarHasher>,
    pub formula_score_weights: data_types::general_data_types::FormulaScoreWeights,
    // pub merged_partitions: PartitionVec<u64>,
}

impl SetCoverInstance {

    pub fn get_predicate_upper_bound(&self, predicate: &SetCoverPredicate) -> f64 {
        let mut upper_bound = 0.0;
        for trace in self.cex_traces.values() {
            if trace.states.keys().any(|state_id| predicate.covered_states.contains(&(*state_id as u32))) {
                upper_bound += costs::get_cex_weight_in_ilp(
                    self.get_total_cex_weight() as usize,
                    self.get_total_bex_weight() as f64,
                ) * (trace.weight as f64);
            }
        }
        upper_bound -= costs::get_predicate_cost(
            self.get_total_cex_weight() as usize,
            self.get_total_bex_weight(),
            self.formula_score_weights.predicate_base_cost,
        );
        upper_bound
    }

    pub fn new_from_teacher_and_predicates<T>(
        teacher: &teacher::Teacher,
        scored_predicates: &Vec<predicates::BasePredicateWithScoreAndObjective<T>>,
        formula_score_weights: data_types::general_data_types::FormulaScoreWeights,
    ) -> Self
    where
        T: predicates::PredicateLike,
    {
        
        let mut all_states: HashSet<data_types::general_data_types::StateIdType, data_types::general_data_types::DefaultScalarHasher>;
        let set_cover_predicates: HashMap<usize, SetCoverPredicate> = scored_predicates
            .iter()
            .enumerate()
            .map(|(idx, scored_predicate)| {
                (
                    idx,
                    SetCoverPredicate {
                        id: idx,
                        covered_states: scored_predicate.score.cover_info.covered_states.clone(),
                        predicate: scored_predicate.predicate.to_candidate(),
                    },
                )
            })
            .collect();
        let bex_samples: HashMap<
            data_types::general_data_types::StateIdType,
            SetCoverBEXSample,
            data_types::general_data_types::DefaultScalarHasher,
        > = teacher
            .bex_samples
            .par_iter()
            .filter_map(|sample| {
                let must_not_cover;
                if sample.must_not_cover() {
                    must_not_cover = true; // Disallow covering this sample (e.g., if it occurs in cycle 0)
                // log::debug!(
                //     "BEX sample path {} cycle {} must_not_cover",
                //     sample.from_path, sample.and_cycle
                // );
                } else {
                    must_not_cover = false;
                }
                return Some((
                    sample.state_id,
                    SetCoverBEXSample {
                        state_id: sample.state_id,
                        weight: sample.occurrence_count as u64,
                        from_path: sample.from_path.clone(),
                        must_not_cover: must_not_cover,
                    },
                ));

                //TODO: We could also filter out samples that are not covered by any predicate
                //But then we need to remove from the other predicates and cex traces as well
                //which is not implemented yet
                // for p in set_cover_predicates.values() {
                //     if p.covered_states.contains(&(sample.state_id as u32)) {
                //         return Some((sample.state_id,
                //         SetCoverBEXSample {
                //             state_id: sample.state_id,
                //             weight: sample.occurrence_count as u64,
                //             from_path: sample.from_path.clone(),
                //             must_not_cover: must_not_cover,
                //         }));
                //     }
                // }
                //None
            })
            .collect();
        println!(
            "Number of BEX samples after filtering out non-covered: {}, before {}",
            bex_samples.len(),
            teacher.bex_samples.len()
        );
        all_states = bex_samples.keys().cloned().collect();
        let cex_traces: HashMap<usize, SetCoverCEXTrace, data_types::general_data_types::DefaultScalarHasher> = teacher
            .cex_traces
            .iter()
            .enumerate()
            .map(|(idx, trace)| {
                (
                    idx,
                    SetCoverCEXTrace {
                        id: idx,
                        weight: 1,
                        states: trace
                            .contained_samples
                            .iter()
                            .map(|sample| {
                                all_states.insert(sample.state_id);
                                return (
                                    sample.state_id,
                                    SetCoverCEXSample {
                                        state_id: sample.state_id,
                                        cycle: sample.and_cycle as cycle_types::CycleCount,
                                    },
                                );
                            })
                            .collect(),
                        from_path: trace.from_path.clone(),
                        file_source: trace.file_source.clone(),
                    },
                )
            })
            .collect();
        let bex_traces: HashMap<usize, SetCoverBEXTrace, data_types::general_data_types::DefaultScalarHasher> = teacher
            .bex_traces
            .iter()
            .enumerate()
            .map(|(idx, trace)| {
                if trace.weight < 1.0 {
                    // println!("Trace {:?} has weight less than 1.0: {}", trace.from_path, trace.weight);
                }
                (
                    idx,
                    SetCoverBEXTrace {
                        id: idx,
                        weight: trace.weight,
                        states: trace
                            .contained_samples
                            .iter()
                            .map(|sample| {
                                all_states.insert(sample.state_id);
                                return (
                                    sample.state_id,
                                    UnweightedBEXSample {
                                        state_id: sample.state_id,
                                        cycle: sample.and_cycle as cycle_types::CycleCount,
                                    },
                                );
                            })
                            .collect(),
                        from_path: trace.from_path.clone(),
                        file_source: trace.file_source.clone(),
                        must_not_cover: trace.file_source == Some(data_types::general_data_types::WaveFormSource::MustFulfill),
                    },
                )
            })
            .collect();
        //let merged_partitions = PartitionVec::from_iter(all_states.iter().cloned());
        Self {
            predicates: set_cover_predicates,
            bex_samples,
            bex_traces,
            cex_traces,
            all_states,
            formula_score_weights,
        }
    }

    pub fn merge_states(&mut self, state_list: &Vec<u64>) {
        if state_list.len() < 2 {
            return;
        }
        let merge_id = state_list[0];
        for state_id in state_list.iter().skip(1) {
            // Merge the states in bex_samples
            if let Some(sample) = self.bex_samples.remove(&state_id) {
                self.bex_samples.get_mut(&merge_id).unwrap().weight += sample.weight;
                if sample.must_not_cover
                    && self.bex_samples.get_mut(&merge_id).unwrap().must_not_cover == false
                {
                    println!("Warning: Merging a must_not_cover sample into a must_cover sample, keeping must_cover");
                    println!(
                        "Merging path {} onto path {}",
                        sample.from_path,
                        self.bex_samples.get_mut(&merge_id).unwrap().from_path
                    );
                }
                self.bex_samples.get_mut(&merge_id).unwrap().must_not_cover |=
                    sample.must_not_cover;
            } else {
                //This can happen, if we merge a CEX-only state onto a bex states.
                //panic!("State id {} not found in bex_samples, but is in all states? {}", state_id, self.all_states.contains(state_id));
            }
            self.all_states.remove(&state_id);
        }
        // Merge the states in cex_traces
        self.cex_traces.par_iter_mut().for_each(|(_, trace)| {
            //Remove the state_id state from the trace, add the merge_id state
            for state_id in state_list.iter().skip(1) {
                if let Some(sample) = trace.states.remove(state_id) {
                    trace.states.entry(merge_id).or_insert(SetCoverCEXSample {
                        state_id: merge_id,
                        cycle: sample.cycle,
                    });
                }
            }
        });
        // Merge the states in predicates
        self.predicates.par_iter_mut().for_each(|(_, predicate)| {
            for state_id in state_list.iter().skip(1) {
                //if predicate.covered_states.contains(&(*state_id as u32)) {
                predicate.covered_states.remove(*state_id as u32);
                //predicate.covered_states.add(merge_id as u32); //This is not correct, because the merged state might not be covered by this predicate!
                //}
            }
        });
    }

    pub fn calculate_allowed_by_map(&self) -> HashMap<u64, data_types::general_data_types::BitSetWrapper> {
        let mut allowed_by: HashMap<u64, data_types::general_data_types::BitSetWrapper> = HashMap::new();
        for (state_id, _bex_sample) in self.bex_samples.iter() {
            let mut not_covered_by = data_types::general_data_types::BitSetWrapper::new();
            for (_, predicate) in self.predicates.iter() {
                if predicate.covered_states.contains(&(*state_id as u32)) {
                    continue;
                }
                not_covered_by.add(predicate.id as u32);
            }
            allowed_by.insert(*state_id, not_covered_by);
        }
        // for (idx,cex_trace) in self.cex_traces.iter() {
        //     for (state_id, cex_sample) in cex_trace.states.iter(){
        //         allowed_by.remove(state_id);
        //     }
        // }
        allowed_by
    }

    pub fn get_total_cex_weight(&self) -> f64 {
        self.cex_traces.values().map(|trace| trace.weight as f64).sum()
    }

    pub fn get_total_bex_weight(&self) -> f64 {
        if constants::SOLVE_STATES_INSTEAD_OF_TRACES {
            return self.bex_samples.values().map(|sample| sample.weight as f64).sum()
        } else {
            self.bex_traces.values().map(|trace| trace.weight).sum()
        }
        
        // self.bex_samples.values().map(|sample| sample.weight).sum()
    }
}

/// A simple representative map for state merging:
/// `rep[e as usize]` stores the current representative of state `e`.
/// Initialize with identity: rep[e] = e.
#[inline]
fn _init_representatives(max_state_id_inclusive: usize) -> Vec<StateIdType> {
    (0..=max_state_id_inclusive as StateIdType).collect()
}

/// Merge `group` of states onto the first element in `group` (the representative).
/// This does NOT mutate any sets; it only updates the representative mapping.
///
/// Example: merge_states(&mut rep, &[e1, e2, e3]) will set rep[e2]=e1, rep[e3]=e1.
/// (rep[e1] stays e1).
pub fn merge_states_rep(rep: &mut [StateIdType], group: &[StateIdType]) {
    if group.is_empty() {
        return;
    }
    let rep0 = find_rep(rep, group[0]);
    for &s in &group[1..] {
        let rs = find_rep(rep, s);
        if rs != rep0 {
            rep[rs as usize] = rep0;
        }
    }
}

/// Path-compressed find for canonical representative
#[inline]
fn find_rep(rep: &mut [StateIdType], mut x: StateIdType) -> StateIdType {
    // Find root
    let mut r = x;
    while rep[r as usize] != r {
        r = rep[r as usize];
    }
    // Path compression
    while rep[x as usize] != x {
        let p = rep[x as usize];
        rep[x as usize] = r;
        x = p;
    }
    r
}
