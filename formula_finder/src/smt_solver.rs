use core::panic;
use std::collections::HashMap;
use std::f64;
use std::sync::Arc;
use crate::data_types;
use crate::data_types::general_data_types::BitSetWrapper;
use crate::data_types::general_data_types::DefaultScalarHasher;
use crate::predicates::InvariantObjective;
use crate::predicates::PriorityScores;
use crate::waveform;
use crate::predicates;
use crate::teacher;
use crate::solver;
use crate::solver::Solver;
use crate::costs;
use crate::utils;
use rayon::iter::{ParallelIterator, IntoParallelRefIterator};
use ustr;

#[derive(PartialEq)]
pub enum SatResult {
    Sat,
    Unsat,
    Unknown
}

#[derive(Debug)]
pub enum Formula {
    KnownBoolean(bool)
}

impl Formula {
    pub fn is_known_true(&self) -> bool {
        match self {
            Formula::KnownBoolean(truth_value) => *truth_value,
        }
    }

    pub fn is_known_false(&self) -> bool {
        match self {
            Formula::KnownBoolean(truth_value) => !(*truth_value),
        }
    }
}

impl Into<SatResult> for Formula {
    fn into(self) -> SatResult {
        match self {
            Formula::KnownBoolean(truth_value) => {
                if truth_value == true {
                    SatResult::Sat
                } else {
                    SatResult::Unsat
                }
            }
        }
    }
}


pub struct SMTSolver<'a> {
    teacher: &'a teacher::Teacher,
}

pub fn apply_operation(operator: &predicates::Operator, left: i64, right: i64) -> bool {
    match operator {
        predicates::Operator::Equal => left == right,
        predicates::Operator::NotEqual => left != right,
        predicates::Operator::GreaterEqual => left >= right,
        predicates::Operator::SmallerEqual => left <= right,
    }
}

fn evaluate_signal_to_const_formula_and_sample(predicate: &predicates::BaseSignalToConstFormula, sample: &teacher::StateMapping) -> Formula {
    let predicate_value = predicate.get_value();
    let signal_value = if let Some(signal_idx) = predicate.signal_idx {
        sample.get_signal_value_from_index(&signal_idx)
    } else {
        sample.get_signal_value_from_string(&predicate.signal_name)
    };
    return Formula::KnownBoolean(apply_operation(&predicate.operator, signal_value, predicate_value));
}

fn evaluate_signal_to_const_set_formula_and_sample(
    predicate: &predicates::BaseSignalToConstSetFormula,
    sample: &teacher::StateMapping,
    not_in: bool,
) -> Formula {
    let signal_value = if let Some(signal_idx) = predicate.signal_idx {
        sample.get_signal_value_from_index(&signal_idx)
    } else {
        sample.get_signal_value_from_string(&predicate.signal_name)
    };
    let is_in_set = predicate.get_values().iter().any(|value| *value == signal_value);
    let result = if not_in { !is_in_set } else { is_in_set };
    Formula::KnownBoolean(result)
}

pub fn evaluate_two_signal_formula(predicate: &predicates::BaseFormulaTwoSignalCompare, sample: &teacher::StateMapping) -> Formula {
    let signal_value_1 = if let Some(signal_idx) = predicate.signal_idx1 {
        sample.get_signal_value_from_index(&signal_idx)
    } else {
        sample.get_signal_value_from_string(&predicate.signal_name1)
    };
    let signal_value_2 = if let Some(signal_idx) = predicate.signal_idx2 {
        sample.get_signal_value_from_index(&signal_idx)
    } else {
        sample.get_signal_value_from_string(&predicate.signal_name2)
    };
    // println!("Evaluating two signal formula: {} {} {} sample {:?}", signal_value_1, predicate.operation, signal_value_2, sample);
    return Formula::KnownBoolean(apply_operation(&predicate.operation, signal_value_1, signal_value_2));
}

fn construct_z3_formula_predicate_and_sample(predicate: & predicates::BasePredicate, sample: &teacher::StateMapping ) -> Formula {
    match predicate.base_formula {
        predicates::BaseFormula::SignalToConst(ref signal_to_const_formula) => {
            return evaluate_signal_to_const_formula_and_sample(signal_to_const_formula, sample);
        },
        predicates::BaseFormula::ValueNotIn(ref signal_to_const_set_formula) => {
            return evaluate_signal_to_const_set_formula_and_sample(signal_to_const_set_formula, sample, true);
        },
        predicates::BaseFormula::ValueIn(ref signal_to_const_set_formula) => {
            return evaluate_signal_to_const_set_formula_and_sample(signal_to_const_set_formula, sample, false);
        },
        predicates::BaseFormula::TwoSignalEqual(ref two_signal_formula) => {
            return evaluate_two_signal_formula(two_signal_formula, sample);
        }
    }
}

fn construct_z3_formula_for_sample_and_invariant(sample: &teacher::StateMapping, invariant: &predicates::Invariant) -> Formula {
    for predicate in invariant.predicate_set.predicates.iter() {
        let formula = construct_z3_formula_predicate_and_sample(predicate, sample);
        if formula.is_known_false() {
            return Formula::KnownBoolean(false);
        }
    }
    return Formula::KnownBoolean(true);
}

pub fn construct_z3_formula_for_bex_sample_and_invariant(sample: &teacher::StateMapping, invariant: &predicates::Invariant) -> Formula {
    let this_formula= construct_z3_formula_for_sample_and_invariant(sample, invariant);
    match this_formula {
        Formula::KnownBoolean(x) => {
            return Formula::KnownBoolean(!x);
        }
    }
}

fn construct_z3_formula_for_maybesample_and_invariant<'ctx>(maybe_sample: &teacher::CexTrace, invariant: &'ctx predicates::Invariant) -> Formula {
    for sample in maybe_sample.contained_samples.iter() {
        let state_mapping = sample.state_pointer.upgrade().unwrap();
        let this_cycle_formula = construct_z3_formula_for_sample_and_invariant(&state_mapping,invariant);
        match this_cycle_formula {
            Formula::KnownBoolean(truth_val) => {
                if truth_val == true { 
                    return Formula::KnownBoolean(true);
                }
            }
        }
        
    }
    return Formula::KnownBoolean(false);
}

fn evaluate_invariant_disjunction_on_maybe_sample(maybe_sample: &teacher::CexTrace, invariant_disjunction: &predicates::InvariantDisjunction) -> Formula {
    for invariant in invariant_disjunction.disjunctions.iter() {
        let formula = construct_z3_formula_for_maybesample_and_invariant(maybe_sample, invariant);
        if formula.is_known_true() {
            return Formula::KnownBoolean(true);
        }
    }
    return Formula::KnownBoolean(false);
}

pub fn evaluate_invariant_disjunction_on_sample(sample: &teacher::StateMapping, invariant_disjunction: &predicates::InvariantDisjunction) -> Formula {
    for invariant in invariant_disjunction.disjunctions.iter() {
        let formula = construct_z3_formula_for_sample_and_invariant(sample, invariant);
        if formula.is_known_true() {
            return Formula::KnownBoolean(true);
        }
    }
    return Formula::KnownBoolean(false);
}

