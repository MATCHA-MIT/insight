use crate::architecture_checker;
use crate::constants;
use crate::data_types;
use crate::data_types::signal_filters;
use crate::predicates;
use crate::predicates::ScoredInvariant;
use crate::smt_solver;
use crate::solver::Solver;
use crate::teacher;
use crate::waveform;
use indicatif::ParallelProgressIterator;
use rayon::prelude::*;
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::Ordering;

/*
pub fn take_top_n_percent(predicates: &Vec<predicates::ScoredInvariantWithFulfilledExample>, percent: f64) -> Vec<predicates::ScoredInvariantWithFulfilledExample> {
    let mut sorted_predicates = predicates.clone();
    sorted_predicates.sort_by(|a, b| a.score.cmp(&b.score));
    let num_to_take = (sorted_predicates.len() as f64 * percent).round() as usize;
    return sorted_predicates.into_iter().take(num_to_take).collect();
}
     */

pub fn filter_predicates_based_on_min_objective<T>(
    scored_predicates: Vec<predicates::BasePredicateWithScoreAndObjective<T>>,
    min_objective: Option<&predicates::InvariantObjective>,
    this_teacher: &teacher::Teacher,
    formula_score_weights: &data_types::general_data_types::FormulaScoreWeights,
) -> Vec<predicates::BasePredicateWithScoreAndObjective<T>>
where
    T: predicates::PredicateLike,
{
    //Left with 165 after removing upper bound <= objective Some(InvariantObjective { objective: 1132.9577506564588 })
    if min_objective.is_none() {
        return scored_predicates;
    }
    let min_objective = min_objective.unwrap();
    let solver = smt_solver::SMTSolver::new_from_teacher(this_teacher);
    // let mut blocked_combinations = HashSet::new();
    // let blocked_combinations_lock = std::sync::Mutex::new(&mut blocked_combinations);

    let ret_vec: Vec<_> = (0..scored_predicates.len())
        .into_par_iter()
        .progress()
        .filter_map(|i| {
            let predicate_i = &scored_predicates[i];
            let current_objective = &predicate_i.objective;

            if current_objective.objective >= min_objective.objective {
                println!(
                    "Predicate {} has objective {} > min objective {}",
                    predicate_i.predicate.to_invariant(),
                    current_objective.objective,
                    min_objective.objective
                );

                return Some(predicate_i.clone());
            }

            let invariant = predicate_i.predicate.to_invariant();
            let upper_bound = solver.invariant_upper_bound(
                &invariant,
                &predicate_i.score.cover_info,
                formula_score_weights,
            );

            if upper_bound < min_objective.objective {
                return None;
            }
            println!(
                "Predicate {} has objective {}, upper bound {}, min objective {}",
                invariant, current_objective.objective, upper_bound, min_objective.objective
            );

            return Some(predicate_i.clone());
        })
        .collect();
    // println!("Blocked combinations: {:?}", blocked_combinations.len());
    ret_vec
}

pub fn filter_basepredicate_list_invariant_and_teacher(
    predicates: &Vec<predicates::BasePredicate>,
    this_teacher: &teacher::Teacher,
) -> Vec<predicates::BasePredicate> {
    let mut filtered_predicates = Vec::new();
    for predicate in predicates.iter() {
        let mut new_invariant = predicates::Invariant::new();
        new_invariant.add_predicate(predicate.clone());
        let pass_architecture_check =
            architecture_checker::architectural_invariant_check(&new_invariant, &this_teacher);
        if pass_architecture_check == false {
            println!(
                "Filtering out {} because of architecture check",
                new_invariant
            );
            continue;
        } else {
            // println!("Keeping {} because of architecture check", new_invariant);
        }
        filtered_predicates.push(predicate.clone());
    }
    return filtered_predicates;
}

pub fn filter_predicate_list_wrt_invariant_and_teacher(
    predicates: &Vec<predicates::InvariantWithScoreAndObjective>,
    this_teacher: &teacher::Teacher,
    maybe_invariant: &Option<predicates::Invariant>,
) -> Box<Vec<predicates::InvariantWithScoreAndObjective>> {
    let mut filtered_predicates = Box::new(Vec::new());
    for predicate in predicates.iter() {
        let mut new_invariant = match maybe_invariant {
            Some(inner_invariant) => inner_invariant.clone(),
            None => predicates::Invariant::new(),
        };
        if new_invariant.should_add_other_invariant(&predicate.invariant) == false {
            continue;
        }
        new_invariant = new_invariant.merge_invariant(&predicate.invariant);
        let pass_architecture_check =
            architecture_checker::architectural_invariant_check(&new_invariant, &this_teacher);
        if pass_architecture_check == false {
            continue;
        }
        filtered_predicates.push(predicate.clone());
    }
    return filtered_predicates;
}

