use crate::predicates::PredicateLike;
use crate::constants as constants_module;
use crate::costs;
use crate::data_types;
use crate::data_types::general_data_types;
use crate::predicates;
use crate::set_cover_heuristic;
use crate::set_cover_instance;
use crate::set_cover_preprocessing;
use crate::smt_solver;
use crate::solver::Solver;
use crate::teacher;
use chrono::Local;
use grb;
use grb::expr::LinExpr;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::BuildHasher;
use std::sync::atomic::Ordering;

//    //https://people.idsia.ch/~grandoni/Pubblicazioni/CGLMPS11fsttcs.pdf might help

macro_rules! format_truncated {
    ($($arg:tt)*) => {{
        let s = format!($($arg)*);
        if s.len() > 254 {
            s[..254].to_string()
        } else {
            s
        }
    }};
}

pub struct SetCoverSolver {
    teacher: teacher::Teacher,
    formula_score_weights: data_types::general_data_types::FormulaScoreWeights,
}

impl SetCoverSolver {
    pub fn new(teacher: teacher::Teacher, formula_score_weights: data_types::general_data_types::FormulaScoreWeights) -> Self {
        SetCoverSolver { teacher, formula_score_weights }
    }


    pub fn add_architecture_constraints<H>(
        &self,
        scored_predicates: &Vec<&set_cover_instance::SetCoverPredicate>,
        predicate_vars: &HashMap<usize, grb::Var, H>,
        model: &mut grb::Model,
    )
    where
        H: BuildHasher,
    {
        // Add architecture constraints here
        println!("Adding architecture constraints");
        let mut sum_not_equal_predicates = grb::expr::LinExpr::new();
        let mut sum_smaller_equal_predicates: LinExpr = grb::expr::LinExpr::new();
        let mut sum_greater_equal_predicates: LinExpr = grb::expr::LinExpr::new();
        let mut sum_equal_address_or_immediate_predicates: LinExpr = grb::expr::LinExpr::new();
        let mut immediate_groups = HashMap::new();
        immediate_groups.insert("funct3", Vec::new());
        immediate_groups.insert("funct7", Vec::new());
        immediate_groups.insert("imm_i", Vec::new());
        immediate_groups.insert("rest", Vec::new());
        let mut allowed_immediate_pair_group = HashMap::new();
        allowed_immediate_pair_group.insert("funct3", Vec::new());
        allowed_immediate_pair_group.insert("funct7", Vec::new());
        allowed_immediate_pair_group.insert("imm_i", Vec::new());
        allowed_immediate_pair_group.insert("rest", Vec::new());

        for predicate in scored_predicates.iter() {
            let predicate_id = predicate.id;
            if predicate.predicate.get_operator()
                == predicates::Operator::NotEqual
            {
                sum_not_equal_predicates.add_term(1.0, predicate_vars.get(&predicate_id).unwrap().clone());
            }
            if predicate.predicate.get_operator()
                == predicates::Operator::SmallerEqual
            {
                sum_smaller_equal_predicates.add_term(1.0, predicate_vars.get(&predicate_id).unwrap().clone());
            }
            if predicate.predicate.get_operator()
                == predicates::Operator::GreaterEqual
            {
                sum_greater_equal_predicates.add_term(1.0, predicate_vars.get(&predicate_id).unwrap().clone());
            }
            // if predicate.predicate.is_two_signal_equal() {
                
            // }
            
            if matches!(predicate.predicate.get_operator(), 
                predicates::Operator::Equal
                | predicates::Operator::NotEqual
                | predicates::Operator::SmallerEqual
                | predicates::Operator::GreaterEqual
            ) {
                let signal_idx_set = predicate.predicate.get_signal_idx();
                let signal_idx = signal_idx_set.iter().next().unwrap();
                let signal_info = self.teacher.get_signal_info_from_index(signal_idx).unwrap();
                if signal_info.signal_types.contains_any_type(&vec![    
                    data_types::general_data_types::SignalType::Address,
                    data_types::general_data_types::SignalType::Immediate,
                    data_types::general_data_types::SignalType::RegisterFileAddress,
                ]) {
                    sum_equal_address_or_immediate_predicates.add_term(1.0, predicate_vars.get(&predicate_id).unwrap().clone());
                    if predicate.predicate.is_two_signal_equal() {
                        continue;
                    }
                    let mut found_group = false;
                    for (alias_key, alias_vec) in immediate_groups.iter_mut() {
                        if signal_info.aliases.iter().any(|alias| alias.to_lowercase().contains(alias_key)) {
                            alias_vec.push(predicate_id);
                            found_group = true;
                        }
                    }
                    if !found_group {
                        immediate_groups.get_mut("rest").unwrap().push(predicate_id);
                    }
                    
                    // // sum_greater_equal_predicates.add_term(1.0, predicate_vars.get(&predicate_id).unwrap().clone());
                    // // sum_smaller_equal_predicates.add_term(1.0, predicate_vars.get(&predicate_id).unwrap().clone());
                    // non_control_to_value_predicate_ids.push(predicate_id);
                    println!("Found equal address or immediate predicate {:?}", predicate.predicate);
                }
                //  else if signal_info.aliases.iter().any(|alias| alias.to_lowercase().contains("funct3")) || 
                //    signal_info.aliases.iter().any(|alias| alias.to_lowercase().contains("csr_written_bitmap_reg_")) {
                //    let funct3_vec = allowed_immediate_pair_group.get_mut("funct3").unwrap();
                //     funct3_vec.push(predicate_id);
                // }
            }
            
            if predicate.predicate.get_operator() == predicates::Operator::Equal && predicate.predicate.get_signal_idx().len() == 1 {
                let signal_idx_set = predicate.predicate.get_signal_idx();
                let signal_idx = signal_idx_set.iter().next().unwrap();
                let signal_info = self.teacher.get_signal_info_from_index(signal_idx).unwrap();
                if signal_info.aliases.iter().any(|alias| alias.to_lowercase().contains("opcode")) || 
                   signal_info.aliases.iter().any(|alias| alias.to_lowercase().contains("csr_written_bitmap_reg_")) {
                    let base_predicate = &predicate.predicate.base_predicate.base_formula;
                    match base_predicate {
                        predicates::BaseFormula::SignalToConst(ref formula) => {
                            
                            if formula.get_value() == constants_module::OPCODE_CSR {
                                allowed_immediate_pair_group.get_mut("funct3").unwrap().push(predicate_id);
                                allowed_immediate_pair_group.get_mut("imm_i").unwrap().push(predicate_id);
                            } else if self.teacher.signal_alias_contains_string(formula.signal_idx.unwrap(),"csr_written_bitmap_reg_"){
                                if formula.get_value() == 1 {
                                    println!("Found opcode written_bitmap_reg predicate {:?}", predicate.predicate);
                                    allowed_immediate_pair_group.get_mut("funct3").unwrap().push(predicate_id);
                                    allowed_immediate_pair_group.get_mut("imm_i").unwrap().push(predicate_id);
                                }
                            } else if matches!(formula.get_value(),
                                constants_module::OPCODE_RTYPE
                                | constants_module::OPCODE_ITYPE_ARITHMETIC
                                | constants_module::OPCODE_STYPE
                                | constants_module::OPCODE_BTYPE
                            ) {
                                allowed_immediate_pair_group.get_mut("funct3").unwrap().push(predicate_id);
                                if formula.get_value() == constants_module::OPCODE_RTYPE {
                                    allowed_immediate_pair_group.get_mut("funct7").unwrap().push(predicate_id);
                                }
                            }
                        },
                        _ => {},
                    }
                }
            }
            // if predicate.predicate.predicate_set.predicates[0].get_signal_idx().len() == 1 {
            //     let signal_idx = predicate.predicate.predicate_set.predicates[0].get_signal_idx().iter().next().unwrap().clone();
            //     let entry = max_const_predicate_per_signal
            //         .entry(signal_idx)
            //         .or_insert_with(|| grb::expr::LinExpr::new());
            //     entry.add_term(1.0, predicate_vars.get(&predicate_id).unwrap().clone());
            // }
        }
        for (match_group_key, predicate_ids) in immediate_groups.iter() {
            let allowed_predicate_ids = allowed_immediate_pair_group.get(match_group_key).unwrap();
            for predicate_id in predicate_ids.iter() {
                model
                    .add_constr(
                        "Immediate group pairing constraints",
                        grb::c!(
                            predicate_vars.get(predicate_id).unwrap()
                                <= allowed_predicate_ids
                                    .iter()
                                    .map(|allowed_id| predicate_vars.get(allowed_id).unwrap().clone())
                                    .fold(grb::expr::LinExpr::new(), |mut acc, var| {
                                        acc.add_term(1.0, var);
                                        acc
                                    })
                        ),
                    )
                    .unwrap();
            }
            println!("Immediate group {} has {} predicates, match predicates {}", match_group_key, predicate_ids.len(), allowed_predicate_ids.len());
        }
        // for non_control_to_value_predicate_id in non_control_to_value_predicate_ids.iter() {
        //     model
        //         .add_constr(
        //             "Non-control to value predicates only if opcode CSR predicate selected",
        //             grb::c!(
        //                 predicate_vars.get(non_control_to_value_predicate_id).unwrap()
        //                     <= opcode_predicate_ids
        //                         .iter()
        //                         .map(|opcode_id| predicate_vars.get(opcode_id).unwrap().clone())
        //                         .fold(grb::expr::LinExpr::new(), |mut acc, var| {
        //                             acc.add_term(1.0, var);
        //                             acc
        //                         })
        //             ),
        //         )
        //         .unwrap();
        // }
        // for (_signal_idx, expr) in max_const_predicate_per_signal.into_iter() {
        //     model
        //         .add_constr(
        //             "Limited Num Const Predicates per Signal",
        //             grb::c!(expr <= 2.2),
        //         )
        //         .unwrap();
        // }

        model.add_constr(
            "Limited Num Equal Address or Immediate",
            grb::c!(sum_equal_address_or_immediate_predicates <= 1),
        )
        .unwrap();
        model
            .add_constr(
                "Limited Num Smaller Equal",
                grb::c!(sum_smaller_equal_predicates <= 1),
            )
            .unwrap();
        model
            .add_constr(
                "Limited Num Greater Equal",
                grb::c!(sum_greater_equal_predicates <= 1),
            )
            .unwrap();
        model
            .add_constr(
                "Limited Num Not Equal",
                grb::c!(
                    sum_not_equal_predicates <= 1
                ),
            )
            .unwrap();
    }