fn _evaluate_invariant_disjunction_on_bex_sample(bex_sample: &teacher::StateMapping, invariant_disjunction: &predicates::InvariantDisjunction) -> Formula {
    for invariant in invariant_disjunction.disjunctions.iter() {
        let formula = construct_z3_formula_for_bex_sample_and_invariant(bex_sample, invariant);
        if formula.is_known_false() {
            return Formula::KnownBoolean(false);
        }
    }
    return Formula::KnownBoolean(true);
}

pub fn evaluate_separator_formula_on_maybe_sample(maybe_sample: &teacher::CexTrace, separator_formula: &predicates::SeparatorFormula) -> Formula {
    match separator_formula {
        predicates::SeparatorFormula::InvariantDisjunction(ref invariant_disjunction) => {
            return evaluate_invariant_disjunction_on_maybe_sample(maybe_sample, invariant_disjunction);
        },
        predicates::SeparatorFormula::Invariant(ref invariant) => {
            return construct_z3_formula_for_maybesample_and_invariant(maybe_sample, invariant);
        }
    }
}

pub fn evaluate_separator_formula_on_sample(sample: &teacher::StateMapping, separator_formula: &predicates::SeparatorFormula) -> Formula {
    match separator_formula {
        predicates::SeparatorFormula::InvariantDisjunction(ref invariant_disjunction) => {
            return evaluate_invariant_disjunction_on_sample(sample, invariant_disjunction);
        },
        predicates::SeparatorFormula::Invariant(ref invariant) => {
            return construct_z3_formula_for_sample_and_invariant(sample, invariant);
        }
    }
}


///
/// Short_circuit: If true, return as soon we know that the waveform fulfills the invariant - exact cycle does not matter.
pub fn check_for_waveform(waveform: &waveform::WaveForm, separator_formula: &predicates::SeparatorFormula, short_circuit: bool, fill_invariants_idx:bool) -> Option<Vec<u64>> {
    let invariant = if fill_invariants_idx{
        let mut invariant = separator_formula.clone();
        teacher::fill_in_indexes_for_formula_and_waveform(&mut invariant, waveform).unwrap();
        invariant 
    } else {
        separator_formula.clone()
    };

    //println!("values_for_predicates {:?}", values_for_predicates);
    //println!("Checking waveform for invariant {}", invariant);

    //println!("Predicate hashmap {:?}", predicate_hashmap);
    // let mut samples: Vec<teacher::ContainedState> = Vec::new();
    // let mut state_vector = Vec::new(); //We need this state vector to not deallocate the arcs
    let mut fulfilled_cycles: Vec<u64> = Vec::new();
    let string_to_id: HashMap<ustr::Ustr, u64> = HashMap::new();
    let string_to_id_arc = Arc::new(string_to_id);

    // Pre-calculate signal mapping
    let relevant_signals: Vec<u64> = invariant.get_relevant_signal_idx().into_iter().collect();
    let mut id_to_idx_map = HashMap::with_capacity_and_hasher(relevant_signals.len(), DefaultScalarHasher::default());
    for (i, &id) in relevant_signals.iter().enumerate() {
        id_to_idx_map.insert(id, i);
    }
    let signal_id_to_index = Arc::new(id_to_idx_map);

    for cycle in 0..waveform.num_cycles {
        let mut signal_values = Vec::with_capacity(relevant_signals.len());
        for signal_idx in &relevant_signals {
            let value = waveform.get_signal_value_at_cycle_from_id(signal_idx, &cycle).unwrap();
            signal_values.push(value);
        }

        let state = teacher::StateMapping {
            signal_values,
            signal_id_to_index: Arc::clone(&signal_id_to_index),
            string_to_id: string_to_id_arc.clone(),
            state_id: cycle as u64
        };
        // let state = Arc::new(state);
        // state_vector.push(state.clone());
        // samples.push(teacher::ContainedState{
        //     state_id: cycle as u64,
        //     and_cycle: cycle as u64,
        //     state_pointer: Arc::downgrade(&state)
        // });
        let evaluate_this_sample = evaluate_separator_formula_on_sample(&state, &invariant);
        if evaluate_this_sample.is_known_true() {
            if short_circuit {
                return Some(vec![0]);
            }
            fulfilled_cycles.push(cycle as u64);
            println!("Cycle {} of waveform {} fulfills invariant {}", cycle, waveform.path, invariant);
            println!("The sample is {:?}", state);
        }
    }
    if fulfilled_cycles.len() > 0 {
        return Some(fulfilled_cycles);
    } else {
        return None;
    }
    // let maybe_sample = teacher::CexTrace{contained_samples: samples, from_path: waveform.path.clone(), ..Default::default()};
    // println!("Samples");
    // maybe_sample.print();
    //println!("Invariant {:?}", invariant);
    // let formula = evaluate_separator_formula_on_maybe_sample(&maybe_sample, &invariant);
    // // let short_circuit = false;
    // //println!("Formula {:?}", formula);
    // if formula.is_known_true() {
    //     if short_circuit {
    //         return Some(vec![0]);
    //     }
    //     let mut fulfilled_cycles = Vec::new();
    //     for sample in maybe_sample.contained_samples.iter() {
    //         //println!("Predicate hashmap {:?}", values_for_predicates);
    //         let state_mapping = sample.state_pointer.upgrade().unwrap();
    //         let this_cycle_formula = evaluate_separator_formula_on_sample(&state_mapping,&invariant);
    //         let res_this_cycle = this_cycle_formula.is_known_true();
    //         if res_this_cycle == true {
    //             //TODO: Note that this cycle is not the COUNTER value!!!
    //             println!("Cycle {} of waveform {} fulfills invariant {}", sample.and_cycle, waveform.path, invariant);
    //             println!("The sample is {:?}", sample);
    //             fulfilled_cycles.push(sample.and_cycle);
    //         }
    //     }
    //     //println!("Fulfilled cycles {:?} invariant {:?} formatted invariant {}", fulfilled_cycles, invariant, invariant);
    //     return Some(fulfilled_cycles);
    // }
    // None
}

pub fn get_gini_coefficient_from_predicate_list(base_predicate_list: &Vec<predicates::InvariantWithScoreAndObjective>, teacher: &teacher::Teacher, base_invariant: Option<&predicates::InvariantWithScoreAndObjective>, formula_score_weights: &data_types::general_data_types::FormulaScoreWeights) -> Vec<(predicates::InvariantWithScoreAndObjective, f64)> {
    let solver_instance = SMTSolver::new_from_teacher(&teacher);
    let ret_vec: Vec<(predicates::InvariantWithScoreAndObjective, f64)> = base_predicate_list.par_iter().map(|base_predicate| {
        let invariant = match base_invariant {
            Some(base_invariant) => { solver_instance.merge_invariant_and_score_from_with_objective(&base_invariant, base_predicate, formula_score_weights)},
            None => base_predicate.clone()
        };
        let gini_coefficient = solver_instance.calculate_gini_coefficient_for_scored_invariant(&invariant); 
        return Some((base_predicate.clone(), gini_coefficient));
    }).filter(|x| x.is_some()).map(|x| x.unwrap()).collect();
    ret_vec

}