pub fn score_invariant_list(
    invariants: &Vec<&predicates::Invariant>,
    this_teacher: &teacher::Teacher,
    formula_score_weights: &data_types::general_data_types::FormulaScoreWeights,
) -> Vec<predicates::InvariantWithScoreAndObjective> {
    println!(
        "Scoring {} invariants with {} states",
        invariants.len(),
        this_teacher.states.len()
    );
    let smt_solver = smt_solver::SMTSolver::new_from_teacher(&this_teacher);
    println!("Solving creation done");
    std::io::stdout().flush().unwrap();
    let this_par_iter = invariants.par_iter().progress();
    println!("This par_iter done");
    std::io::stdout().flush().unwrap();
    let ret_vec = this_par_iter
        .map(|i| {
            let score = smt_solver.score_invariant_with_fulfilled_examples(i);
            let objective = smt_solver.calculate_invariant_objective_from_covered_states(
                i,
                &score.cover_info.covered_states,
                true,
                formula_score_weights,
                false,
            );
            return predicates::InvariantWithScoreAndObjective {
                invariant: (*i).clone(),
                score: score,
                objective: predicates::InvariantObjective {
                    objective: objective,
                },
            };
        })
        .collect::<Vec<predicates::InvariantWithScoreAndObjective>>();
    println!("Scoring done");
    std::io::stdout().flush().unwrap();
    return ret_vec;
}

pub fn get_scoring_info_for_base_predicates<T>(
    predicates: &[T],
    this_teacher: &teacher::Teacher,
    formula_score_weights: &data_types::general_data_types::FormulaScoreWeights,
    current_disjunction: Option<&predicates::InvariantDisjunction>,
) -> Vec<predicates::BasePredicateWithScoreAndObjective<T>>
where
    T: predicates::PredicateLike,
{
    let smt_solver = smt_solver::SMTSolver::new_from_teacher(&this_teacher);
    let base_disjunction = match current_disjunction {
        Some(d) => d.clone(),
        None => predicates::InvariantDisjunction::new(),
    };
    predicates
        .par_iter()
        .progress()
        .map(|p| {
            let base_predicate = p.to_base_predicate();
            let mut invariant = predicates::Invariant::new();
            invariant.add_predicate(base_predicate.clone());
            let mut this_disjunction = base_disjunction.clone();
            this_disjunction.add_invariant(invariant.clone());
            let score = smt_solver.score_disjunction_with_fulfilled_examples(&this_disjunction);
            let this_objective = smt_solver.get_objective_from_separator_formula(
                &predicates::SeparatorFormula::InvariantDisjunction(this_disjunction),
                true,
                formula_score_weights,
            );
            predicates::BasePredicateWithScoreAndObjective::<T> {
                predicate: p.clone(),
                score,
                objective: this_objective,
            }
        })
        .collect()
}

pub fn get_node_gini_impurity(num_cex_samples: usize, num_bex_samples: usize) -> f64 {
    let total_samples = num_bex_samples + num_cex_samples;
    if total_samples == 0 {
        return 0.0;
    }
    let cex_prob = num_cex_samples as f64 / total_samples as f64;
    let bex_prob = num_bex_samples as f64 / total_samples as f64;
    let cex_impurity = cex_prob * (1.0 - cex_prob);
    let bex_impurity = bex_prob * (1.0 - bex_prob);
    let total_impurity = cex_impurity + bex_impurity;
    total_impurity
}