    pub fn solve<T>(
        &mut self,
        scored_predicates: &Vec<predicates::BasePredicateWithScoreAndObjective<T>>,
        min_objective: Option<f64>
    ) -> Option<predicates::InvariantDisjunction> 
    where 
        T: predicates::PredicateLike,
    {
        //We want to maximize the set of the intersection of the allowed CEX samples
        //while minimizing the set of the intersection of the allowed BEX samples
        //Construct an ILP as follows
        //For each cex, create a variable
        //For each bex, create a variable
        println!("Now starting set cover solver");
        // let mut ret_vec = utils::score_invariant_list(
        //     &scored_predicates
        //         .iter()
        //         .map(|sp| &(sp.invariant))
        //         .collect::<Vec<&predicates::Invariant>>(),
        //     &self.teacher,
        //     &self.formula_score_weights
        // );
        // scored_predicates.retain_mut(|x| x.score.score.cex_only_score != predicates::ScoreResult::Unsat);
        self.create_model_and_solve(scored_predicates, min_objective)
    }

    pub fn create_model_and_solve<T>(
        &self,
        scored_predicates_arg: &Vec<predicates::BasePredicateWithScoreAndObjective<T>>,
        min_objective: Option<f64>,
    ) -> Option<predicates::InvariantDisjunction> 
    where 
        T: predicates::PredicateLike,
    {
        println!("now solving with weights {:?}", self.formula_score_weights);
        let mut set_cover_instance  = set_cover_instance::SetCoverInstance::new_from_teacher_and_predicates(&self.teacher, scored_predicates_arg,
            self.formula_score_weights.clone());

        let _required_ids: HashSet<usize> = set_cover_instance
            .cex_traces
            .iter()
            .filter(|(_, trace)| trace.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex))
            .map(|(id, _)| *id)
            .collect();
        let mut predicate_per_cycle_map = HashMap::new();
        let mut none_predicates_list = Vec::new();
        for (id,predicate) in set_cover_instance.predicates.iter() {
            match &predicate.predicate.only_in_cycles {
                Some(cycles) => {
                    for cycle in cycles.iter() {
                        predicate_per_cycle_map.entry(cycle).or_insert(Vec::new()).push(id);
                    }
                },
                None => {
                    none_predicates_list.push(*id);
                }
            }
        }
        // let mut global_best_solution = None;
        // let mut global_best_objective = std::f64::MIN;
        for (cycle, predicates_in_cycle) in predicate_per_cycle_map.iter_mut() {
            println!("Cycle {} has {} predicates and {} none predicates total predicates {}", cycle, predicates_in_cycle.len(), none_predicates_list.len(), set_cover_instance.predicates.len());
        }
            // let mut this_cycle_predicates = predicates_in_cycle.clone();
            // this_cycle_predicates.extend(none_predicates_list.iter().map(|id| id).collect::<Vec<&usize>>());
            // let mut this_set_cover_instance = set_cover_instance::SetCoverInstance {
            //     predicates: set_cover_instance.predicates.iter().filter(|(id,_)| this_cycle_predicates.contains(id)).map(|(id,p)| (*id,p.clone())).collect(),
            //     all_states: set_cover_instance.all_states.clone(),
            //     cex_traces: set_cover_instance.cex_traces.clone(),
            //     bex_samples: set_cover_instance.bex_samples.clone(),
            //     formula_score_weights: set_cover_instance.formula_score_weights.clone(),
            // };
            let this_set_cover_instance = set_cover_preprocessing::preprocess_instance(&mut set_cover_instance, min_objective);
            let set_cover_instance = if this_set_cover_instance.is_none() {
                log::warn!("Preprocessing return None, assuming instance infeasible");
                return None;
            } else {
                this_set_cover_instance.unwrap()
            };
            let solution = self.solve_set_cover_instance(&set_cover_instance, min_objective);
            // match solution {
            //     Some(ref sol) => {
            //         if sol.objective.objective > global_best_objective {
            //             global_best_objective = sol.objective.objective;
            //             global_best_solution = Some(predicates::InvariantDisjunction {
            //                 disjunctions: vec![sol.invariant.clone()],
            //             });
            //         }
            //     },
            //     None => {
            //         log::warn!("No solution found for cycle {}, continuing", cycle);
            //     }
            // }
            let final_disjunction = if solution.is_some() {
                let mut disjunction = predicates::InvariantDisjunction::new();
                disjunction.disjunctions.push(solution.unwrap().invariant);
                Some(disjunction)
            } else {
                None
            };
            final_disjunction
        // }
        // global_best_solution
    }

    /*
    eal    15m7.763s
user    120m17.793s
sys     17m14.890s
 */