pub fn score_list_of_predicates_with_fulfilled_examples(base_predicate_list: &Vec<predicates::BasePredicate>, teacher: &teacher::Teacher) -> Vec<(predicates::BasePredicate, predicates::ScoreAndFulfilledExample)> {
    let solver_instance = SMTSolver::new_from_teacher(&teacher);
    let ret_vec: Vec<(predicates::BasePredicate, predicates::ScoreAndFulfilledExample)> = base_predicate_list.par_iter().map(|base_predicate| {
        let mut invariant = predicates::Invariant::new();
        invariant.add_predicate(base_predicate.clone());
        let invariant_score: predicates::ScoreAndFulfilledExample = solver_instance.score_invariant_with_fulfilled_examples(&invariant);
        return Some((base_predicate.clone(), invariant_score));
    }).filter(|x| x.is_some()).map(|x| x.unwrap()).collect();
    ret_vec
}


pub fn score_predicate_list(base_predicate_list: &Vec<predicates::BasePredicate>, teacher: &teacher::Teacher, base_invariant: Option<&predicates::Invariant>) -> Vec<(predicates::BasePredicate, predicates::PriorityScores)> {
    let solver_instance = SMTSolver::new_from_teacher(&teacher);
    let ret_vec:Vec<(predicates::BasePredicate,predicates::PriorityScores)> = base_predicate_list.par_iter().map(|base_predicate| {
        let mut invariant = match base_invariant {
            Some(base_invariant) => base_invariant.clone(),
            None => predicates::Invariant::new(),
        };
        invariant.add_predicate(base_predicate.clone());
        let invariant_score: predicates::PriorityScores = solver_instance.score_invariant(&invariant);
        return Some((base_predicate.clone(), invariant_score));
    }).filter(|x| x.is_some()).map(|x| x.unwrap()).collect();
    
        //.map(|(base_formula, group)| {
        //    let scores: Vec<(Option<i64>, predicates::PriorityScores)> = group.map(|(_, value, score)| (value, score)).collect();
        //    (base_formula, scores)
        //})
        //.collect();
    //println!("\n ### Grouped res list {:?}", grouped_res_list);
    //for (base_formula, scores) in &grouped_res_list {
    //    println!("{:?}: {:?}", base_formula, scores);
    //}
    //panic!("Done scoring");
    ret_vec
} 


pub fn calculate_examples_split_from_scored_invariant(invariant: &predicates::InvariantWithScoreAndObjective, teacher: &teacher::Teacher) -> (teacher::Teacher, teacher::Teacher, teacher::Teacher) {
        let mut remaining_cex = Vec::new(); //Cex classified as allowed
        let mut remaining_bex = Vec::new(); //Bex classified as blocked
        let mut left_side_cex = Vec::new(); //Cex classified as "positive" (blocked)
        let mut left_side_bex = Vec::new(); //Bex classified as positive (blocked)
        let mut right_side_cex = Vec::new(); //Cex classified as negative (allowed)
        let mut right_side_bex = Vec::new(); //Bex classified as negative (allowed)
        for (_, maybe_sample) in teacher.cex_traces.iter().enumerate() {
            let cex_covered = maybe_sample.is_covered_by_blocked_states(&invariant.score.cover_info.covered_states);
            if cex_covered {
                left_side_cex.push(maybe_sample.clone());
                remaining_cex.push(maybe_sample.clone());
            } else {
                right_side_cex.push(maybe_sample.clone());
                //if maybe_sample.file_source == Some(data_types::WaveFormSource::OriginalCex) {
                //    remaining_cex.push(maybe_sample.clone());
                //}
                remaining_cex.push(maybe_sample.clone()); //We always want to maximize the number of CEX that are "still fulfilled"
            }
        }
        for (_, sample) in teacher.bex_samples.iter().enumerate() {
            let bex_covered = invariant.score.cover_info.covered_states.contains(&(sample.state_id as u32));
            if !(bex_covered) { //Bex is allowed
                right_side_bex.push(sample.clone());
            } else { //Bex is not allowed (since it is covered)
                // println!("Remaing bex path {} and cycle {}", sample.from_path, sample.and_cycle);
                left_side_bex.push(sample.clone());
                remaining_bex.push(sample.clone());
            }
        }
        //println!("Counting bex done for invariant {:?}", invariant);
        //println!("Score invariant {:?}", self.score_invariant(invariant, None, true));
        let remaining_teacher = teacher::new_from_samples(remaining_cex, remaining_bex, &teacher);
        let lhs_teacher = teacher::new_from_samples(left_side_cex, left_side_bex, &teacher);
        let rhs_teacher = teacher::new_from_samples(right_side_cex, right_side_bex, &teacher);
        (remaining_teacher, lhs_teacher, rhs_teacher)
    }

impl<'a> SMTSolver<'a> {

    
    pub fn get_fulfilled_cex_samples(&self, invariant: &predicates::Invariant) -> Vec<u32> {
        let mut result_vec = Vec::new();
        for cex in self.teacher.cex_traces.iter() {
            for sample in cex.contained_samples.iter() {
                let state_mapping = sample.state_pointer.upgrade().unwrap();
                let formula = construct_z3_formula_for_sample_and_invariant(&state_mapping, invariant);
                match formula {
                    Formula::KnownBoolean(truth_val) => {
                        match truth_val {
                            true => {
                                result_vec.push(sample.state_id as u32);
                            },
                            false => {}
                        }
                    }
                }
            }
        }
        return result_vec;
    }

    pub fn get_covered_states_from_formula(&self, separator_formula: &predicates::SeparatorFormula) -> BitSetWrapper {
        let mut covered_states = BitSetWrapper::new();
        for (state_id, state_mapping) in self.teacher.states.iter() {
            let formula = evaluate_separator_formula_on_sample(&state_mapping, separator_formula);
            match formula {
                Formula::KnownBoolean(truth_val) => {
                    if truth_val == true {
                        covered_states.add(*state_id as u32);
                    }
                }
            }
        }
        covered_states
    }

    pub fn get_covered_states_disjunction(&self, invariant_disjunction: &predicates::InvariantDisjunction) -> BitSetWrapper {
        let mut covered_states = BitSetWrapper::new();
        for (state_id, state_mapping) in self.teacher.states.iter() {
            let formula = evaluate_invariant_disjunction_on_sample(&state_mapping, invariant_disjunction);
            match formula {
                Formula::KnownBoolean(truth_val) => {
                    if truth_val == true {
                        covered_states.add(*state_id as u32);
                    }
                }
            }
        }
        covered_states
    }

    pub fn get_covered_states(&self, invariant: &predicates::Invariant) -> BitSetWrapper {
        let mut covered_states = BitSetWrapper::new();
        for (state_id, state_mapping) in self.teacher.states.iter() {
            let formula = construct_z3_formula_for_sample_and_invariant(&state_mapping, invariant);
            match formula {
                Formula::KnownBoolean(truth_val) => {
                    if truth_val == true {
                        covered_states.add(*state_id as u32);
                    }
                }
            }
        }
        covered_states
    }