pub fn is_dominated_by(
    this_invariant: &predicates::InvariantWithScoreAndObjective,
    other_invariant: &predicates::InvariantWithScoreAndObjective,
) -> bool {
    let is_strict_superset_cex = other_invariant
        .score
        .cover_info
        .blocked_cex_states
        .is_strict_superset(&this_invariant.score.cover_info.blocked_cex_states);
    let is_strict_superset_bex = other_invariant
        .score
        .cover_info
        .allowed_bex_states
        .is_strict_superset(&this_invariant.score.cover_info.allowed_bex_states);
    let is_superset_cex = other_invariant
        .score
        .cover_info
        .blocked_cex_states
        .is_superset(&this_invariant.score.cover_info.blocked_cex_states);
    let is_superset_bex = other_invariant
        .score
        .cover_info
        .allowed_bex_states
        .is_superset(&this_invariant.score.cover_info.allowed_bex_states);
    return (is_strict_superset_cex && is_superset_bex)
        || (is_strict_superset_bex && is_superset_cex);
}

pub fn take_first_n_values_per_predicate(
    predicates: Vec<(
        predicates::BaseSignalToConstFormula,
        Vec<(Option<i64>, predicates::PriorityScores)>,
    )>,
) -> Vec<(
    predicates::BaseSignalToConstFormula,
    Vec<(Option<i64>, predicates::PriorityScores)>,
)> {
    //For each predicate in predicates, take the first 10 values in the vector, sorted by priorityscores (highest priority scores first).
    let predicate_limits = constants::PREDICATE_TAKE_LIMITS.load(Ordering::Relaxed);
    let mut return_predicates = Vec::new();
    for (base_formula, values_and_scores) in predicates.iter() {
        let mut sorted_values_and_scores = values_and_scores.clone();
        sorted_values_and_scores.sort_by(|a, b| b.1.cmp(&a.1));
        let mut new_values_and_scores = Vec::new();
        for (value, score) in sorted_values_and_scores.iter() {
            if new_values_and_scores.len() < predicate_limits.regular_predicates {
                new_values_and_scores.push((value.clone(), score.clone()));
            } else {
                break;
            }
        }
        return_predicates.push((base_formula.clone(), new_values_and_scores));
    }
    return_predicates
}

pub fn collect_predicates_for_threshold_from_base_predicate_list(
    basic_predicate_scores: &Vec<(predicates::BasePredicate, predicates::PriorityScores)>,
    this_threshold: i64,
) -> (
    Vec<predicates::BasePredicate>,
    Vec<predicates::ScoredInvariant>,
    Vec<predicates::ScoredInvariant>,
) {
    let mut start_of_heap: Vec<ScoredInvariant> = Vec::new();
    let mut return_predicates = Vec::new();
    let mut return_scored_invariants = Vec::new();
    for (base_predicate, score) in basic_predicate_scores.iter() {
        let mut this_invariant = predicates::Invariant::new();
        let this_predicate = base_predicate.clone();
        this_invariant.add_predicate(this_predicate.clone());
        let scored_invariant = predicates::ScoredInvariant {
            invariant: this_invariant,
            score: score.clone(),
        };
        if let predicates::ScoreResult::Sat(inner_score) = score.cex_only_score {
            if inner_score >= this_threshold.into() {
                start_of_heap.push(scored_invariant.clone());
                return_predicates.push(this_predicate.clone());
            }
        }
        return_scored_invariants.push(scored_invariant.clone());
    }
    return (return_predicates, return_scored_invariants, start_of_heap);
}

pub fn get_separator_formula_from_json_file(
    json_path: &str,
) -> Result<predicates::SeparatorFormula, Box<dyn Error + Send + Sync>> {
    let file = std::fs::File::open(json_path)?;
    let reader = std::io::BufReader::new(file);
    //"Parse the "separator_formula" key if possible, otherwise fallback to "invariant"
    let separator_formula: predicates::SeparatorFormula = {
        if let Ok(json_value) = serde_json::from_reader::<_, serde_json::Value>(reader) {
            if let Some(sep_value) = json_value.get("separator_formula") {
                if let Ok(inner_separator) = serde_json::from_value(sep_value.clone()) {
                    inner_separator
                } else {
                    panic!(
                        "Failed to extract separator_formula from json file {}",
                        json_path
                    );
                }
            } else if let Some(inv_value) = json_value.get("invariant") {
                if let Ok(inner_invariant) = serde_json::from_value(inv_value.clone()) {
                    predicates::SeparatorFormula::Invariant(inner_invariant)
                } else {
                    panic!("Failed to extract invariant from json file {}", json_path);
                }
            } else {
                panic!(
                    "No separator_formula or invariant field in json file {}",
                    json_path
                );
            }
        } else {
            panic!("Failed to parse json file {}", json_path);
        }
    };
    Ok(separator_formula)
    // let invariant: predicates::Invariant = {
    //     if let Ok(json_value) = serde_json::from_reader::<_, serde_json::Value>(reader) {
    //         if let Some(inv_value) = json_value.get("invariant") {
    //             if let Ok(inner_invariant) = serde_json::from_value(inv_value.clone()) {
    //                 inner_invariant
    //             } else {
    //                 panic!("Failed to extract invariant from json file {}", json_path);
    //             }
    //         } else {
    //             panic!("No invariant field in json file {}", json_path);
    //         }
    //     }
    //     else {
    //         panic!("Failed to parse json file {}", json_path);
    //     }
    // };
    // return Ok(invariant);
}

