use crate::allowed_signal_config;
use crate::constants;
use crate::data_types::general_data_types::SignalIndexSet;
use crate::decision_tree;
use crate::jg_formatter;
use crate::predicates;
use crate::predicates::PredicateLike;
use crate::predicates::PriorityScores;
use crate::smt_solver::SMTSolver as thisSolver;
use crate::teacher;
use crate::waveform;
use crate::waveform::WaveForm;
//use crate::ilp_solver::ILPSolver as thisSolver;
use crate::data_types;
use crate::set_cover_solver as set_cover_solver_module;
use crate::solver::Solver;
use crate::utils;
use core::panic;
use indicatif::{self, ParallelProgressIterator};
use log;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rayon::slice::ParallelSliceMut;
use serde;
use serde_json::{self};
use std;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InputCexDescription {
    pub bug_type: Option<String>,
    pub instructions: Option<Vec<String>>,
    pub minimized_instructions: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SearchResult {
    pub separator_formula: predicates::SeparatorFormula,
    pub assume_invariant: String,
    pub assert_invariant: Option<String>,
    pub signal_aliases: std::collections::HashMap<Arc<str>, Vec<Arc<str>>>,
    pub input_cex: Option<InputCexDescription>,
    pub bex_fulfilled_percentage: Option<i64>,
    pub cex_fulfilled_percentage: Option<i64>,
    pub score: Option<PriorityScores>,
    pub objective: Option<predicates::InvariantObjective>,
}

/*
mod hex_hashmap {
    use super::*;
    use serde::ser::SerializeMap;
    use serde::de::{self, MapAccess, Visitor};
    use std::fmt;

    pub fn serialize<S>(map: &HashMap<String, i64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut hex_map = serializer.serialize_map(Some(map.len()))?;
        for (k, &v) in map {
            hex_map.serialize_entry(k, &format!("0x{:x}", v))?;
        }
        hex_map.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<String, i64>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HexHashMapVisitor;

        impl<'de> Visitor<'de> for HexHashMapVisitor {
            type Value = HashMap<String, i64>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with hex string values")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut map = HashMap::with_capacity(access.size_hint().unwrap_or(0));
                while let Some((key, value)) = access.next_entry::<String, String>()? {
                    let value = i64::from_str_radix(&value, 16).map_err(de::Error::custom)?;
                    map.insert(key, value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(HexHashMapVisitor)
    }
}
*/

// pub struct PriorityInvariantSearcher {
//    main_teacher: teacher::Teacher,
// }

pub struct PriorityInvariantSearcher<
    H: std::hash::BuildHasher = data_types::general_data_types::DefaultScalarHasher,
> {
    pub cex_datapoints: Vec<data_types::general_data_types::FuzzerDataPoint>,
    pub bex_datapoints: Vec<data_types::general_data_types::FuzzerDataPoint>,
    clock_signal: String,
    signal_remap_info: Option<data_types::signal_remap_info::SignalRemappingInfo>,
    pub name_to_code: HashMap<ustr::Ustr, u64, H>,
    pub allowed_signal_idxs: Option<SignalIndexSet>,
    pub formula_score_weights: data_types::general_data_types::FormulaScoreWeights,
}

pub fn load_waveforms_in_parallel(paths: Vec<String>, clock_signal: &str) -> Vec<WaveForm> {
    println!("Loading {:?} wavefroms", paths.len());
    let style = indicatif::ProgressStyle::default_bar();
    let map_result = paths.par_iter().progress_with_style(style).map(|path| {
        let waveform = WaveForm::load_waveform_and_cycle_map(path, clock_signal, None).unwrap();
        waveform
    });
    map_result.collect()
}

pub fn load_waveforms_in_parallel_from_fuzzer_data_points(
    data_points: Vec<data_types::general_data_types::FuzzerDataPoint>,
    clock_signal: &str,
) -> Vec<WaveForm> {
    println!("Loading {:?} wavefroms", data_points.len());
    let style = indicatif::ProgressStyle::default_bar();
    let map_result = data_points
        .par_iter()
        .progress_with_style(style)
        .map(|data_point| {
            let waveform = WaveForm::load_waveform_and_cycle_map_from_fuzzer_datapoint(
                data_point,
                clock_signal,
                None,
            )
            .unwrap();
            //println!("Ustr cache size {}", ustr::num_entries());
            waveform
        });
    map_result.collect()
}

impl PriorityInvariantSearcher {
    /*pub fn new_from_waveforms(cex_waveform_paths: Vec<String>, bex_waveform_paths: Vec<String>, original_cex_path: String, clock_signal: &str) -> Self {
        let cex_waveforms = load_waveforms_in_parallel(cex_waveform_paths, clock_signal);
        let mut bex_waveforms = load_waveforms_in_parallel(bex_waveform_paths, clock_signal);
        //Sort bex waveforms so that the waveform with source data_types::WaveFormSource::Mutations come firs
        // Sort the waveforms
        bex_waveforms.sort_by(|a, b| {
            if a.file_source == data_types::WaveFormSource::Mutations && b.file_source != data_types::WaveFormSource::Mutations {
                Ordering::Less
            } else if a.file_source != data_types::WaveFormSource::Mutations && b.file_source == data_types::WaveFormSource::Mutations {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });


        let original_cex_waveform = WaveForm::load_waveform_and_cycle_map(original_cex_path.as_str(), clock_signal).unwrap();
        let signal_list = cex_waveforms[0].name_to_code.keys().cloned().map(|s| Arc::from(s)).collect::<Vec<_>>();
        //let signal_list = cex_waveforms[0].name_to_code.keys().cloned().collect();
        println!("Waveforms loaded"); //     signal_list {:?}", signal_list);
        PriorityInvariantSearcher {
            cex_waveforms,
            bex_waveforms,
            original_cex_waveform,
            signal_list
        }
    }

    pub fn new_from_fuzzer_data_point(cex_datapoints: Vec<data_types::FuzzerDataPoint>, bex_datapoints: Vec<data_types::FuzzerDataPoint>, original_cex_path: String, clock_signal: &str) -> Self {
        let cex_waveforms = load_waveforms_in_parallel_from_fuzzer_data_points(cex_datapoints, clock_signal);
        let mut reduced_bex_datapoints = bex_datapoints
            .iter()
            .filter(|datapoint| datapoint.file_source == data_types::WaveFormSource::Mutations)
            .cloned()
            .collect::<Vec<_>>();
        reduced_bex_datapoints.extend(
            bex_datapoints.
            iter().filter(|datapoint| datapoint.file_source != data_types::WaveFormSource::Mutations).take(constants::MAX_NUM_BEX).cloned());
        let bex_waveforms = load_waveforms_in_parallel_from_fuzzer_data_points(reduced_bex_datapoints, clock_signal);
        //for (i, waveform) in bex_waveforms.iter().enumerate() {
            //println!("BEX Waveform {}: Path: {}, Data Source: {:?}", i, waveform.path, waveform.file_source);
        //}
        let original_cex_waveform = WaveForm::load_waveform_and_cycle_map(original_cex_path.as_str(), clock_signal).unwrap();
        let signal_list = cex_waveforms[0].name_to_code.keys().cloned().map(|s| Arc::from(s)).collect::<Vec<_>>();
        println!("Waveforms loaded"); //     signal_list {:?}", signal_list);
        PriorityInvariantSearcher {
            cex_waveforms,
            bex_waveforms,
            original_cex_waveform,
            signal_list
        }

    }*/
    pub fn new_from_fuzzer_data_point(
        cex_datapoints: Vec<data_types::general_data_types::FuzzerDataPoint>,
        bex_datapoints: Vec<data_types::general_data_types::FuzzerDataPoint>,
        clock_signal: &str,
        formula_score_weights: data_types::general_data_types::FormulaScoreWeights,
    ) -> Self {
        // let teacher = teacher::new_from_fuzzer_data_point(&cex_datapoints, &bex_datapoints, clock_signal);
        // PriorityInvariantSearcher { main_teacher: teacher }
        let some_data_point = if cex_datapoints.len() > 0 {
            cex_datapoints.first().unwrap()
        } else {
            bex_datapoints.first().unwrap()
        };
        //println!("Sorted bex datapoints {:?}", sorted_bex_datapoints);
        let some_waveform = waveform::WaveForm::load_waveform_and_cycle_map_from_fuzzer_datapoint(
            some_data_point,
            clock_signal,
            None,
        )
        .unwrap();
        let name_to_code = some_waveform.name_to_code;
        log::debug!("Name to code has {} entries", name_to_code.len());
        for (entry_key, entry_value) in name_to_code.iter().take(300) {
            log::debug!(
                "Name to code entry key {:?} value {:?}",
                entry_key,
                entry_value
            );
        }
        PriorityInvariantSearcher {
            cex_datapoints: cex_datapoints,
            bex_datapoints: bex_datapoints,
            clock_signal: clock_signal.to_owned(),
            signal_remap_info: None,
            name_to_code: name_to_code,
            allowed_signal_idxs: None,
            formula_score_weights: formula_score_weights,
        }
    }

    pub fn new_from_fuzzer_data_point_and_signal_remap_info(
        cex_datapoints: Vec<data_types::general_data_types::FuzzerDataPoint>,
        bex_datapoints: Vec<data_types::general_data_types::FuzzerDataPoint>,
        verilator_clock_signal: &str,
        signal_remap_info: data_types::signal_remap_info::SignalRemappingInfo,
        formula_score_weights: data_types::general_data_types::FormulaScoreWeights,
    ) -> Self {
        let some_data_point = if cex_datapoints.len() > 0 {
            cex_datapoints.first().unwrap()
        } else {
            bex_datapoints.first().unwrap()
        };
        //println!("Sorted bex datapoints {:?}", sorted_bex_datapoints);
        let some_waveform = waveform::WaveForm::load_waveform_and_cycle_map_from_fuzzer_datapoint(
            some_data_point,
            verilator_clock_signal,
            None,
        )
        .unwrap();
        let name_to_code = some_waveform.name_to_code;
        log::debug!("Name to code has {} entries", name_to_code.len());

        let allowed_signal_list =
            signal_remap_info.create_allowed_signal_list_from_name_to_code_map(&name_to_code);
        for (entry_key, entry_value) in name_to_code.iter().take(300) {
            if !(allowed_signal_list.contains(entry_value)) {
                log::debug!(
                    "Skipping Name to code entry key {:?} value {:?}",
                    entry_key,
                    entry_value
                );
            } else {
                // log::debug!("Name to code entry key {:?} value {:?}", entry_key, entry_value);
            }
            // log::debug!("Name to code entry key {:?} value {:?}", entry_key, entry_value);
        }
        PriorityInvariantSearcher {
            cex_datapoints: cex_datapoints,
            bex_datapoints: bex_datapoints,
            clock_signal: verilator_clock_signal.to_owned(),
            signal_remap_info: Some(signal_remap_info),
            name_to_code: name_to_code,
            formula_score_weights: formula_score_weights,
            allowed_signal_idxs: Some(allowed_signal_list),
        }
        // let mut teacher = teacher::new_from_fuzzer_data_point(&cex_datapoints, &bex_datapoints, verilator_clock_signal);
        // teacher.remove_signals_not_in_jg_signal_list(allowed_signal_list);
        // PriorityInvariantSearcher { main_teacher: teacher }
    }

    pub fn search_separator_in_stages(
        &self,
        regex_stages: serde_json::Value,
        bug_type: Option<String>,
    ) -> Option<SearchResult> {
        //let this_regex_stages = regex_stages.as_array().unwrap();

        let this_regex_stages: Vec<&serde_json::Value> = if let Some(bug_type) = bug_type {
            if bug_type == "BugTypeResult.EXPOSE_ONLY" {
                regex_stages.as_array().unwrap().iter().collect()
                // let this_regex_stages: Vec<&serde_json::Value> = regex_stages.as_array().unwrap()
                //     .iter()
                //     .filter(|stage| stage.as_object().unwrap().contains_key("decoder_only") || stage.as_object().unwrap().contains_key("ref_only"))
                //     .collect();
                // this_regex_stages
                // let mut this_regex_stages: Vec<&serde_json::Value> = regex_stages.as_array().unwrap().iter().collect();
                // this_regex_stages.sort_by_key(|stage| {
                //     if stage.as_object().unwrap().contains_key("decoder_only") {
                //         0
                //     } else {
                //         1
                //     }
                // });
                // this_regex_stages<
            } else {
                regex_stages.as_array().unwrap().iter().collect()
            }
        } else {
            regex_stages.as_array().unwrap().iter().collect()
        };
        //for _ in 0..10 {
        let mut max_res: Option<SearchResult> = None;
        // let main_teacher = teacher::new_from_fuzzer_data_point(&self.cex_datapoints, &self.bex_datapoints, &self.clock_signal);
        // let main_teacher_solver = thisSolver::new_from_teacher(self.main_teacher.clone());
        for stage in this_regex_stages.clone() {
            // let (signal_list_final, core_type) = self.get_signal_list_from_stage(stage);
            // let stage_filter = self.get_stage_filter_from_stage_json(stage);
            // let signal_list_final = signal_list_final.into_iter().map(|s| Arc::from(s)).collect::<Vec<_>>();
            // println!("NOw searching for separator in stage {:?} core_type {:?}", stage, core_type);
            let threshold = match &max_res {
                Some(ref res) => Some(res.cex_fulfilled_percentage.unwrap() as usize),
                None => None,
            };
            let this_stage: data_types::general_data_types::StageFilter =
                self.get_stage_filter_from_stage_json(stage);
            let min_objective = match &max_res {
                Some(ref res) => res.objective.clone(),
                None => None,
            };
            log::info!("Now searching for separator in stage {:?} core_type {:?} with threshold {:?} objective {:?}", stage, this_stage.core_type, threshold, min_objective);
            let (res, is_csr_formula) =
                self.search_separator_with_stage_filter(&this_stage, min_objective);
            // let res = self.search_separator_from_signals(signal_list_final.clone(), core_type.clone(), threshold);
            //if core_type == teacher::CoreType::RefCore {
            //    continue;
            //}
            if res.is_some() {
                let res = res.unwrap();
                // return Some(res);
                if is_csr_formula {
                    println!("Found CSR formula, returning immediately");
                    return Some(res);
                } else {
                    println!("Found non-CSR formula, continuing search");
                }
                if res.separator_formula.get_all_predicates().len() == 1
                    && res.cex_fulfilled_percentage.unwrap_or(0) == 100
                    && res.bex_fulfilled_percentage.unwrap_or(0) == 100
                {
                    println!("Found perfect formula, skipping further search");
                    return Some(res);
                }
                // res.score = Some(main_teacher_solver.score_invariant(&res.invariant, None, None, true)); //Rescore w.r.t. main teacher
                // let num_allowed_bex = res.score.as_ref().unwrap().cex_and_bex_score.get_inner_or_zero() as usize-res.score.as_ref().unwrap().cex_only_score.get_inner_or_zero() as usize;
                // res.bex_fulfilled_percentage = Some(((num_allowed_bex as f64) /
                // (self.main_teacher.bex_samples.len() as f64) * 100.0) as i64);
                // res.cex_fulfilled_percentage = Some(((res.score.as_ref().unwrap().cex_only_score.get_inner_or_zero() as f64) /
                // (self.main_teacher.cex_traces.len() as f64) * 100.0) as i64);
                println!(
                    "Found invariant {} with score {:?} objective {:?}",
                    res.separator_formula, res.cex_fulfilled_percentage, res.objective
                );
                let max_res_objective = match max_res {
                    Some(ref r) => r.objective.as_ref().unwrap().clone(),
                    None => predicates::InvariantObjective::default(),
                };

                if max_res.is_none()
                    || max_res
                        .as_ref()
                        .unwrap()
                        .objective
                        .as_ref()
                        .unwrap()
                        .objective
                        < res.objective.as_ref().unwrap().objective
                {
                    max_res = Some(res.clone());
                    //println!("Found invariant {} with score {:?}", res.invariant, res.cex_fulfilled_percentage);
                    //return Some(res);
                } else {
                    println!("Skipping invariant as objective did not improve. Current max res objective {:?} new objective {:?}", max_res_objective, res.objective.as_ref().unwrap());
                }
                println!(
                    "Current max res objective {:?} max_res formula {}",
                    max_res_objective,
                    max_res.as_ref().unwrap().separator_formula
                );
            }
            //return None;
        }
        if max_res.is_some() {
            println!(
                "Returning max res {} with objective {:?}",
                max_res.as_ref().unwrap().separator_formula,
                max_res.as_ref().unwrap().objective
            );
            return max_res;
        }
        //REQUIRED_BEX_FULFILLED.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        //}
        None
    }

    pub fn is_csr_formula(
        &self,
        this_teacher: &teacher::Teacher,
        formula: &predicates::SeparatorFormula,
    ) -> bool {
        let predicates = formula.get_all_predicates();
        let mut csr_opcode_predicate_contained = false;
        let mut contains_imm_i = false;
        for predicate in predicates {
            if predicate.is_two_signal_equal() {
                continue;
            }
            let signal_idx_set = predicate.get_signal_idx();
            let signal_idx = signal_idx_set.iter().next().unwrap();
            let signal_info = this_teacher.get_signal_info_from_index(signal_idx);
            if signal_info.is_none() {
                continue;
            }
            let signal_info = signal_info.unwrap();
            if signal_info.aliases.is_empty() {
                continue;
            }
            let contains_csr_written_bitmap_reg =
                signal_info.any_alias_contains("csr_written_bitmap_reg", false);
            if contains_csr_written_bitmap_reg {
                let base_formula = &predicate.base_formula;
                match base_formula {
                    predicates::BaseFormula::SignalToConst(ref signal_to_const_formula) => {
                        let const_value = signal_to_const_formula.get_value();
                        if const_value == 1 {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            let contains_opcode = signal_info.any_alias_contains("opcode", false);
            if contains_opcode {
                let base_formula = &predicate.base_formula;
                match base_formula {
                    predicates::BaseFormula::SignalToConst(ref signal_to_const_formula) => {
                        let const_value = signal_to_const_formula.get_value();
                        if const_value == constants::OPCODE_CSR {
                            csr_opcode_predicate_contained = true;
                        }
                    }
                    _ => {}
                }
            }
            let contains_imm_i_signal = signal_info.any_alias_contains("imm_i", true);
            if contains_imm_i_signal {
                contains_imm_i = true;
            }
        }
        contains_imm_i && csr_opcode_predicate_contained
    }

    pub fn search_separator_with_stage_filter(
        &self,
        stage_filter: &data_types::general_data_types::StageFilter,
        min_objective: Option<predicates::InvariantObjective>,
    ) -> (Option<SearchResult>, bool) {
        let predicate_generation_config =
            allowed_signal_config::get_config_from_stage_filter(stage_filter);
        let global_teacher = teacher::new_from_fuzzer_data_point(
            &self.cex_datapoints,
            &self.bex_datapoints,
            &self.clock_signal,
            Some(&stage_filter),
            self.allowed_signal_idxs.as_ref(),
            &predicate_generation_config,
        );
        let global_teacher_solver = thisSolver::new_from_teacher(&global_teacher);
        let mut this_teacher = global_teacher.clone();
        let mut current_disjunction: Option<&predicates::InvariantDisjunction> = None;
        let mut current_res: Option<SearchResult> = None;
        let done = false;
        let mut iter_idx = 0;
        let stage_name = &stage_filter.stage_name;
        while !done {
            println!(
                "Searching for next disjunction component at iteration {}",
                iter_idx
            );
            let objective_lower_bound = match &current_res {
                Some(res) => Some(res.objective.as_ref().unwrap()),
                None => match &min_objective {
                    Some(obj) => Some(obj),
                    None => None,
                },
            };
            let this_res: Option<SearchResult> = self.search_from_teacher(
                &this_teacher,
                &stage_filter.core_type,
                current_disjunction,
                &stage_name,
                objective_lower_bound,
                &predicate_generation_config,
            );
            if this_res.is_none() {
                break;
            }
            let this_res = this_res.unwrap();
            if current_res.is_none() {
                current_res = Some(this_res);
            } else {
                if this_res.objective.as_ref().unwrap().objective
                    > current_res
                        .as_ref()
                        .unwrap()
                        .objective
                        .as_ref()
                        .unwrap()
                        .objective
                {
                    current_res = Some(this_res);
                } else {
                    println!("Objective did not improve, stopping. Current objective {:?}, new objective {:?}", current_res.as_ref().unwrap().objective.as_ref().unwrap().objective, this_res.objective.as_ref().unwrap().objective);
                    println!("Final disjunction: {}", current_disjunction.unwrap());
                    println!("Stopped at {}", this_res.separator_formula);
                    break;
                }
            }
            iter_idx += 1;
            let covered_states = global_teacher_solver
                .get_covered_states_from_formula(&current_res.as_ref().unwrap().separator_formula);
            let covered_cex_traces =
                this_teacher.get_covered_cex_traces_from_covered_states(&covered_states);
            if covered_cex_traces.len() == this_teacher.cex_traces.len() {
                println!("All CEX traces covered, stopping");
                break;
            }
            this_teacher.set_new_original_cex(&covered_cex_traces);
            match current_res.as_ref().unwrap().separator_formula {
                predicates::SeparatorFormula::InvariantDisjunction(ref this_disjuction) => {
                    current_disjunction = Some(this_disjuction);
                }
                _ => {
                    panic!("Expected disjunction, unimplemented for other types");
                }
            }
            println!(
                "Current disjunction {} at iteration {}",
                current_res.as_ref().unwrap().separator_formula,
                iter_idx
            );

            let rescore =
                global_teacher_solver.score_invariant_disjunction(current_disjunction.unwrap());
            println!(
                "Rescored disjunction after iteration {}: score {:?}, objective {:?}",
                iter_idx,
                rescore,
                current_res.as_ref().unwrap().objective
            );
            let objective = global_teacher_solver.get_objective_from_separator_formula(
                &current_res.as_ref().unwrap().separator_formula,
                false,
                &self.formula_score_weights,
            );
            println!("Objective after iteration {}: {:?}", iter_idx, objective);
            let is_csr = self.is_csr_formula(
                &this_teacher,
                &current_res.as_ref().unwrap().separator_formula,
            );
            println!("Is CSR formula after iteration {}: {}", iter_idx, is_csr);
            if current_res.is_some()
                && self.is_csr_formula(
                    &this_teacher,
                    &current_res.as_ref().unwrap().separator_formula,
                )
            {
                return (current_res, false);
            } else {
                return (current_res, false);
            }
        }
        if current_res.is_some()
            && self.is_csr_formula(
                &this_teacher,
                &current_res.as_ref().unwrap().separator_formula,
            )
        {
            return (current_res, false);
        } else {
            return (current_res, false);
        }

        //How can we perform a disjunction search?
        //Iter: Start with "empty disjunction"
        //Then, score all predicates "w.r.t. the current disjunction"
        //Do set-cover algorithm to find best invariant
        //Loop: Add one invariant to the disjunction
        //Do we need to remove already covered CEX traces from the teacher?
        //I do not think so: The coversets of each predicate will all include the already covered CEX traces
        //However: We do need to adjust the OriginalCEX in the predicate generation
        // Check if objective is improved
        // Stop when objective does not improve anymore
    }

    pub fn get_stage_filter_from_stage_json(
        &self,
        stage: &serde_json::Value,
    ) -> data_types::general_data_types::StageFilter {
        let stage_name = stage.as_object().unwrap().keys().next().unwrap();
        // let core_type = match stage_name.as_str() {
        //     "decoder_only" => data_types::CoreType::RefCore,
        //     "dut_only" => data_types::CoreType::DutCore,
        //     "ref_only" => data_types::CoreType::RefCore,
        //     _ => data_types::CoreType::DutCore
        // };
        let core_type = stage[stage_name].get("core_type");
        let core_type = match core_type {
            Some(core_type) => match core_type.as_str().unwrap() {
                "RefCore" => data_types::general_data_types::CoreType::RefCore,
                "DutCore" => data_types::general_data_types::CoreType::DutCore,
                _ => data_types::general_data_types::CoreType::DutCore,
            },
            None => match stage_name.as_str() {
                "decoder_only" => data_types::general_data_types::CoreType::RefCore,
                "dut_only" => data_types::general_data_types::CoreType::DutCore,
                "ref_only" => data_types::general_data_types::CoreType::RefCore,
                _ => data_types::general_data_types::CoreType::DutCore,
            },
        };
        println!("Doing stage {}", stage_name);
        let stage_filters_include: Vec<regex::Regex> = stage[stage_name]["include"]
            .as_array()
            .unwrap()
            .iter()
            .map(|this_regex| regex::Regex::new(this_regex.as_str().unwrap()).unwrap())
            .collect();
        let stage_filters_exclude: Vec<regex::Regex> =
            stage[stage_name]
                .get("exclude")
                .map_or(Vec::new(), |exclude| {
                    exclude
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|this_regex| regex::Regex::new(this_regex.as_str().unwrap()).unwrap())
                        .collect()
                });
        println!("Stage filters {:?}", stage_filters_include);
        println!("Stage filters {:?}", stage_filters_exclude);
        data_types::general_data_types::StageFilter {
            include: stage_filters_include,
            exclude: stage_filters_exclude,
            core_type,
            stage_name: stage_name.to_string(),
        }
    }

    // pub fn get_signal_list_from_stage(&self, stage: &serde_json::Value) -> (Vec<Arc<str>>, teacher::CoreType) {
    //     let stage_name = stage.as_object().unwrap().keys().next().unwrap();
    //     let core_type = match stage_name.as_str() {
    //         "decoder_only" => teacher::CoreType::RefCore,
    //         "dut_only" => teacher::CoreType::DutCore,
    //         "ref_only" => teacher::CoreType::RefCore,
    //         _ => teacher::CoreType::DutCore
    //     };
    //     println!("Doing stage {}", stage_name);
    //     let stage_filters_include: Vec<regex::Regex> = stage[stage_name]["include"]
    //         .as_array()
    //         .unwrap()
    //         .iter()
    //         .map(|this_regex| regex::Regex::new(this_regex.as_str().unwrap()).unwrap())
    //         .collect();
    //     let stage_filters_exclude: Vec<regex::Regex> = stage[stage_name]
    //         .get("exclude")
    //         .map_or(Vec::new(), |exclude| {
    //         exclude
    //             .as_array()
    //             .unwrap()
    //             .iter()
    //             .map(|this_regex| regex::Regex::new(this_regex.as_str().unwrap()).unwrap())
    //             .collect()
    //         });
    //     println!("Stage filters {:?}", stage_filters_include);
    //     println!("Stage filters {:?}", stage_filters_exclude);
    //     let signal_list_old: Vec<Arc<str>> = self.main_teacher.get_signal_list().iter()
    //         .filter(|signal| stage_filters_include.iter().any(|re| re.is_match(signal)))
    //         .cloned()
    //         .collect();
    //     // for signal in  signal_list_old.iter() {
    //     //     println!("Signal {}", signal);
    //     // }
    //     //println!("signal_list_old {:?}", &signal_list_old);
    //     println!("Found in signal_list_old {:?}", signal_list_old.contains(&Arc::from("TOP.correctness.sodor_core.router.resp_in_range".to_string())));
    //     let signal_list_old: Vec<Arc<str>> = signal_list_old
    //         .into_iter()
    //         .filter(|signal| !stage_filters_exclude.iter().any(|re| re.is_match(signal)))
    //         .collect();
    //     println!("Found in signal_list_old {:?}", signal_list_old.contains(&Arc::from("TOP.correctness.sodor_core.router.resp_in_range".to_string())));
    //     //println!("Signal list {:?}", signal_list_old);
    //     let signal_list_new: Vec<Arc<str>> = self.main_teacher.filter_unique_signals(&signal_list_old); //Filter out signal with duplicate id codes
    //     println!("Reduction from {} to {} by filtering out unique ids", signal_list_old.len(), signal_list_new.len());
    //     println!("Found in unique list list {:?}", signal_list_new.contains(&Arc::from("TOP.correctness.sodor_core.router.resp_in_range".to_string())));
    //     //println!("Signal list after filtering unique ids {:?}", signal_list_new);
    //     let signal_list_new: Vec<Arc<str>> = self.main_teacher.filter_control_signals(&signal_list_new); //Filter out signal with duplicate id codes
    //     //println!("Signal list after filtering control signal {:?}", signal_list_new);
    //     println!("To {} signals after filtering control signals", signal_list_new.len());
    //     println!("Found in list {:?}", signal_list_new.contains(&Arc::from("TOP.correctness.sodor_core.router.resp_in_range".to_string())));
    //     //let this_teacher: teacher::Teacher = teacher::get_samples_from_waveforms(&self.cex_waveforms, &self.bex_waveforms, &self.original_cex_waveform, signal_list_new.clone(), &core_type);
    //     let signal_list_final = signal_list_new.clone();  //this_teacher.filter_useful_signals(&signal_list_new);
    //     println!("Reduction from {} to {} by filtering out useful signals", signal_list_new.len(), signal_list_final.len());
    //     //println!("Found in list {:?}", signal_list_final.contains(&Arc::from("TOP.correctness.sodor_core.router.resp_in_range".to_string())));
    //     // println!("Final signal list after filtering");
    //     // for signal in signal_list_final.iter() {
    //         // println!("Signal {}", signal);
    //     // }
    //     //println!("Useful signals {:?}", signal_list_final);
    //     //drop(signal_list_new);drop(signal_list_old);
    //     return (signal_list_final, core_type);
    // }

    pub fn debug_formula(
        &self,
        invariant: &mut predicates::SeparatorFormula,
        regex_stages: serde_json::Value,
        bug_type: Option<String>,
    ) {
        let this_regex_stages: Vec<&serde_json::Value> = if let Some(bug_type) = bug_type {
            if bug_type == "BugTypeResult.EXPOSE_ONLY" {
                // let mut this_regex_stages: Vec<&serde_json::Value> = regex_stages.as_array().unwrap().iter().collect();
                // this_regex_stages.sort_by_key(|stage| {
                //     if stage.as_object().unwrap().contains_key("decoder_only") {
                //         0
                //     } else {
                //         1
                //     }
                // });
                // this_regex_stages
                regex_stages.as_array().unwrap().iter().collect()
                // let this_regex_stages: Vec<&serde_json::Value> = regex_stages.as_array().unwrap()
                //     .iter()
                //     .filter(|stage| stage.as_object().unwrap().contains_key("decoder_only") || stage.as_object().unwrap().contains_key("ref_only"))
                //     .collect();
                // this_regex_stages
            } else {
                regex_stages.as_array().unwrap().iter().collect()
            }
        } else {
            regex_stages.as_array().unwrap().iter().collect()
        };

        for stage in this_regex_stages {
            //println!("For stage {:?}", stage);
            let _skip_stage = false;
            // let (signal_list_final, core_type) = self.get_signal_list_from_stage(stage);
            //println!("Signal list final {:?}", signal_list_final);
            /*for predicate in invariant.predicate_set.predicates.iter() {
                if !signal_list_final.contains(&predicate.get_signal_name().to_string()) {
                    println!(
                        "Invariant contains signal '{}' not in signal_list_final",
                        predicate.get_signal_name()
                    );
                    skip_stage = true;
                }
            }*/
            //if (skip_stage) {
            //    println!("Skipping stage {}, invariant not in signal_list_final", stage);
            //    continue;
            //}
            //let signal_list_final: Vec<String> = invariant.get_relevant_signals().iter().map(|s| s.to_string()).collect();
            //let core_type = teacher::CoreType::RefCore;
            //let this_teacher = teacher::get_samples_from_waveforms(&self.cex_waveforms, &self.bex_waveforms, &self.original_cex_waveform, signal_list_final.clone(), &core_type);
            // let this_teacher = teacher::new_teacher_from_restricted_signals(&self.main_teacher, &signal_list_final, &core_type);
            let stage_filter = self.get_stage_filter_from_stage_json(stage);
            let predicate_generation_config =
                allowed_signal_config::get_config_from_stage_filter(&stage_filter);
            let this_teacher = teacher::new_from_fuzzer_data_point(
                &self.cex_datapoints,
                &self.bex_datapoints,
                &self.clock_signal,
                Some(&stage_filter),
                self.allowed_signal_idxs.as_ref(),
                &predicate_generation_config,
            );
            let solver_instance = thisSolver::new_from_teacher(&this_teacher);
            if this_teacher.fill_in_indexes_for_formula(invariant).is_err() {
                println!("Signal not present in this stage, skipping");
                continue;
            }
            /*for predicate in invariant.predicate_set.predicates.iter_mut() {
                if this_teacher.fill_in_indexes_for_predicate(predicate).is_err() {
                    println!("Predicate {} not in waveform", predicate.get_signal_name());
                    skip_stage = true;
                } else if this_teacher.cex_traces[0].samples[0].sample.get(&predicate.base_formula..unwrap()) == None {
                    println!("Predicate {} not in cex samples", predicate.get_signal_name());
                    skip_stage = true;
                }
            }
            if skip_stage {
                println!("Skipping stage {}, invariant not in signal_list_final", stage);
                continue;
            }*/
            println!("Debugging invariant {}", invariant);
            solver_instance.debug_formula(invariant, &self.formula_score_weights);
            println!("Found fitting state, done!");
            return;
        }
    }

    pub fn get_signal_aliases(&self, signal_name: &str) -> Vec<Arc<str>> {
        match self.name_to_code.get(&ustr::Ustr::from(signal_name)) {
            Some(&code) => {
                let mut aliases: Vec<Arc<str>> = self
                    .name_to_code
                    .iter()
                    .filter(|(_, &v)| v == code)
                    .map(|(k, _)| Arc::from(k.as_str()))
                    .collect();
                aliases.sort();
                aliases.dedup();
                aliases
            }
            None => {
                println!("Signal {} not found in name_to_code", signal_name);
                vec![]
            }
        }
    }

    pub fn invariant_conjunction_to_search_result(
        &self,
        invariant_disjunction: &predicates::InvariantDisjunction,
    ) -> SearchResult {
        let separator_formula =
            predicates::SeparatorFormula::InvariantDisjunction(invariant_disjunction.clone());
        let assume_invariant = jg_formatter::get_jg_assume_command(
            &separator_formula,
            self.signal_remap_info.as_ref(),
            &self.name_to_code,
        );
        let assert_invariant = None; //format!("// Not implemented for disjunctions yet");
        let signal_aliases: HashMap<Arc<str>, Vec<Arc<str>>> = std::collections::HashMap::new();
        // for invariant in invariant_disjunction.invariants.iter() {
        //     for signal_name in invariant.get_relevant_signals() {
        //         let aliases = self.get_signal_aliases(&signal_name);
        //         signal_aliases.insert(Arc::from(signal_name), aliases);
        //     }
        // }
        let res = SearchResult {
            separator_formula: predicates::SeparatorFormula::InvariantDisjunction(
                invariant_disjunction.clone(),
            ),
            assume_invariant,
            assert_invariant,
            signal_aliases: signal_aliases,
            input_cex: None,
            ..Default::default()
        };
        return res;
    }

    pub fn formula_to_search_result(
        &self,
        invariant: &predicates::SeparatorFormula,
    ) -> SearchResult {
        let assume_invariant = jg_formatter::get_jg_assume_command(
            invariant,
            self.signal_remap_info.as_ref(),
            &self.name_to_code,
        );
        let assert_invariant = None; // format!("Not implemented for disjunctions yet"); //jg_formatter::get_jg_assert_command(invariant, self.signal_remap_info.as_ref(),&self.name_to_code);

        let mut signal_aliases: HashMap<Arc<str>, Vec<Arc<str>>> = std::collections::HashMap::new();
        for signal_name in invariant.get_relevant_signals() {
            let aliases = self.get_signal_aliases(&signal_name);
            signal_aliases.insert(Arc::from(signal_name), aliases);
        }
        let res = SearchResult {
            separator_formula: invariant.clone(),
            assume_invariant,
            assert_invariant,
            signal_aliases: signal_aliases,
            input_cex: None,
            ..Default::default()
        };
        //formatted_invariant = invariant_string.replace("TOP.correctness.", "")
        //search_result.values = invariant.get_values();
        return res;
    }

    pub fn solve_through_set_cover<PredicateLikeType>(
        &self,
        this_teacher: &teacher::Teacher,
        basic_predicates: &[PredicateLikeType],
        current_disjunction: Option<&predicates::InvariantDisjunction>,
        min_objective: Option<f64>,
    ) -> Option<SearchResult>
    where
        PredicateLikeType: predicates::PredicateLike,
    {
        log::info!(
            "Solving through set cover with {} basic predicates",
            basic_predicates.len()
        );
        let mut ret_vec = utils::get_scoring_info_for_base_predicates(
            basic_predicates,
            this_teacher,
            &self.formula_score_weights,
            current_disjunction,
        );
        // for predicate in ret_vec.iter() {
        //     println!("Predicate {} score {:?}", predicate.invariant, predicate.score.score);
        // }
        ret_vec.retain_mut(|x| x.score.score.cex_only_score != predicates::ScoreResult::Unsat);
        let mut set_cover_solver = set_cover_solver_module::SetCoverSolver::new(
            this_teacher.clone(),
            self.formula_score_weights.clone(),
        );
        if let Some(invariant_disjunction) = set_cover_solver.solve(&ret_vec, min_objective) {
            let solver = thisSolver::new_from_teacher(&this_teacher);
            let new_disjunction =
                current_disjunction.map_or(invariant_disjunction.clone(), |current| {
                    let mut new_disjunction_inner = current.clone();
                    for inv in invariant_disjunction.disjunctions.iter() {
                        new_disjunction_inner.add_invariant(inv.clone());
                    }
                    new_disjunction_inner
                });
            let rescore = solver.score_invariant_disjunction(&new_disjunction);
            let formula =
                predicates::SeparatorFormula::InvariantDisjunction(new_disjunction.clone());
            // let rescore_objective = solver.calculate_invariant_objective(&invariant_disjunction, false,
            // &self.formula_score_weights);
            //TODO: Why is this buggy?
            //assert_eq!(rescore.cex_with_bex_allow_score.get_inner_or_zero(), scored_invariant.score.score.cex_with_bex_allow_score.get_inner_or_zero(), "score {:?} vs. score {:?}", rescore, scored_invariant.score.score);
            // println!("Outside Found invariant {} with score {:?} and objective {:?}", scored_invariant.invariant, rescore, rescore_objective);
            let mut res = self.formula_to_search_result(&formula);
            let num_fulfilled_bex = rescore.cex_and_bex_score.get_inner_or_zero() as usize
                - rescore.cex_only_score.get_inner_or_zero() as usize;
            res.bex_fulfilled_percentage = Some(
                ((num_fulfilled_bex as f64) / (this_teacher.get_total_num_bex() as f64) * 100.0)
                    as i64,
            );
            res.cex_fulfilled_percentage = Some(
                ((rescore.cex_only_score.get_inner_or_zero() as f64)
                    / (this_teacher.cex_traces.len() as f64)
                    * 100.0) as i64,
            );
            res.score = Some(rescore);
            res.objective = Some(solver.get_objective_from_separator_formula(
                &formula,
                false,
                &self.formula_score_weights,
            ));
            println!(
                "search result to string {}",
                serde_json::to_string_pretty(&res).unwrap()
            );
            // println!("invariant JG  string {}", &res.invariant);
            return Some(res);
        }
        return None;
    }

    pub fn search_from_teacher(
        &self,
        this_teacher: &teacher::Teacher,
        core_type: &data_types::general_data_types::CoreType,
        current_disjunction: Option<&predicates::InvariantDisjunction>,
        stage_name: &str,
        objective_lower_bound: Option<&predicates::InvariantObjective>,
        predicate_generation_config: &allowed_signal_config::PredicateGenerationConfig,
    ) -> Option<SearchResult> {
        log::info!("Starting search from teacher, collecting predicates");
        let all_potential_predicates: Vec<predicates::BasePredicateCandidate> =
            this_teacher.collect_predicates(None, &core_type, predicate_generation_config);
        log::debug!(
            "Creation of teacher done; All potential predicates len {}",
            all_potential_predicates.len()
        );
        log::debug!(
            "Teacher name to id {} entries",
            this_teacher.name_to_id.len()
        );
        let predicate_take_limits = constants::PREDICATE_TAKE_LIMITS.load(Ordering::Relaxed);
        let max_num_control = predicate_take_limits.control_predicates;
        log::debug!(
            "Using predicate-take limits in stage {}: {:?}",
            stage_name,
            predicate_take_limits
        );
        // let all_potential_predicates = utils::filter_basepredicate_list_invariant_and_teacher(&all_potential_predicates, &this_teacher);
        log::debug!(
            " Now scoring all {} potential predicates",
            all_potential_predicates.len()
        );
        for predicate in all_potential_predicates.iter() {
            log::debug!(
                "Scoring Predicate {} alias(es) {:?}",
                predicate,
                self.get_signal_aliases(&predicate.get_signal_names()[0])
            );
        }
        let mut scored_predicate_list: Vec<
            predicates::BasePredicateWithScoreAndObjective<predicates::BasePredicateCandidate>,
        > = utils::get_scoring_info_for_base_predicates(
            &all_potential_predicates,
            &this_teacher,
            &self.formula_score_weights,
            current_disjunction,
        );
        log::debug!(
            "Scoring of {:?} predicate sdone, now filtering",
            scored_predicate_list.len()
        );
        scored_predicate_list
            .retain_mut(|x| x.score.score.cex_only_score != predicates::ScoreResult::Unsat); // Remove predicates with negative objective
        log::info!(
            "Left with {:?} after removing non original-cex predicates",
            scored_predicate_list.len()
        );
        // let scored_predicate_list = utils::prefilter_dominated_scored_invariants(&scored_predicate_list)
        // let solver = thisSolver::new_from_teacher(&this_teacher);
        // let mut scored_predicate_list: Box<Vec<predicates::BasePredicateWithScoreAndObjective<predicates::BasePredicateCandidate>>> = Box::new(scored_predicate_list);
        scored_predicate_list.par_sort_by(|a, b| b.objective.partial_cmp(&a.objective).unwrap()); //Sort descending
        let mut control_predicates = 0;
        for (idx, predicate) in scored_predicate_list.iter().enumerate() {
            log::info!(
                "Predicate from teacher {} score {:?} objective {} position {}",
                predicate.predicate,
                predicate.score.score,
                predicate.objective.objective,
                idx
            );
            let signal_idx_set = predicate.predicate.base_predicate.get_signal_idx();
            for this_signal_idx in signal_idx_set.iter() {
                let signal_types = this_teacher
                    .get_signal_type_from_index(this_signal_idx)
                    .unwrap();
                if signal_types.contains(&data_types::general_data_types::SignalType::Control) {
                    control_predicates += 1;
                }
            }
        }
        log::info!(
            "Total control predicates in scored predicate list: {}",
            control_predicates
        );

        // panic!("Here");
        //this_teacher.calculate_values_per_signal();
        // log::info!("Skipping greedy and decision tree");
        // let greedy_invariant = decision_tree::next_best_algorithm(&scored_predicate_list, &this_teacher, &self.formula_score_weights);
        // if greedy_invariant.is_none() {
        //     println!("No next best invariant found");
        //     return None;
        // }
        // let next_best_invariant = greedy_invariant.unwrap();
        // let greedy_invariant_objective = solver.calculate_invariant_objective(&next_best_invariant.invariant, false,
        //     &self.formula_score_weights);
        // log::info!("calculated greedy invariant objective {:?}", greedy_invariant_objective);
        // let greedy_invariant_objective_upper_bound = solver.invariant_upper_bound(&next_best_invariant.invariant, &next_best_invariant.score.cover_info, &self.formula_score_weights);
        // log::info!("Done with greedy algorithm, got invariant {} score {:?} objective {:?} upper bound {}", next_best_invariant.invariant, next_best_invariant.score.score, greedy_invariant_objective, greedy_invariant_objective_upper_bound);
        // for predicate in next_best_invariant.invariant.predicate_set.predicates.iter() {
        //     let mut invariant = predicates::Invariant::new();
        //     invariant.add_predicate(predicate.clone());
        //     let score = solver.score_invariant_with_fulfilled_examples(&invariant);
        //     let scored_invariant = predicates::ScoredInvariantWithFulfilledExample {
        //         invariant: invariant.clone(),
        //         score: score.clone()
        //     };
        //     let objective_upper_bound = solver.invariant_upper_bound(&scored_invariant.invariant, &scored_invariant.score.cover_info, &self.formula_score_weights);
        //     log::info!("Predicate in greedy invariant {} objective upper bound {} score {:?}", predicate, objective_upper_bound, score.score);
        //     if objective_upper_bound < greedy_invariant_objective.objective {
        //         panic!("Predicate {} upper bound is worse than greedy invariant objective", predicate);
        //     }
        // }
        let tree = decision_tree::DecisionTree::new();
        let scored_invariant_list = scored_predicate_list
            .iter()
            .map(|p| predicates::InvariantWithScoreAndObjective {
                invariant: {
                    let mut inv = predicates::Invariant::new();
                    inv.add_predicate(p.predicate.to_base_predicate().clone());
                    inv
                },
                score: p.score.clone(),
                objective: p.objective.clone(),
            })
            .collect::<Vec<_>>();
        println!(
            "Scored invariant list length {}, now growing tree",
            scored_invariant_list.len()
        );
        // decision_tree::grow_decision_tree(&mut tree, &this_teacher, &scored_invariant_list, None, &self.formula_score_weights);
        println!("Grew tree, now optimizing");
        println!(
            "Done with tree algorithm, got invariant tree {}",
            tree.to_logic_formula()
        );
        let mut tree_predicates = HashSet::new();
        tree.get_used_predicate(&mut tree_predicates);
        //let scored_tree_predicates = utils::get_scoring_info_for_base_predicates(&tree_predicates.iter().map(|p| p.clone()).collect::<Vec<_>>(), &this_teacher, &self.formula_score_weights, current_disjunction);
        // let mut tree_invariants = Vec::new();
        // decision_tree::decision_tree_to_invariant_collection(&tree, &mut tree_invariants, None);
        // log::debug!("Invariant collection from tree"); //{:?}", tree_invariants);
        // tree.get_used_predicate(&mut collected_predicates);
        // let mut filtered_out_predicates = 0;
        // Filter based on lower bound
        // let before = scored_predicate_list.len();
        // scored_predicate_list = utils::filter_predicates_based_on_min_objective(scored_predicate_list, objective_lower_bound, this_teacher, &self.formula_score_weights);
        // let after = scored_predicate_list.len();
        // println!("Filtered scored predicate list based on objective lower bound from {} to {} lower bound {:?}", before, after, objective_lower_bound);
        // scored_predicate_list = utils::filter_scored_predicate_list_by_objective_lower_bound(scored_predicate_list, objective_lower_bound);

        let mut collected_predicates: HashSet<predicates::BasePredicateCandidate> = HashSet::new(); //next_best_invariant.invariant.predicate_set.predicates.iter().map(|x| x.clone()).collect();
        collected_predicates.extend(tree_predicates.iter().map(|p| p.to_candidate().clone()));
        #[derive(Default)]
        struct PredicateCollectionCounters {
            not_equal: usize,
            regular: usize,
            control: usize,
            signal_equal: usize,
        }

        let mut filtered_out_predicates = 0;
        let mut counters = PredicateCollectionCounters::default();
        let is_control_predicate_fn = |predicate: &predicates::BasePredicateCandidate,
                                       teacher: &teacher::Teacher,
                                       only_manually_added: bool|
         -> bool {
            let signal_idx_set = predicate.base_predicate.get_signal_idx();
            for this_signal_idx in signal_idx_set.iter() {
                let signal_types = teacher.get_signal_type_from_index(this_signal_idx).unwrap();
                if signal_types.contains(&data_types::general_data_types::SignalType::Control) {
                    if teacher.get_signal_length_from_index(this_signal_idx) == 1 {
                        let signal_info = teacher.get_signal_info_from_index(this_signal_idx);
                        if let Some(signal_info) = signal_info {
                            if only_manually_added
                                && signal_info
                                    .aliases
                                    .iter()
                                    .any(|alias| alias.to_lowercase().contains("added__"))
                            {
                                // println!("Found control predicate on added__ signal {}, counting as control predicate", predicate);
                                return true;
                            } else {
                                // println!("Found control predicate {}, counting as control predicate", predicate);
                                return true;
                            }
                        }
                        // return true;
                    }
                    return true;
                }
            }
            return false;
        };
        //scored_predicate_list.iter().filter(|p| p.predicate.get_operator() == predicates::Operator::NotEqual)
        for (idx, predicate) in scored_predicate_list.iter().enumerate() {
            log::info!(
                "Sort by objective Predicate from teacher {} score {:?} objective {:?} position {} took {} control predicates so far, {} not equal predicates, {} regular",
                predicate.predicate,
                predicate.score.score,
                predicate.objective,
                idx,
                counters.control,
                counters.not_equal,
                counters.regular
            );
            if predicate.score.score.cex_only_score == predicates::ScoreResult::Unsat {
                filtered_out_predicates += 1;
                continue;
            }

            if predicate.predicate.get_operator() == predicates::Operator::NotEqual {
                if counters.not_equal < predicate_take_limits.not_equal_predicates {
                    collected_predicates.insert(predicate.predicate.clone());
                    counters.not_equal += 1;
                } else {
                    log::debug!(
                        "Filtered out NotEqual predicate {} max already reached",
                        predicate.predicate
                    );
                }
            } else if is_control_predicate_fn(&predicate.predicate, &this_teacher, false)
                && counters.control < max_num_control
            {
                println!("Adding control predicate {}", predicate.predicate);
                counters.control += 1;
                collected_predicates.insert(predicate.predicate.clone());
            } else if counters.regular < predicate_take_limits.regular_predicates {
                counters.regular += 1;
                collected_predicates.insert(predicate.predicate.clone());
            } else if counters.signal_equal < predicate_take_limits.signal_equal_predicates
                && predicate.predicate.is_two_signal_equal()
            {
                counters.signal_equal += 1;
                collected_predicates.insert(predicate.predicate.clone());
            }
            if counters.signal_equal >= predicate_take_limits.signal_equal_predicates
                && counters.regular >= predicate_take_limits.regular_predicates
                && counters.not_equal >= predicate_take_limits.not_equal_predicates
                && counters.control >= max_num_control
            {
                break;
            }
        }
        let mut counters = PredicateCollectionCounters::default();
        let sort_by_key = |p: &predicates::BasePredicateWithScoreAndObjective<
            predicates::BasePredicateCandidate,
        >|
         -> usize {
            let s1 = p.score.cover_info.allowed_bex_states.len();
            let s2 = p.score.cover_info.blocked_cex_states.len();
            return s1 + s2;
        };
        scored_predicate_list
            .par_sort_by(|a, b| sort_by_key(b).partial_cmp(&sort_by_key(a)).unwrap()); //Sort descending
                                                                                       //scored_predicate_list.iter().filter(|p| p.predicate.get_operator() == predicates::Operator::NotEqual)
        for (idx, predicate) in scored_predicate_list.iter().enumerate() {
            let sorted_obj = sort_by_key(predicate);
            log::info!(
                "Sort by cex_only_score Predicate from teacher {} score {:?} objective {:?} key {} position {} took {} control predicates so far",
                predicate.predicate,
                predicate.score.score,
                predicate.objective,
                sorted_obj,
                idx,
                counters.control
            );
            if predicate.score.score.cex_only_score == predicates::ScoreResult::Unsat {
                filtered_out_predicates += 1;
                continue;
            }

            if predicate.predicate.get_operator() == predicates::Operator::NotEqual {
                if counters.not_equal < predicate_take_limits.not_equal_predicates {
                    collected_predicates.insert(predicate.predicate.clone());
                    counters.not_equal += 1;
                } else {
                    log::debug!(
                        "Filtered out NotEqual predicate {} max already reached",
                        predicate.predicate
                    );
                }
            } else if is_control_predicate_fn(&predicate.predicate, &this_teacher, true)
                && counters.control < max_num_control
            {
                counters.control += 1;
                collected_predicates.insert(predicate.predicate.clone());
            } else if counters.regular < predicate_take_limits.regular_predicates {
                counters.regular += 1;
                collected_predicates.insert(predicate.predicate.clone());
            }
            if counters.regular >= predicate_take_limits.regular_predicates
                && counters.not_equal >= predicate_take_limits.not_equal_predicates
                && counters.control >= max_num_control
            {
                break;
            }
        }
        log::debug!(
            "Filtered {:?} predicates left with {:?}, not equal num {:?} regular {:?}",
            filtered_out_predicates,
            collected_predicates.len(),
            counters.not_equal,
            counters.regular
        );
        log::info!("Collected {} control predicates", counters.control);
        let collected_predicates: Vec<predicates::BasePredicateCandidate> =
            collected_predicates.iter().map(|s| s.clone()).collect();
        constants::BENCHMARK_NUM_COLLECTED_PREDICATES
            .store(collected_predicates.len(), Ordering::Relaxed);
        //We need to filter again, as decision tree can introduce inverse of predicates.
        //So fo rinstance, decision tree can split on allowed predicate x == y, but then we would add x != y, whichw e do not want
        // let collected_predicates = utils::filter_basepredicate_list_invariant_and_teacher(&collected_predicates, &this_teacher);
        //for predicate in collected_predicates.iter() {
        //    println!("Collected predicate {:?}", predicate);
        //}
        // log::debug!("Collected predicates list len {:?} ", collected_predicates.len());
        // log::debug!("Before restricting signals, teacher has {} bex and {} cex traces", this_teacher.bex_samples.len(), this_teacher.cex_traces.len());
        // log::debug!("Before filtering for cex-only predicates, we have {} predicates", collected_predicates.len());
        // let mut scored_predicate_list: Vec<predicates::InvariantWithScoreAndObjective> = utils::get_covered_for_predicates(&collected_predicates, &this_teacher, &self.formula_score_weights, current_disjunction);
        // scored_predicate_list.retain_mut(|x| x.score.score.cex_only_score != predicates::ScoreResult::Unsat);
        // log::debug!("After filtering for cex-only predicates, we have {} predicates", scored_predicate_list.len());
        // let mut collected_signals: Vec<Arc<str>> = Vec::new();
        // let mut collected_predicates: Vec<predicates::BasePredicate> = Vec::new();
        // for (idx, predicate) in scored_predicate_list.iter().enumerate() {
        //     log::debug!("Predicate from teacher {} score {:?} postion {}", predicate.invariant, predicate.score.score, idx);
        //     if predicate.score.score.cex_only_score != predicates::ScoreResult::Unsat {
        //         collected_predicates.push(predicate.invariant.predicate_set.predicates[0].clone());
        //         collected_signals.extend(predicate.invariant.get_signal_names());
        //     } else {
        //         log::debug!("Filtered out predicate {} with score {:?}", predicate.invariant, predicate.score.score);
        //         filtered_out_predicates += 1;

        //     }
        // }
        for predicate in collected_predicates.iter() {
            println!("Final collected predicate {} ", predicate);
        }
        let res = match objective_lower_bound {
            Some(min_ob) => self.solve_through_set_cover(
                &this_teacher,
                &collected_predicates,
                current_disjunction,
                Some(min_ob.objective),
            ),
            None => self.solve_through_set_cover(
                &this_teacher,
                &collected_predicates,
                current_disjunction,
                None,
            ),
        };
        return res;
    }

    // pub fn search_separator_from_signals(&self, signal_list_arg: Vec<Arc<str>>, core_type: data_types::CoreType, min_treshold_percent: Option<usize>) -> Option<SearchResult>{
    //     println!("Creating new teacher");
    //     //let mut this_teacher: teacher::Teacher = teacher::get_samples_from_waveforms(&self.cex_waveforms, &self.bex_waveforms,&self.original_cex_waveform, signal_list_arg.clone(), &core_type);
    //     let this_teacher = teacher::new_teacher_from_restricted_signals(&self.main_teacher, &signal_list_arg, &core_type);

    // }
}