    pub fn calculate_examples_split(&self, invariant: &predicates::Invariant) -> (teacher::Teacher, teacher::Teacher, teacher::Teacher) {
        let covered_states = self.get_covered_states(invariant);
        let mut remaining_cex = Vec::new(); //Cex classified as allowed
        let mut remaining_bex = Vec::new(); //Bex classified as blocked
        let mut left_side_cex = Vec::new(); //Cex classified as "positive" (blocked)
        let mut left_side_bex = Vec::new(); //Bex classified as positive (blocked)
        let mut right_side_cex = Vec::new(); //Cex classified as negative (allowed)
        let mut right_side_bex = Vec::new(); //Bex classified as negative (allowed)
        for (_idx, maybe_sample) in self.teacher.cex_traces.iter().enumerate() {
            let cex_covered = maybe_sample.is_covered_by_blocked_states(&covered_states);
            if cex_covered {
                left_side_cex.push(maybe_sample.clone());
                remaining_cex.push(maybe_sample.clone());
            } else {
                right_side_cex.push(maybe_sample.clone());
                remaining_cex.push(maybe_sample.clone());
            }
        }
        for (_idx, bex_trace) in self.teacher.bex_traces.iter().enumerate() {
            let bex_covered =  bex_trace.is_covered_by_blocked_states(&covered_states);
            if !(bex_covered) { //Bex is allowed
                right_side_bex.push(bex_trace.clone());
            } else {
                left_side_bex.push(bex_trace.clone());
                remaining_bex.push(bex_trace.clone());
            }
        }
        //println!("Counting bex done for invariant {:?}", invariant);
        //println!("Score invariant {:?}", self.score_invariant(invariant, None, true));
        let remaining_teacher = teacher::new_from_traces(remaining_cex, remaining_bex, &self.teacher);
        let lhs_teacher = teacher::new_from_traces(left_side_cex, left_side_bex, &self.teacher);
        let rhs_teacher = teacher::new_from_traces(right_side_cex, right_side_bex, &self.teacher);
        (remaining_teacher, lhs_teacher, rhs_teacher)
    }   