pub fn load_json_and_check_on_waveform(
    json_path: &str,
    waveform_path: &str,
    clock_signal: &str,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    // let file = std::fs::File::open(json_path)?;
    //             // let file = File::open(entry.path()).unwrap();
    //     let reader = std::io::BufReader::new(file);
    //     // Only parse the "invariant" key
    //     let invariant: predicates::Invariant = {
    //         if let Ok(json_value) = serde_json::from_reader::<_, serde_json::Value>(reader) {
    //             if let Some(inv_value) = json_value.get("invariant") {
    //                 if let Ok(inner_invariant) = serde_json::from_value(inv_value.clone()) {
    //                     inner_invariant
    //                 } else {
    //                     panic!("Failed to extract invariant from json file {}", json_path);
    //                 }
    //             } else {
    //                 panic!("No invariant field in json file {}", json_path);
    //             }
    //         }
    //         else {
    //             panic!("Failed to parse json file {}", json_path);
    //         }
    // };
    // let reader = std::io::BufReader::new(file);
    // let search_result: serde_json::Value = serde_json::from_reader(reader).unwrap();
    // let serde_value_invariant = search_result.get("invariant").unwrap_or_else(|| panic!("No invariant field in json"));
    // let invariant: predicates::Invariant = serde_json::from_value(serde_value_invariant.clone()).unwrap_or_else(|_| panic!("Failed to extract invariant"));
    // let filter_signal_list: HashSet<u64, data_types::DefaultScalarHasher> = invariant.get_relevant_signal_idx().into_iter().collect::<HashSet<u64, data_types::DefaultScalarHasher>>();
    let separator_formula = get_separator_formula_from_json_file(json_path)?;
    let signal_filter = {
        let signal_str_list = separator_formula.get_relevant_signals();
        // println!("Signals relevant for invariant {:?}", signal_str_list);
        let regexes: Vec<regex::Regex> = signal_str_list
            .iter()
            .filter_map(|s| regex::Regex::new(&format!("^{}$", regex::escape(s))).ok())
            .collect();
        signal_filters::SignalFilter::RegexFilter(regexes)
    };
    // println!("Using signal filter: {:?} for invariant {}", signal_filter, separator_formula);
    let all_filters = signal_filters::SignalFilters {
        filters: vec![signal_filter.clone()],
    };
    // } else {
    //     waveform::SignalFilter::SignalIdxFilter(filter_signal_list.clone())
    // };
    return check_on_waveform(
        waveform_path,
        &separator_formula,
        clock_signal,
        false,
        Some(&all_filters),
    );
}

///
/// Short_circuit: If true, return as soon we know that the waveform fulfills the invariant - exact cycle does not matter.
pub fn check_on_waveform(
    waveform_path: &str,
    separator: &predicates::SeparatorFormula,
    clock_signal: &str,
    short_circuit: bool,
    signal_filter: Option<&signal_filters::SignalFilters>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    // println!("Checking invariant {} on waveform {}", separator, waveform_path);
    let waveform = match waveform::WaveForm::load_waveform_and_cycle_map(
        waveform_path,
        clock_signal,
        signal_filter,
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error loading waveform: {}", e);
            panic!("Error loading waveform: {}", e);
        }
    };
    //println!("Checking waveform {}, value of sodor_core.core.d.imm_s, {:?}, opcode {:?}",waveform_path, waveform.get_signal_value_at_cycle("TOP.correctness.sodor_core.core.d.imm_s", 0), waveform.get_signal_value_at_cycle("TOP.correctness.sodor_core.core.d.opcode", 0));
    if let Some(res) = smt_solver::check_for_waveform(&waveform, &separator, short_circuit, true) {
        println!("Checking waveform {}", waveform_path);
        for fulfilled_cycle in res.iter() {
            println!("Fulfilled cycle: {}", fulfilled_cycle);
        }
        return Ok(true);
    } else {
        //println!("No cycles fulfilled");
        return Ok(false);
    }
}