/*
 real    14m29.100s
user    123m57.153s
sys     22m53.929s
with forced states
 */
/*
real    10m40.637s
user    116m32.300s
sys     14m57.026s
without force states and with concurrent solving
*/
    fn collect_cex_state_ids(
        set_cover_instance: &set_cover_instance::SetCoverInstance,
    ) -> Vec<u64> {
        let mut state_ids = HashSet::new();
        for (_trace_id, trace) in set_cover_instance.cex_traces.iter() {
            if trace.file_source == Some(general_data_types::WaveFormSource::OriginalCex) {
                println!("CEX trace {} has {} states", trace.from_path, trace.states.len());
                for (state_id, _samples) in trace.states.iter() {
                    state_ids.insert(*state_id as u64);
                }
                break;
                //We ignore all other OriginalCex (there cna be multiple), but 
                //we all of them definitely need to be covered, so fine to just consider this one
            }
        }
        state_ids.into_iter().collect()
    }

    fn solve_set_cover_instance_internal(
        &self,
        set_cover_instance: &set_cover_instance::SetCoverInstance,
        min_objective: Option<f64>,
        forced_state_ids: &[u64],
    ) -> Option<(predicates::InvariantWithScoreAndObjective, f64)> {
        let required_ids = set_cover_instance.cex_traces.iter()
            .filter(|(_, tr)| tr.file_source == Some(general_data_types::WaveFormSource::OriginalCex))
            .map(|(&tid, _)| tid)
            .collect::<HashSet<usize>>();
        let heuristic_solution: Option<set_cover_heuristic::HeuristicResult> = set_cover_heuristic::run_heuristic_trace_level(&set_cover_instance, &required_ids);
        let total_cex_weight = set_cover_instance.get_total_cex_weight();
        let total_bex_weight = set_cover_instance.get_total_bex_weight();
        println!(
            "Left with {} predicates and {} states after filtering",
            set_cover_instance.predicates.len(),
            set_cover_instance.all_states.len()
        );

        constants_module::BENCHMARK_ILP_STATES.store(set_cover_instance.all_states.len(), Ordering::Relaxed);

        let mut model = grb::Model::new("model1").unwrap();
        model
            .set_param(grb::parameter::IntParam::OutputFlag, 1)
            .unwrap();
        model.set_param(grb::param::TimeLimit, 4000.0).unwrap();
        model.set_param(grb::param::Threads, 100).unwrap();
        model.set_param(grb::param::ConcurrentMIP, 10).unwrap();
        model.set_param(grb::param::Method, 3).unwrap();

        let mut state_vars: HashMap<u64, grb::Var, data_types::general_data_types::DefaultScalarHasher> = HashMap::default();
        let mut predicate_vars: HashMap<usize, grb::Var, data_types::general_data_types::DefaultScalarHasher> = HashMap::default();
        let mut cex_trace_vars: HashMap<u64, grb::Var, data_types::general_data_types::DefaultScalarHasher> = HashMap::default();
        let mut bex_trace_vars: HashMap<u64, grb::Var, data_types::general_data_types::DefaultScalarHasher> = HashMap::default();

        for state_id in set_cover_instance.all_states.iter() {
            let state_var =
                grb::add_binvar!(model, name: format_truncated!("state_{}", state_id).as_str())
                    .unwrap();
            state_vars.insert(*state_id, state_var);
        }

        // if !forced_state_ids.is_empty() {
        //     let mut sum_forced_states = grb::expr::LinExpr::new();
        //     let mut num_present_forced_states = 0;
        //     for forced_state_id in forced_state_ids.iter() {
        //         if let Some(state_var) = state_vars.get(forced_state_id) {
        //             sum_forced_states.add_term(1.0, state_var.clone());
        //             num_present_forced_states += 1;
        //         } else {
        //             println!(
        //                 "Skipping forced state {}: not present in model",
        //                 forced_state_id
        //             );
        //         }
        //     }
        //     if num_present_forced_states == 0 {
        //         println!("No forced states are present in the model");
        //         return None;
        //     }
        //     model
        //         .add_constr(
        //             "at_least_one_forced_state_covered",
        //             grb::c!(sum_forced_states >= 0.9),
        //         )
        //         .unwrap();
        // }

        for (predicate_id, predicate) in set_cover_instance.predicates.iter() {
            let predicate_var = grb::add_binvar!(
                model,
                name: format_truncated!("predicate_{}", predicate_id).as_str()
            )
            .unwrap();
            predicate_vars.insert(*predicate_id, predicate_var);
            if let Some(ref heuristic_solution) = heuristic_solution {
                if heuristic_solution.selected_predicates.contains(predicate_id) {
                    model.set_obj_attr(grb::attr::Start, &predicate_var, 1.0).unwrap();
                    println!("Setting warm start for predicate {}", predicate.predicate);
                }
            }
        }
        for (_idx, cex_trace) in set_cover_instance.cex_traces.iter() {
            let cex_var = grb::add_binvar!(
                model,
                name: format_truncated!("cex_trace_{}", cex_trace.from_path).as_str()
            )
            .unwrap();
            cex_trace_vars.insert(cex_trace.id as u64, cex_var);
        }
            for (_idx, bex_trace) in set_cover_instance.bex_traces.iter() {
                let bex_var = grb::add_binvar!(
                    model,
                    name: format_truncated!("bex_trace_{}", bex_trace.from_path).as_str()
                )
                .unwrap();
                bex_trace_vars.insert(bex_trace.id as u64, bex_var);
            }
        

        let bex_weight = costs::get_bex_weight_in_ilp(set_cover_instance.get_total_cex_weight() as usize, total_bex_weight as f64, self.formula_score_weights.bex_multiplier);
        let cex_weight_per_trace = costs::get_cex_weight_in_ilp(set_cover_instance.get_total_cex_weight() as usize, total_bex_weight as f64);
        println!("BEX weight in ILP: {} total bex weight {} total cex weight {} total bex weight {} multiplier {}", bex_weight, total_bex_weight, total_cex_weight, total_bex_weight, self.formula_score_weights.bex_multiplier);
        let total_num_bex_teacher = self.teacher.get_total_bex_weight();
        let num_cex_traces = self.teacher.cex_traces.len();
        let bex_weight_teacher = costs::get_bex_weight_in_ilp(num_cex_traces, total_num_bex_teacher, self.formula_score_weights.bex_multiplier);
        println!("BEX weight in teacher {} total num bex {} num cex traces {}", bex_weight_teacher, total_num_bex_teacher, num_cex_traces);
        println!("CEX weight per trace in ILP: {}", cex_weight_per_trace);

        let mut objective = grb::expr::LinExpr::new();
        let mut sum_traces = grb::expr::LinExpr::new();
        let mut max_possible_objective = 0.0;
        for (id, cex_trace_var) in cex_trace_vars.iter() {
            let cex_trace = set_cover_instance.cex_traces.get(&(*id as usize)).unwrap();
            let cex_trace_weight = cex_trace.weight;
            objective.add_term(
                cex_weight_per_trace * cex_trace_weight as f64,
                cex_trace_var.clone(),
            );
            sum_traces.add_term(cex_weight_per_trace * cex_trace_weight as f64, cex_trace_var.clone());
            max_possible_objective += cex_weight_per_trace * cex_trace_weight as f64;
        }
        let mut sum_bex_traces =  grb::expr::LinExpr::new();
        if !(constants_module::SOLVE_STATES_INSTEAD_OF_TRACES) {
            for (id, bex_trace_var) in bex_trace_vars.iter() {
                let bex_trace = set_cover_instance.bex_traces.get(&(*id as usize)).unwrap();
                let bex_trace_weight = bex_trace.weight;
                objective.add_term(
                    -bex_weight * bex_trace_weight as f64,
                    bex_trace_var.clone(),
                );
                sum_bex_traces.add_term(-bex_weight * bex_trace_weight as f64, bex_trace_var.clone());
                max_possible_objective += -bex_weight * bex_trace_weight as f64;
            }
        } else {
            let mut sum_bex_samples = grb::expr::LinExpr::new();
            for (_bex_id,bex_sample) in set_cover_instance.bex_samples.iter() {
                let state_var = state_vars.get(&bex_sample.state_id).unwrap();
                objective.add_term(
                    -bex_weight * bex_sample.weight as f64,
                    state_var.clone(),
                );
                sum_bex_samples.add_term(-bex_weight * bex_sample.weight as f64, state_var.clone());
            }
        }
        println!("ILP Max possible objective from CEX: {}", max_possible_objective);

        let mut sum_predicates = grb::expr::LinExpr::new();
        let mut sum_predicate_cost_term = grb::expr::LinExpr::new();
        for (predicate_id, predicate_var) in predicate_vars.iter() {
            objective.add_term(
                -costs::get_invariant_cost(
                    total_cex_weight as usize,
                    total_bex_weight,
                    &set_cover_instance.predicates.get(predicate_id).unwrap().predicate.to_invariant(),
                    self.formula_score_weights.predicate_base_cost
                ),
                predicate_var.clone(),
            );
            sum_predicate_cost_term.add_term(
                -costs::get_invariant_cost(
                    total_cex_weight as usize,
                    total_bex_weight,
                    &set_cover_instance.predicates.get(predicate_id).unwrap().predicate.to_invariant(),
                    self.formula_score_weights.predicate_base_cost
                ),
                predicate_var.clone(),
            );
            sum_predicates.add_term(1.0, predicate_var.clone());
        }
        model.add_constr("at least one predicate", grb::c!(sum_predicates >= 1.0)).unwrap();
        // model.add_constr("min_score", grb::c!(objective.clone() >= min_objective.unwrap_or(f64::MIN))).unwrap();
        model
            .set_objective(objective, grb::ModelSense::Maximize)
            .unwrap();
        model.update().unwrap();

        println!("Adding state contraints");
        for (state_id, state_var) in state_vars.iter() {
            let mut sum_predicates = grb::expr::LinExpr::new();
            let mut sum_non_covering = grb::expr::LinExpr::new();
            for (predicate_id, predicate) in set_cover_instance.predicates.iter() {
                if predicate.covered_states.contains(&(*state_id as u32)) {
                    sum_predicates.add_term(1.0, predicate_vars[predicate_id].clone());
                } else {
                    let predicate_var = predicate_vars.get(predicate_id).unwrap();
                    sum_non_covering.add_term(1.0, predicate_var.clone());
                    //The below constraint says: If this state is selected,
                    //Then no predicate that does not cover it can be selected. 
                    //Should be right?
                    // model
                    //     .add_constr(
                    //         format_truncated!("state_not_cov_cut_{}_{}", state_id, predicate_id).as_str(),
                    //         grb::c!(state_var.clone() + predicate_var.clone() <= 1.0),
                    //     )
                    //     .unwrap();
                }   
            }
            if sum_non_covering.iter_terms().len() == predicate_vars.len() {
                println!("No predicates cover state {}", state_id);
                model
                    .add_constr(
                        format_truncated!("state_not_covered_{}", state_id).as_str(),
                        grb::c!(state_var <= 0.1),
                    )
                    .unwrap();
                continue;
            }
            model
                .add_genconstr_indicator(
                    format_truncated!("state_constraint_sel2_{}", state_id).as_str(),
                    state_var.clone(),
                    true,
                    grb::c!(sum_non_covering.clone() <= 0.1),
                )
                .unwrap();
            model
                .add_genconstr_indicator(
                    format_truncated!("state_constraint_unsel_{}", state_id).as_str(),
                    state_var.clone(),
                    false,
                    grb::c!(sum_non_covering >= 0.9),
                )
                .unwrap();
        }

        for (_bex_id, bex_sample) in set_cover_instance.bex_samples.iter() {
            if bex_sample.must_not_cover == true {
                let state_var = state_vars.get(&bex_sample.state_id).unwrap();
                model
                    .add_constr(
                        format_truncated!(
                            "bex_constraint_{}",
                            bex_sample.state_id
                        )
                        .as_str(),
                        grb::c!(state_var == 0),
                    )
                    .unwrap();
            }
        }

        for (idx, cex_trace_var) in cex_trace_vars.iter() {
            let mut state_vars_for_this_trace = Vec::new();
            let mut sum_states_for_this_trace = grb::expr::LinExpr::new();
            let mut num_states_for_this_trace = 0usize;
            let cex_trace_obj = set_cover_instance.cex_traces.get(&(*idx as usize)).unwrap();
            for (state_id,_cex_samples) in cex_trace_obj.states.iter() {
                let state_var = state_vars.get(&(*state_id as u64)).unwrap();
                state_vars_for_this_trace.push(state_var.clone());
                sum_states_for_this_trace.add_term(1.0, state_var.clone());
                num_states_for_this_trace += 1;
                //This constraint says: If a state in the trace is selected, then the trace must be selected.
                model
                    .add_constr(
                        format_truncated!("cex_or_lb_{}_{}", idx, state_id).as_str(),
                        grb::c!(cex_trace_var.clone() >= state_var.clone()),
                    )
                    .unwrap();
            }
            if num_states_for_this_trace == 0 {
                if cex_trace_obj.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                    log::warn!(
                        "Original CEX trace {} has no states after preprocessing; model is infeasible",
                        cex_trace_obj.from_path
                    );
                    println!("Original CEX trace {} has no states after preprocessing; model is infeasible", cex_trace_obj.from_path);
                    return None;
                }
                model
                    .add_constr(
                        format_truncated!("cex_empty_trace_{}", idx).as_str(),
                        grb::c!(cex_trace_var.clone() <= 0.1),
                    )
                    .unwrap();
                continue;
            }
            model
                .add_constr(
                    format_truncated!("cex_or_ub_{}", idx).as_str(),
                    grb::c!(cex_trace_var.clone() <= sum_states_for_this_trace),
                )
                .unwrap();
            model
                .add_genconstr_or(
                    format_truncated!("cex_constraint_{}", idx).as_str(),
                    cex_trace_var.clone(),
                    state_vars_for_this_trace,
                )
                .unwrap();
            if cex_trace_obj.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex) {
                model
                    .add_constr(
                        format_truncated!("origin_cex_constraint").as_str(),
                        grb::c!(cex_trace_var >= 0.9),
                    )
                    .unwrap();
                println!("Adding original CEX constraint for trace {}", cex_trace_obj.from_path);
            }
        }
        let mut num_must_not_cover_constraints = 0;
        for (idx, bex_trace_var) in bex_trace_vars.iter() {
            let mut state_vars_for_this_trace = Vec::new();
            let mut sum_states_for_this_trace = grb::expr::LinExpr::new();
            let mut num_states_for_this_trace = 0usize;
            let bex_trace_obj = set_cover_instance.bex_traces.get(&(*idx as usize)).unwrap();
            for (state_id,_bex_samples) in bex_trace_obj.states.iter() {
                let state_var = state_vars.get(&(*state_id as u64)).unwrap();
                state_vars_for_this_trace.push(state_var.clone());
                sum_states_for_this_trace.add_term(1.0, state_var.clone());
                num_states_for_this_trace += 1;
                model
                    .add_constr(
                        format_truncated!("bex_or_lb_{}_{}", idx, state_id).as_str(),
                        grb::c!(bex_trace_var.clone() >= state_var.clone()),
                    )
                    .unwrap();
            }
            if num_states_for_this_trace == 0 {
                model
                    .add_constr(
                        format_truncated!("bex_empty_trace_{}", idx).as_str(),
                        grb::c!(bex_trace_var.clone() <= 0.1),
                    )
                    .unwrap();
                continue;
            }
            model
                .add_constr(
                    format_truncated!("bex_or_ub_{}", idx).as_str(),
                    grb::c!(bex_trace_var.clone() <= sum_states_for_this_trace),
                )
                .unwrap();
            model
                .add_genconstr_or(
                    format_truncated!("bex_trace_constraint_{}", idx).as_str(),
                    bex_trace_var.clone(),
                    state_vars_for_this_trace,
                )
                .unwrap();
            if bex_trace_obj.must_not_cover == true {
                // model
                //     .add_constr(
                //         format_truncated!("must_not_cover_{}", idx).as_str(),
                //         grb::c!(bex_trace_var <= 0.1),
                //     )
                //     .unwrap();
                num_must_not_cover_constraints += 1;
            }
        }
        println!("Added {} must-not-cover BEX trace constraints", num_must_not_cover_constraints);
        

        println!("Adding architecture constraints");
        self.add_architecture_constraints(&set_cover_instance.predicates.values().collect::<Vec<_>>(), &predicate_vars, &mut model);
        model.update().unwrap();
        println!("Done with architecture constraints");

        model.update().unwrap();
        let current_time = Local::now();
        println!("Now solving cex traces len {:?}, bex_samples_len {:?}, states_len {:?} predicates len {:?}",total_cex_weight, set_cover_instance.bex_samples.len(), set_cover_instance.all_states.len(), set_cover_instance.predicates.len());
        println!(
            "Started solving at time: {}",
            current_time.format("%d/%m/%y %H:%M")
        );
        model.optimize().unwrap();
        let current_time = Local::now();
        println!(
            "Stopped solving at time: {}",
            current_time.format("%d/%m/%y %H:%M")
        );

        let status = model.status().unwrap();
        if status == grb::Status::Optimal
            || status == grb::Status::TimeLimit
            || status == grb::Status::IterationLimit
            || status == grb::Status::SubOptimal
        {
            let sol_count = model.get_attr(grb::attr::SolCount).unwrap();
            if sol_count == 0 {
                println!("No feasible solution found (status {:?})", status);
                let _ = model.write("set_cover_model.lp");
                if status == grb::Status::Infeasible || status == grb::Status::InfOrUnbd {
                    println!("Attempting IIS computation...");
                    if model.compute_iis().is_ok() {
                        let _ = model.write("set_cover_iis.ilp");
                        if let Ok(constrs) = model.get_constrs() {
                            if let Ok(iis_flags) = model.get_obj_attr_batch(grb::attr::IISConstr, constrs.iter().copied()) {
                                println!("IIS linear constraints (up to 60):");
                                let mut count = 0usize;
                                for (is_iis, constr) in iis_flags.into_iter().zip(constrs.into_iter()) {
                                    if is_iis > 0 {
                                        if let Ok(name) = model.get_obj_attr(grb::attr::ConstrName, &constr) {
                                            println!("  {}", name);
                                        }
                                        count += 1;
                                        if count >= 60 {
                                            break;
                                        }
                                    }
                                }
                                if count == 0 {
                                    println!("No linear IIS constraints found; conflict may involve general constraints.");
                                }
                            }
                        }
                    } else {
                        println!("IIS computation failed");
                    }
                }
                return None;
            }

            println!("Solution found, status is {:?}", status);
            let cex_cost = sum_traces.get_value(&model);
            let bex_cost = sum_bex_traces.get_value(&model);
            let predicate_cost: Result<f64, grb::Error> = sum_predicate_cost_term.get_value(&model);
            println!("CEX cost in solution: {:?}", cex_cost);
            println!("BEX cost in solution: {:?}", bex_cost);
            println!("Predicate cost in solution: {:?}", predicate_cost);

            let mut final_invariant = predicates::InvariantWithScoreAndObjective::default();
            final_invariant.invariant = predicates::Invariant::new();
            for (predicate_id, predicates) in set_cover_instance.predicates.iter() {
                let predicate_var = predicate_vars.get(predicate_id).unwrap().clone();
                let value = model.get_obj_attr(grb::attr::X, &predicate_var).unwrap();
                if value > 0.5 {
                    println!("Predicate {} is selected", predicates.predicate);
                    final_invariant
                        .invariant
                        .add_predicate(predicates.predicate.base_predicate.clone());
                }
            }

            if final_invariant.invariant.predicate_set.predicates.is_empty() {
                println!("No predicates selected? Candidate predicaters were");

                let state_bitset = data_types::general_data_types::BitSetWrapper::from_vec(
                    self.teacher
                        .states
                        .keys()
                        .map(|x| *x as u32)
                        .collect::<Vec<u32>>(),
                );
                let orig_cex_statest = self.teacher.cex_traces.iter().find(|x| x.file_source == Some(data_types::general_data_types::WaveFormSource::OriginalCex)).map(|x|x.contained_samples.iter().map(|y| y.state_id as u32).collect::<Vec<u32>>()).unwrap_or_default();
                println!("Orgin cex states: {:?}", orig_cex_statest);
                for (_id,predicate) in set_cover_instance.predicates.iter() {
                    let covered_states = predicate.covered_states.intersection(&state_bitset).collect();
                    println!("Predicate: {} covering states {:?}", predicate.predicate, covered_states);
                }
            }

            let this_solver = smt_solver::SMTSolver::new_from_teacher(&self.teacher);
            let final_invariant_score = this_solver.score_invariant_with_fulfilled_examples(&final_invariant.invariant);
            let final_invariant_objective = this_solver.calculate_invariant_objective_from_covered_states(&final_invariant.invariant, &final_invariant_score.cover_info.covered_states, false,&self.formula_score_weights, false);
            final_invariant.score = final_invariant_score;
            final_invariant.objective = predicates::InvariantObjective { objective: final_invariant_objective };
            println!("In Solver Final invariant {}", final_invariant.invariant);
            println!("In Solver Final score {:?}", final_invariant.score.score);

            println!(
                "In Solver Final objective {:?}",
                this_solver
                    .calculate_invariant_objective(&final_invariant.invariant, false, &self.formula_score_weights)
                    .objective
            );

            let obj_val = model.get_attr(grb::attr::ObjVal).unwrap();
            println!("Objective from ILP: {}", obj_val);
            println!("bex weight in ILP: {} {} {}", bex_weight, total_cex_weight, total_bex_weight);
            Some((final_invariant, obj_val))
        } else {
            println!("No optimal solution found, status {:?}", status);
            let _ = model.write("set_cover_model.lp");
            if status == grb::Status::Infeasible || status == grb::Status::InfOrUnbd {
                println!("Attempting IIS computation...");
                if model.compute_iis().is_ok() {
                    let _ = model.write("set_cover_iis.ilp");
                    if let Ok(constrs) = model.get_constrs() {
                        if let Ok(iis_flags) = model.get_obj_attr_batch(grb::attr::IISConstr, constrs.iter().copied()) {
                            println!("IIS linear constraints (up to 60):");
                            let mut count = 0usize;
                            for (is_iis, constr) in iis_flags.into_iter().zip(constrs.into_iter()) {
                                if is_iis > 0 {
                                    if let Ok(name) = model.get_obj_attr(grb::attr::ConstrName, &constr) {
                                        println!("  {}", name);
                                    }
                                    count += 1;
                                    if count >= 60 {
                                        break;
                                    }
                                }
                            }
                            if count == 0 {
                                println!("No linear IIS constraints found; conflict may involve general constraints.");
                            }
                        }
                    }
                } else {
                    println!("IIS computation failed");
                }
            }
            None
        }
    }

    pub fn solve_set_cover_instance(
        &self,
        set_cover_instance: &set_cover_instance::SetCoverInstance,
        min_objective: Option<f64>,
    ) -> Option<predicates::InvariantWithScoreAndObjective> {
        let forced_state_ids = Self::collect_cex_state_ids(set_cover_instance);
        println!("Solving set cover instance with {} predicates, {} cex traces, {} bex samples and {} states. Forced state ids from cex traces: {:?}",
            set_cover_instance.predicates.len(),
            set_cover_instance.cex_traces.len(),
            set_cover_instance.bex_samples.len(),
            set_cover_instance.all_states.len(),
            forced_state_ids.len()
        );

        self.solve_set_cover_instance_internal(
            set_cover_instance,
            min_objective,
            &forced_state_ids,
        )
        .map(|(solution, _)| solution)
    }
}