    fn calculate_gini_coefficient_for_scored_invariant(&self, invariant: &predicates::InvariantWithScoreAndObjective) -> f64 {
        let mut left_side_cex_count = 0; //Cex classified as "positive" (blocked)
        let mut left_side_bex_count = 0; //Bex classified as positive (blocked)
        let mut right_side_cex_count = 0; //Cex classifed as negative (allowed)
        let mut right_side_bex_count = 0; //Bex classified as negative (allowed)
        let mut original_cex_misclassified = false;
        for cex_trace in self.teacher.cex_traces.iter() {
            if cex_trace.is_covered_by_blocked_states(&invariant.score.cover_info.covered_states) {
                left_side_cex_count += 1;
            } else {
                if cex_trace.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                    //println!("Original cex misclassified");
                    original_cex_misclassified = true;
                    break;
                }
                right_side_cex_count += 1;
            }
        }
        if original_cex_misclassified {
            //println!("Original cex misclassified");
            return 1.0;
        }
        // for  bex_sample in self.teacher.bex_samples.iter() {
        //     if !(invariant.score.cover_info.covered_states.contains(&(bex_sample.state_id as u32))) {
        //         right_side_bex_count += 1;
        //     } else {
        //         left_side_bex_count += 1;
        //     }
        // }
        for bex_trace in self.teacher.bex_traces.iter() {
            let bex_covered = bex_trace.is_covered_by_blocked_states(&invariant.score.cover_info.covered_states);
            if !(bex_covered) { //is allowed
                right_side_bex_count += 1;
            } else { //is blocked
                left_side_bex_count += 1;
            }
        }
        let impurity_lhs = utils::get_node_gini_impurity(left_side_cex_count, left_side_bex_count);
        let impurity_rhs = utils::get_node_gini_impurity(right_side_cex_count, right_side_bex_count);
        let total_lhs = left_side_cex_count + left_side_bex_count;
        let total_rhs = right_side_cex_count + right_side_bex_count;
        let gini_coeff_this_split = (impurity_lhs * total_lhs as f64 + impurity_rhs * total_rhs as f64) / (total_lhs as f64 + total_rhs as f64);
        gini_coeff_this_split

    }

    fn _calculate_gini_coefficient(&self, invariant: &predicates::Invariant) -> f64 {
        //let (remaining_teacher, lhs_teacher, rhs_teacher) = self.calculate_examples_split( invariant);
        let covered_states = self.get_covered_states(invariant);
        let mut left_side_cex_count = 0; //Cex classified as "positive" (blocked)
        let mut left_side_bex_count = 0; //Bex classified as positive (blocked)
        let mut right_side_cex_count = 0; //Cex classifed as negative (allowed)
        let mut right_side_bex_count = 0; //Bex classified as negative (allowed)
        let mut original_cex_misclassified = false;
        for cex_trace in self.teacher.cex_traces.iter() {
            let cex_covered = cex_trace.is_covered_by_blocked_states(&covered_states);
            if cex_covered { //is allowed
                left_side_cex_count += 1;
            } else {
                if cex_trace.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                    //println!("Original cex misclassified");
                    original_cex_misclassified = true;
                    break;
                }
                right_side_cex_count += 1;
            }
        }
        if original_cex_misclassified {
            //println!("Original cex misclassified");
            return 1.0;
        }
        for  bex_sample in self.teacher.bex_samples.iter() {
            if !(covered_states.contains(&(bex_sample.state_id as u32))) {
                right_side_bex_count += 1;
            } else { //is blocked
                left_side_bex_count += 1;
            }
        }
        //println!("Calculating split done for invariant {:?}", invariant);
        //let impurity_lhs = lhs_teacher.calculate_gini_impurity();
        //let impurity_rhs = rhs_teacher.calculate_gini_impurity();
        //let total_lhs = lhs_teacher.cex_samples.len() as f64 + lhs_teacher.bex_samples.len() as f64;
        //let total_rhs = rhs_teacher.cex_samples.len() as f64 + rhs_teacher.bex_samples.len() as f64;
        //let gini_coeff_this_split = (impurity_lhs * total_lhs + impurity_rhs * total_rhs) / (total_lhs + total_rhs);
        let impurity_lhs = utils::get_node_gini_impurity(left_side_cex_count, left_side_bex_count);
        let impurity_rhs = utils::get_node_gini_impurity(right_side_cex_count, right_side_bex_count);
        let total_lhs = left_side_cex_count + left_side_bex_count;
        let total_rhs = right_side_cex_count + right_side_bex_count;
        let gini_coeff_this_split = (impurity_lhs * total_lhs as f64 + impurity_rhs * total_rhs as f64) / (total_lhs as f64 + total_rhs as f64);
        gini_coeff_this_split
    }

    pub fn invariant_upper_bound(&self, invariant: &predicates::Invariant, cover_info: &predicates::CoverInformation,
        formula_score_weight: & data_types::general_data_types::FormulaScoreWeights
    ) -> f64 {
        // println!("Calculating upper bound for invariant {}", invariant.invariant);
        let covered_states = cover_info.covered_states.clone();
        //We know which cex are blocked, we can only block less after
        //But: We can potentially allow more bex samples
        let all_bex_states =  data_types::general_data_types::BitSetWrapper::from_vec(self.teacher.bex_samples.iter().map(|x| x.state_id as u32).collect());
        let _intersection_bex_and_cex_states = all_bex_states.intersection(&cover_info.blocked_cex_states);
        let mut intersection_must_be_blocked =  data_types::general_data_types::BitSetWrapper::new();
        //How should we handle an intersection? There are two cases:
        //1. We will keep that state as blocked, but then we will loose the bex
        //2. We will allow that state, but then we will loose the cex state
        //- this is guaranteed, since it is an intersection!
        //But: There might be other cex states that are blocked which will state block the traces
        //What is the upper bound? For each state, we make the decision that is optimal
        //This has additional complexity, since this decision is not independent per state
        //Probably the best is to say to:
        //For each cex trace
        //If all its state are in the intersection, keep the states tat would lead to best outcome
        //[..]
        //Or: Just say maximum is maximum of blocked cex traces + #number of total bex traces *weight
        //
        let total_num_cex = self.teacher.cex_traces.len();
        let total_bex_weight = self.teacher.get_total_bex_weight();
        // let bex_weight = costs::get_bex_weight_in_ilp(total_num_cex, total_num_bex, formula_score_weight.bex_multiplier);
        let cex_weight = costs::get_cex_weight_in_ilp(total_num_cex, total_bex_weight);
        //println!("Total num cex {}, total num bex {},
        let mut score = 0.0; 
        let mut count_per_shared_bex_and_cex_state = HashMap::new();
        
        for cex_trace in self.teacher.cex_traces.iter() {
            if cex_trace.is_covered_by_blocked_states(&covered_states) {
                let intersection_with_this_trace = cex_trace.get_intersection_with_states(&cover_info.blocked_cex_states);
                if intersection_with_this_trace.len() == 1 {
                    let state = intersection_with_this_trace.return_first_element().unwrap();
                    if cex_trace.file_source == Some( data_types::general_data_types::WaveFormSource::OriginalCex) {
                        //println!("Original cex trace is blocked
                        
                        intersection_must_be_blocked.insert_all(intersection_with_this_trace.collect().iter().map(|x| *x as u32));
                        score += cex_weight;
                    } else {
                        //We only have one state to block this cex
                        let intersection_with_bex_states = intersection_with_this_trace.intersection(&all_bex_states);
                        if intersection_with_bex_states.len() == 1 {
                            count_per_shared_bex_and_cex_state.entry(state).and_modify(|e| *e += 1).or_insert(1);
                            // if let Some(bex_sample) = self.teacher.bex_samples.iter().find(|bex| intersection_with_bex_states.contains(&(bex.state_id as u32))) {
                            //     let this_bex_weight = bex_sample.occurrence_count as f64 * bex_weight;
                            //     if (this_bex_weight > cex_weight) || bex_sample.and_cycle==0 {
                            //         //This one state is also a bex - and the bex weight is higher than the cex weight
                            //         //Then our best bet is to not block this cex trace, but instead allow the bex
                            //         println!("Not counting cex intersection state {} to be blocked this_bex_weight {} cex_weight {}", bex_sample.state_id, this_bex_weight, cex_weight);
                            //         continue;
                            //     } else {
                            //         //Otherwise we either can or want to block this cex
                            //         println!("Adding intersection state {} to be blocked this_bex_weight {} cex_weight {}", bex_sample.state_id, this_bex_weight, cex_weight);
                            //         intersection_must_be_blocked.insert_all(intersection_with_this_trace.collect().iter().map(|x| *x as u32));
                            //         score += costs::get_cex_weight_in_ilp(total_num_cex, total_num_bex);
                            //     }
                            // }
                        } else {
                            score += costs::get_cex_weight_in_ilp(total_num_cex, total_bex_weight);
                        }
                    }
                } else {
                    score += costs::get_cex_weight_in_ilp(total_num_cex, total_bex_weight);
                }
            } else if cex_trace.file_source == Some( data_types::general_data_types::WaveFormSource::OriginalCex) {
                return f64::MIN;
            }
        }
        let mut num_still_covered_bex_states =0;
        for trace in self.teacher.bex_traces.iter() {
            if trace.is_covered_by_blocked_states(&intersection_must_be_blocked) {
                score -= costs::get_bex_weight_in_ilp(total_num_cex, total_bex_weight, formula_score_weight.bex_multiplier) * (trace.contained_samples.len() as f64);
                if trace.must_not_cover() {
                    return f64::MIN;
                }
            }
            if trace.is_covered_by_blocked_states(&covered_states) {
                num_still_covered_bex_states += 1;
            }
        }
//         for (state, count) in count_per_shared_bex_and_cex_state.iter() {
//             //println!("State {} is shared by {} cex traces", state, count);
//             let this_cex_weight = *count as f64 * cex_weight;
//             let bex_sample = self.teacher.bex_samples.iter().find(|bex| bex.state_id as u32 == *state);
//             if let Some(bex_sample) = bex_sample {
//                 let this_bex_weight = bex_sample.occurrence_count as f64 * bex_weight;
//                 if (this_bex_weight > this_cex_weight) || bex_sample.must_not_cover() {
//                     //This one state is also a bex - and the bex weight is higher than the cex weight
//                     //Then our best bet is to not block this cex trace, but instead allow the bex
//                     //println!("Not counting cex intersection state {} to be blocked this_bex_weight {} cex_weight {}", bex_sample.state_id, this_bex_weight, lost_cex);
//                     continue;
//                 } else {
//                     //Otherwise we either can or want to block this cex
//                     //println!("Adding intersection state {} to be blocked this_bex_weight {} cex_weight {}", bex_sample.state_id, this_bex_weight, this_cex_weight);
//                     intersection_must_be_blocked.insert(*state);
//                     score += this_cex_weight;
//                 }
//             } else {
//                 //println!("State {} is not a bex sample, but shared by {} cex traces", state, count);
//                 unreachable!("State {} is not a bex sample, but shared by {} cex traces", state, count);
//             }
//         }
//         for (idx, bex) in self.teacher.bex_samples.iter().enumerate() {
// //            if (covered_states.contains(&(bex.state_id as u32)) == false) {
//                 if intersection_must_be_blocked.contains(&(bex.state_id as u32)) {
//                     if bex.must_not_cover() {
//                         return 0.0; //We do not want to block bex cycle 0 as to not block reset states
//                     }
//                 } else {
//                     //println!("BEX state {} is allowed", bex.state_id);
//                     score += costs::get_bex_weight_in_ilp(total_num_cex, total_num_bex, formula_score_weight.bex_multiplier)*bex.occurrence_count as f64;
//                 }
//   //          }
//         }
        for _ in invariant.predicate_set.predicates.iter() {
            score -= costs::get_predicate_cost( total_num_cex, total_bex_weight, formula_score_weight.predicate_base_cost);
        }
        if num_still_covered_bex_states >0 { // Need at least one more predicate to block the remaining bex states
            score -= costs::get_predicate_cost( total_num_cex, total_bex_weight, formula_score_weight.predicate_base_cost);
        }
        score
            /*
        covered_states.remove_all(self.teacher.bex_samples.iter().map(|x| x.state_id as u32));
        let obj1 = self.calculate_invariant_objective_from_covered_states(&invariant.invariant, &covered_states);
        covered_states.insert_all(invariant.score.blocked_cex_states.collect().iter().map(|x| *x as u32));
        let obj2 = self.calculate_invariant_objective_from_covered_states(&invariant.invariant, &covered_states);
        println!("Upper bound for invariant {} is {} and {} intersection cex and bex {}", invariant.invariant, obj1, obj2, intersection_size);
        let max_obj = obj1.max(obj2);
        max_obj
            */
    }

    pub fn calculate_invariant_objective_from_scored_invariant(&self, invariant: &predicates::ScoredInvariantWithFulfilledExample, allow_must_fullfill_bex_not_covered: bool, formula_score_weights: & data_types::general_data_types::FormulaScoreWeights) -> InvariantObjective {
        let covered_states = &invariant.score.cover_info.covered_states;
        let score= self.calculate_invariant_objective_from_covered_states(&invariant.invariant, covered_states, allow_must_fullfill_bex_not_covered, formula_score_weights, false);
        predicates::InvariantObjective {
            objective: score
        }
    }

    pub fn calculate_objective_from_covered_states(&self, 
        covered_states: &BitSetWrapper, allow_must_fullfill_bex_not_covered: bool, formula_score_weights: & data_types::general_data_types::FormulaScoreWeights,
        _verbose: bool) -> f64 {
        let total_num_cex = self.teacher.cex_traces.len();
        let total_num_bex_weight = self.teacher.get_total_bex_weight();
        let bex_weight_per_sample = costs::get_bex_weight_in_ilp(total_num_cex, total_num_bex_weight, formula_score_weights.bex_multiplier);
        let cex_weight_per_trace = costs::get_cex_weight_in_ilp(total_num_cex, total_num_bex_weight);
        // println!("Objective BEX weight per sample {}, len {}, CEX weight per trace {}", bex_weight_per_sample,total_num_bex_weight, cex_weight_per_trace);
        let mut score = 0.0;
         for cex_trace in self.teacher.cex_traces.iter() {
            if cex_trace.is_covered_by_blocked_states(&covered_states) {
                score += cex_weight_per_trace;
            } else if cex_trace.file_source == Some( data_types::general_data_types::WaveFormSource::OriginalCex) {
                return f64::MIN;
            }
        }
        // println!("Max possible objective from CEX: {}", max_possible_objective);
        // #[allow(unused_assignments)]
        // let mut bex_loss = 0.0;
        // for (idx, bex) in self.teacher.bex_samples.iter().enumerate() {
        //     if (covered_states.contains(&(bex.state_id as u32)) == true) {
        //         score -= bex_weight_per_sample * bex.occurrence_count as f64;
        //         bex_loss -= bex_weight_per_sample * bex.occurrence_count as f64;
        //         if !allow_must_fullfill_bex_not_covered && bex.must_not_cover() {
        //             return 0.0; //We do not want to allow bex cycle 0 as to not block reset states
        //         }
        //     }
        //     //We only want to substract if we cover
        //     // score += bex_weight_per_sample * bex.occurrence_count as f64; //We always want to allow bex
        // }
        for (_idx, bex_trace) in self.teacher.bex_traces.iter().enumerate() {
            if bex_trace.is_covered_by_blocked_states(&covered_states) {
                if !allow_must_fullfill_bex_not_covered && bex_trace.must_not_cover() {
                        return f64::MIN; //We do not want to allow bex cycle 0 as to not block reset states
                    }
                    let bex_occurrence_count = bex_trace.weight;
                    score -= bex_weight_per_sample * bex_occurrence_count as f64;
                    // bex_loss -= bex_weight_per_sample * bex_occurrence_count as f64;
            }
        }
        return score;
    }

    pub fn calculate_invariant_objective_from_covered_states(&self, invariant: &predicates::Invariant, covered_states: &BitSetWrapper, allow_must_fullfill_bex_not_covered: bool,
      formula_score_weights: &data_types::general_data_types::FormulaScoreWeights,
      verbose: bool
    ) -> f64{
        if verbose {
            println!("Calculating objective for invariant {} from covered states {:?}", invariant, covered_states);
        }
        let total_num_cex = self.teacher.cex_traces.len();
        let total_num_bex = self.teacher.get_total_bex_weight();
        // println!("Objective BEX weight per sample {}, len {}, CEX weight per trace {}", bex_weight_per_sample,total_num_bex, cex_weight_per_trace);
        let mut score = self.calculate_objective_from_covered_states(covered_states, allow_must_fullfill_bex_not_covered, formula_score_weights, verbose);
        for _ in invariant.predicate_set.predicates.iter() {
            score -= costs::get_predicate_cost( total_num_cex, total_num_bex,formula_score_weights.predicate_base_cost);
        }
        // println!("Objective for invariant {} is {} (CEX gain {}, BEX loss {}, predicate cost {})", invariant, score, cex_trace_gain, bex_loss, predicate_cost);
        return score;
    }

    pub fn merge_invariant_and_score_from_with_objective(&self, left_invariant: &predicates::InvariantWithScoreAndObjective, right_invariant: &predicates::InvariantWithScoreAndObjective, formula_score_weights: &data_types::general_data_types::FormulaScoreWeights) -> predicates::InvariantWithScoreAndObjective {
        let merged_invariant = left_invariant.invariant.merge_invariant(&right_invariant.invariant);
        let score = self.get_score_from_merge_sets(&left_invariant.score.cover_info.covered_states, &right_invariant.score.cover_info.covered_states);
        let objective = predicates::InvariantObjective {
            objective: self.calculate_invariant_objective_from_covered_states(&merged_invariant, &score.cover_info.covered_states, false,
            formula_score_weights, false)
        };
        let ret = predicates::InvariantWithScoreAndObjective {
            invariant: merged_invariant,
            score: score,
            objective: objective
        };
        ret
    }

    pub fn get_score_from_merge_sets(&self, left_cover_set: &BitSetWrapper, right_cover_set: &BitSetWrapper) -> predicates::ScoreAndFulfilledExample {
        let covered_states = left_cover_set.intersection(right_cover_set);
        let score = self.teacher.get_score_from_covered_states(&covered_states);
        let (blocked_cex_states, allowed_bex_states) = self.teacher.break_down_covered_states(&covered_states);
        let res = predicates::ScoreAndFulfilledExample {
            score: score,
            cover_info: predicates::CoverInformation {
                covered_states: covered_states,
                blocked_cex_states: blocked_cex_states,
                allowed_bex_states: allowed_bex_states,
            },
        };
        res
    }

    pub fn score_invariant_disjunction(&self, invariant_disjunction: &predicates::InvariantDisjunction) -> predicates::PriorityScores {
        let covered_states = self.get_covered_states_disjunction(invariant_disjunction);
        let score = self.teacher.get_score_from_covered_states(&covered_states);
        let (blocked_cex_states, allowed_bex_states) = self.teacher.break_down_covered_states(&covered_states);
        let res = predicates::ScoreAndFulfilledExample {
            score: score,
            cover_info: predicates::CoverInformation { covered_states: covered_states, blocked_cex_states: blocked_cex_states, allowed_bex_states: allowed_bex_states }
        };
        res.score
    }

    pub fn get_objective_from_separator_formula(&self, formula: &predicates::SeparatorFormula, allow_must_fullfill_bex_not_covered: bool, formula_score_weights: &data_types::general_data_types::FormulaScoreWeights) -> predicates::InvariantObjective {
        let covered_states = match formula {
            predicates::SeparatorFormula::Invariant(ref invariant) => self.get_covered_states(invariant),
            predicates::SeparatorFormula::InvariantDisjunction(ref invariant_disjunction) => self.get_covered_states_disjunction(invariant_disjunction),
        };
        let mut score = self.calculate_objective_from_covered_states(&covered_states, allow_must_fullfill_bex_not_covered, formula_score_weights, false);
        score -= costs::get_separator_formula_cost(self.teacher.cex_traces.len(), self.teacher.get_total_bex_weight(), formula, formula_score_weights.predicate_base_cost);
        return InvariantObjective { objective: score  };
    }

    // fn calculate_invariant_disjunction_objective(&self, invariant: &predicates::InvariantDisjunction, allow_must_fullfill_bex_not_covered: bool, formula_score_weights: &data_types::general_data_types::FormulaScoreWeights) -> predicates::InvariantObjective {
    //     let covered_states = self.get_covered_states_disjunction(invariant);
    //     let score = self.calculate_invariant_objective_from_covered_states(invariant, &covered_states, allow_must_fullfill_bex_not_covered, formula_score_weights);
    //     predicates::InvariantObjective {
    //         objective: score
    //     }
    // }
    pub fn debug_formula(&self, formula: &predicates::SeparatorFormula, formula_score_weights: &data_types::general_data_types::FormulaScoreWeights) {
        match formula {
            predicates::SeparatorFormula::Invariant(ref invariant) => {
                self.debug_invariant(invariant, formula_score_weights);
            },
            predicates::SeparatorFormula::InvariantDisjunction(ref invariant_disjunction) => {
                self.debug_disjunction(invariant_disjunction, formula_score_weights);
            },
        }
        let objective = self.get_objective_from_separator_formula(formula, false, formula_score_weights);
        println!("Objective from formula {:?}", objective);
    }

    pub fn debug_disjunction(&self, invariant_disjunction: &predicates::InvariantDisjunction, _formula_score_weights: & data_types::general_data_types::FormulaScoreWeights) {
        let covered_states: BitSetWrapper = self.get_covered_states_disjunction(invariant_disjunction);
        let mut num_bex_ci_and_cai_covered = 0;
        for (_, cex_trace) in self.teacher.cex_traces.iter().enumerate() {
            let cex_covered = cex_trace.is_covered_by_blocked_states(&covered_states);
            if cex_covered {
                if cex_trace.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                    let mut covered_cycles = Vec::new();
                    for sample in cex_trace.contained_samples.iter() {
                        if covered_states.contains(&(sample.state_id as u32)) {
                            covered_cycles.push(sample.and_cycle);
                        }
                    }
                    println!("CEX trace {:?} is blocked with covered cycles {:?}", cex_trace.from_path, covered_cycles);
                }
                // println!("CEX trace {:?} is blocked by invariant disjunction {:?}", cex_trace.from_path, invariant_disjunction);
            } else {
                let mut non_covered_cycles = Vec::new();
                for sample in cex_trace.contained_samples.iter() {
                    if !(covered_states.contains(&(sample.state_id as u32))) {
                        non_covered_cycles.push(sample.and_cycle);
                    }
                }
                println!("CEX trace {:?} is allowed with non-covered cycles {:?}", cex_trace.from_path, non_covered_cycles);
            }
        }
        let mut cnt_covered_bex = 0;
        for (_, bex_trace) in self.teacher.bex_traces.iter().enumerate() {
            let bex_covered = bex_trace.is_covered_by_blocked_states(&covered_states);
            if !bex_covered {
                // println!("BEX trace {:?} is allowed by invariant disjunction {:?}", bex_trace.from_path, invariant_disjunction);
            } else {
                // // let mut covered_cycles = HashSet::new();
                // let mut covered = false;
                // for state_id in bex_trace.contained_samples.iter().map(|x| x.state_id as u32) {
                //     if covered_states.contains(&state_id) {
                //         covered = true;
                //     }
                // }
                cnt_covered_bex += 1;
                let cycles = bex_trace.contained_samples.iter().filter_map(|sample| {
                    if covered_states.contains(&(sample.state_id as u32)) {
                        Some(sample.and_cycle)
                    } else {
                        None
                    }
                }).collect::<Vec<u64>>();
                println!("BEX trace {:?} is blocked at cycles {:?}", bex_trace.from_path, cycles);
                if bex_trace.from_path.contains("ci") || bex_trace.from_path.contains("cai") {
                    num_bex_ci_and_cai_covered += 1;
                }
                // println!("BEX trace {:?} is blocked", bex_trace.from_path);
            }   
        }
        // }
        println!("BEX samples covered {:?} (immediate {:?}), not covered {:?}", cnt_covered_bex, num_bex_ci_and_cai_covered, self.teacher.bex_traces.len().saturating_sub(cnt_covered_bex));

        let score = self.teacher.get_score_from_covered_states(&covered_states);
        println!("Score invariant {:?}", score);
        println!("Out of {:?} cex traces {:?} bex traces", self.teacher.cex_traces.len(), self.teacher.bex_traces.len());
        // println!("Invariant objective {:?}", self.calculate_invariant_objective(invariant, false,formula_score_weights));
    }


    pub fn score_disjunction_with_fulfilled_examples(&self, invariant_disjunction: &predicates::InvariantDisjunction) -> predicates::ScoreAndFulfilledExample {
        let covered_states = self.get_covered_states_disjunction(invariant_disjunction);
        let score = self.teacher.get_score_from_covered_states(&covered_states);
        let (blocked_cex_states, allowed_bex_states) = self.teacher.break_down_covered_states(&covered_states);
        let res = predicates::ScoreAndFulfilledExample {
            score: score,
            cover_info: predicates::CoverInformation { covered_states: covered_states, blocked_cex_states: blocked_cex_states, allowed_bex_states: allowed_bex_states }
        };
        res
    }
}