pub fn check_any_separator_formula_fulfilled_on_waveform(
    waveform_path: &str,
    invariants: &Vec<&predicates::SeparatorFormula>,
    clock_signal: &str,
    short_circuit: bool,
    signal_filter: Option<&signal_filters::SignalFilters>,
) -> Option<usize> {
    //let start_time = std::time::Instant::now();
    let waveform = match waveform::WaveForm::load_waveform_and_cycle_map(
        waveform_path,
        clock_signal,
        signal_filter,
    ) {
        Ok(w) => w,
        Err(e) => {
            panic!("Error loading waveform: {}", e);
            //return Err(e);
        }
    };
    //let elapsed_time = start_time.elapsed();
    // println!("Time taken to load waveform: {:?}", elapsed_time);
    for (idx, invariant) in invariants.iter().enumerate() {
        //let start_time = std::time::Instant::now();
        let maybe_res = smt_solver::check_for_waveform(&waveform, invariant, short_circuit, true);
        //let elapsed_time = start_time.elapsed();
        //println!("Time taken to check invariant {}: {:?}", invariant, elapsed_time);
        if let Some(res) = maybe_res {
            if !short_circuit {
                println!(
                    "Invariant {} fulfilled on waveform {}",
                    invariant, waveform_path
                );
                for fulfilled_cycle in res.iter() {
                    println!("Fulfilled cycle: {}", fulfilled_cycle);
                }
            }
            return Some(idx);
        } else {
            // println!("Invariant {} not fulfilled on waveform {}", invariant, waveform_path);
        }
    }
    return None;
}

pub fn check_cex_items_against_invariants<'a>(
    cex_items: &'a Vec<data_types::general_data_types::FuzzerDataPoint>,
    invariants: &[predicates::SeparatorFormula],
    short_circuit: bool,
) -> Vec<(
    &'a data_types::general_data_types::FuzzerDataPoint,
    Vec<usize>,
)> {
    // use std::sync::Arc;

    let signal_filter_regex = invariants
        .iter()
        .flat_map(|inv| inv.get_relevant_signals())
        .map(|s| regex::Regex::new(&format!("^{}$", regex::escape(&s))).unwrap())
        .collect::<Vec<regex::Regex>>();
    let signal_filter = signal_filters::SignalFilter::RegexFilter(signal_filter_regex);

    let progress_bar = indicatif::ProgressBar::new(cex_items.len() as u64);
    progress_bar.set_style(indicatif::ProgressStyle::with_template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) - ETA: {eta_precise}").unwrap()
        .progress_chars("##-"));
    if cex_items.is_empty() {
        progress_bar.finish();
        return Vec::new();
    }
    let load_some_waveform = waveform::WaveForm::load_waveform_and_cycle_map(
        &cex_items[0].waveform_path,
        &"TOP.correctness.clk",
        Some(&signal_filters::SignalFilters {
            filters: vec![signal_filter.clone()],
        }),
    )
    .unwrap();
    let signal_idx_list = load_some_waveform
        .name_to_code
        .values()
        .cloned()
        .collect::<HashSet<u64, data_types::general_data_types::DefaultScalarHasher>>();
    // println!("Signal idx list used for filtering: {:?}", signal_idx_list);
    let signal_idx_filter = signal_filters::SignalFilter::StrictSignalIdxFilter(signal_idx_list);
    let filters = signal_filters::SignalFilters {
        filters: vec![signal_idx_filter.clone()],
    };
    // let progress_bar = Arc::new(progress_bar);
    let mut new_invariants_list = Vec::new();
    for inv in invariants.iter() {
        let mut invariant = inv.clone();
        teacher::fill_in_indexes_for_formula_and_waveform(&mut invariant, &load_some_waveform)
            .unwrap();
        new_invariants_list.push(invariant);
    }

    let results = cex_items
        .par_iter()
        .progress_with(progress_bar.clone())
        .map(|cex_item| {
            // println!("Checking CEX item: {:?}", cex_item);
            let mut this_cex_res: Vec<usize> = Vec::new();

            let waveform = match waveform::WaveForm::load_waveform_and_cycle_map(
                &cex_item.waveform_path,
                &"TOP.correctness.clk",
                Some(&filters),
            ) {
                Ok(w) => w,
                Err(e) => {
                    println!("Error loading waveform {}: {}", cex_item.waveform_path, e);
                    panic!("Error loading waveform {}: {}", cex_item.waveform_path, e);
                }
            };

            for (idx, invariant) in new_invariants_list.iter().enumerate() {
                if let Some(_) = smt_solver::check_for_waveform(&waveform, invariant, true, false) {
                    this_cex_res.push(idx);
                    if short_circuit {
                        break;
                    }
                }
            }
            // println!("CEX item {:?} satisfied invariants at indices {:?}", cex_item, this_cex_res);
            (cex_item, this_cex_res)
        })
        .collect::<Vec<_>>();

    progress_bar.finish();

    results
}

// pub fn is_control_signal(signal_name: &str, signal_length: usize) -> bool {
//     if signal_length <= constants::MAX_CONTROL_SIGNAL_LENGTH || signal_name.contains("inst") || signal_name.contains("addr") || signal_name.contains("address") || signal_name.ends_with("imm_i") {
//         true
//     } else {
//         false
//     }
// }

pub fn signal_passes_regex(
    signal_name: &str,
    include_regex: &Option<Vec<regex::Regex>>,
    exclude_regex: &Option<Vec<regex::Regex>>,
) -> bool {
    let mut passes_some_include = false;
    let mut passes_some_exclude = false;
    if let Some(include) = include_regex {
        for regex in include.iter() {
            if regex.is_match(signal_name) {
                passes_some_include = true;
                break;
            }
        }
    }
    if let Some(exclude) = exclude_regex {
        for regex in exclude.iter() {
            if regex.is_match(signal_name) {
                passes_some_exclude = true;
                break;
            }
        }
    }
    if passes_some_include && !passes_some_exclude {
        return true;
    } else {
        return false;
    }
}

pub fn signal_passes_filter(
    signal_name: &str,
    _signal_length: usize,
    include_regex: &Option<Vec<regex::Regex>>,
    exclude_regex: &Option<Vec<regex::Regex>>,
) -> bool {
    // if signal_name == constants::INCORRECTNESS_SIGNAL || signal_name == constants::MISMATCH_INSTRUCTION_DUT_CORE_SIGNAL ||
    //    signal_name == constants::MISMATCH_INSTRUCTION_REF_CORE_SIGNAL || signal_name == constants::COUNTER_SIGNAL {
    //     return true;
    // }
    // if !(is_control_signal(signal_name, signal_length)) {
    //     return false;
    // }
    return signal_passes_regex(signal_name, include_regex, exclude_regex);
}

pub fn load_separator_formulas_from_directory(
    invariants_directory: &str,
) -> Vec<predicates::SeparatorFormula> {
    let mut invariants = Vec::new();
    for entry in std::fs::read_dir(invariants_directory).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
            let separator =
                get_separator_formula_from_json_file(entry.path().to_str().unwrap()).unwrap();
            invariants.push(separator);
        }
    }
    invariants
}

pub fn load_invariant_from_directory(invariants_directory: &str) -> Vec<predicates::Invariant> {
    let mut invariants = Vec::new();
    for entry in std::fs::read_dir(invariants_directory).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
            let file = File::open(entry.path()).unwrap();
            let reader = std::io::BufReader::new(file);
            // Only parse the "invariant" key
            if let Ok(json_value) = serde_json::from_reader::<_, serde_json::Value>(reader) {
                if let Some(inv_value) = json_value.get("invariant") {
                    if let Ok(invariant) = serde_json::from_value(inv_value.clone()) {
                        invariants.push(invariant);
                    }
                }
            }
        }
    }
    invariants
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[test]
//     fn test_is_control_signal() {
//         assert!(is_control_signal("correctness.opcode", 7));
//         assert!(is_control_signal("addr", 15));
//         assert!(is_control_signal("rd", 5));
//     }
// }