impl<'a> solver::Solver<'a> for SMTSolver<'a> {

    fn new_from_teacher(teacher: &'a teacher::Teacher) -> Self {
        SMTSolver {
            teacher
        }
    }





    fn debug_invariant(&self, invariant: &predicates::Invariant, formula_score_weights: & data_types::general_data_types::FormulaScoreWeights) {
        let mut cnt = 0;
        for maybe_sample in self.teacher.cex_traces.iter() {//.chain(vec![&self.teacher.original_cex].into_iter()) {
            let formula = construct_z3_formula_for_maybesample_and_invariant(maybe_sample, &invariant);
            //let res= solver.check_assumptions(&[formula]);
            if maybe_sample.file_source == Some( data_types::general_data_types::WaveFormSource::OriginalCex) {
                if maybe_sample.contained_samples.len() == 0 {
                    panic!("No samples in original cex");
                }
                for sample in maybe_sample.contained_samples.iter() {
                    let state = sample.state_pointer.upgrade().unwrap();
                    let restricted_sample = state.get_projection_from_signal_idx(&invariant.get_relevant_signal_idx());
                    println!("Restricted sample for original cex {:?} formula {:?}", restricted_sample, formula);
                }
            }
            let res: SatResult = formula.into();
            if res == SatResult::Unsat {
                println!("CEX sample {:?} not blocked", maybe_sample.from_path);
                cnt += 1;
//                if maybe_sample.from_path == "/home/viniul/formal/cex-generator/output/cexs/waveforms/minimized_cex.vcd" {
//                    println!("Sample {:?}", maybe_sample.samples[0].get_projection_from_signal_idx(&invariant.get_relevant_signal_idx()));
//                }
            } else {
                //println!("Fulfilled: CEX sample fulfilled");
            }
        }
        println!("CEX samples not covered {:?}, fulfilled {:?}", cnt, self.teacher.cex_traces.len().saturating_sub(cnt));
        cnt = 0;
        for sample in self.teacher.bex_samples.iter() {
            let formula = construct_z3_formula_for_bex_sample_and_invariant(&sample.state_pointer.upgrade().unwrap(), &invariant);
            let res: SatResult = formula.into();
            //let res= solver.check_assumptions(&[formula]);
            if res == SatResult::Unsat {
                println!("BEX waveform {:?} at cycle {:?} not allowed", sample.from_path, sample.and_cycle);
                cnt += 1;
            } else {
                // if sample.from_path == "/home/vincent/formal/cex-generator/output/benign_examples/waveforms/instructions_798.s.bin.vcd" && sample.and_cycle == 15 {
                //     println!("BEX waveform {:?} at cycle {:?} allowed occurence count {:?}", sample.from_path, sample.and_cycle, sample.occurrence_count);
                //     println!("Sample {:?}", sample.state_pointer.upgrade().unwrap().signal_to_value_map.get(&23039));
                // }
                // println!("BEX waveform {:?} at cycle {:?} allowed occurence count {:?} for paths {:?}", sample.from_path, sample.and_cycle, sample.occurrence_count);
                // if sample.from_path == "/home/vincent/formal/cex-generator/output/benign_examples/waveforms/instructions_661.s.bin.vcd" && (sample.and_cycle == 15 || sample.and_cycle == 16) {
                //     println!("BEX waveform {:?} at cycle {:?} allowed occurence count {:?}", sample.from_path, sample.and_cycle, sample.occurrence_count);
                //     println!("Vincent Debug Sample {:?}", sample.state_pointer.upgrade().unwrap().signal_to_value_map.get(&23039));
                // }
                //println!("Fulfilled: BEX waveform {:?} fulfilled");//, solver.get_model());
                //println!("Fulfilled: BEX waveform {:?} fulfilled", waveform.path);
            }
        }
        println!("BEX samples covered {:?}, not covered {:?}", cnt, self.teacher.bex_samples.len().saturating_sub(cnt));
        println!("Score invariant {:?}", self.score_invariant(invariant));
        println!("Out of {:?} cex samples {:?} bex samples", self.teacher.cex_traces.len(), self.teacher.bex_samples.len());
        println!("Invariant objective {:?}", self.calculate_invariant_objective(invariant, false,formula_score_weights));
    }   

    fn score_invariant_with_fulfilled_examples(&self, invariant: &predicates::Invariant) -> predicates::ScoreAndFulfilledExample {
        let covered_states = self.get_covered_states(invariant);
        let score = self.teacher.get_score_from_covered_states(&covered_states);
        let (blocked_cex_states, allowed_bex_states) = self.teacher.break_down_covered_states(&covered_states);
        let res = predicates::ScoreAndFulfilledExample {
            score: score,
            cover_info: predicates::CoverInformation { covered_states: covered_states, blocked_cex_states: blocked_cex_states, allowed_bex_states: allowed_bex_states }
        };
        res
    }


    fn merge_invariant_and_score(&self, left_invariant: &predicates::ScoredInvariantWithFulfilledExample, right_invariant: &predicates::ScoredInvariantWithFulfilledExample) -> predicates::ScoredInvariantWithFulfilledExample {
        let merged_invariant = left_invariant.invariant.merge_invariant(&right_invariant.invariant);
        let score = self.score_merged_invariant(left_invariant, right_invariant);
        let ret = predicates::ScoredInvariantWithFulfilledExample {
            invariant: merged_invariant,
            score: score,
        };
        ret
    }



    fn score_merged_invariant(&self, left_invariant: &predicates::ScoredInvariantWithFulfilledExample, right_invariant: &predicates::ScoredInvariantWithFulfilledExample) -> predicates::ScoreAndFulfilledExample {
        // let merged_invariant = left_invariant.invariant.merge_invariant(&right_invariant.invariant);
        let mut score_and_fulfilled_examples = predicates::ScoreAndFulfilledExample {
            score: PriorityScores::default(),
            cover_info: predicates::CoverInformation {
                covered_states: BitSetWrapper::new(),
                blocked_cex_states: BitSetWrapper::new(),
                allowed_bex_states: BitSetWrapper::new(),
            },
        };
        let covered_states = left_invariant.score.cover_info.covered_states.intersection(&right_invariant.score.cover_info.covered_states); //.cloned().collect::<HashSet<_>>();
        let score = self.teacher.get_score_from_covered_states(&covered_states);
        let (blocked_cex_states, allowed_bex_states) = self.teacher.break_down_covered_states(&covered_states);
        score_and_fulfilled_examples.score = score;
        score_and_fulfilled_examples.cover_info.covered_states = covered_states;
        score_and_fulfilled_examples.cover_info.blocked_cex_states = blocked_cex_states;
        score_and_fulfilled_examples.cover_info.allowed_bex_states = allowed_bex_states;
        return score_and_fulfilled_examples;
        //merged_invariant.score = merged_score.score;
        //merged_invariant
    }

    fn score_invariant(&self, invariant: &predicates::Invariant) -> predicates::PriorityScores {
        let res = self.score_invariant_with_fulfilled_examples(invariant);
        res.score
    }



    fn calculate_invariant_objective(&self, invariant: &predicates::Invariant, allow_must_fullfill_bex_not_covered: bool, formula_score_weights: &data_types::general_data_types::FormulaScoreWeights) -> predicates::InvariantObjective {
        let covered_states = self.get_covered_states(invariant);
        let score = self.calculate_invariant_objective_from_covered_states(invariant, &covered_states, allow_must_fullfill_bex_not_covered, formula_score_weights, false);
        predicates::InvariantObjective {
            objective: score
        }
    }
}
