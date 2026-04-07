use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use indicatif::ParallelProgressIterator;


use crate::predicates;
use crate::predicates::BasePredicate;
use crate::teacher;
use crate::smt_solver::SMTSolver as thisSolver;
use crate::data_types::general_data_types;
//use crate::ilp_solver::ILPSolver as thisSolver;
use crate::solver::Solver;
use crate::constants as constants_module;
use crate::smt_solver::get_gini_coefficient_from_predicate_list;
use crate::smt_solver;
use std::collections::HashSet;
use std;
use std::sync::Arc;
use log;



#[derive(Debug)]
pub struct DecisionTree {
    pub split_predicate: Option<predicates::BasePredicate>,
    pub true_branch: Option<Box<DecisionTree>>,
    pub false_branch: Option<Box<DecisionTree>>,
}

impl DecisionTree {
    pub fn to_logic_formula(&self) -> String {
        match &self.split_predicate {
            Some(predicate) => {
                let true_formula = self.true_branch.as_ref().map_or("true".to_string(), |branch| branch.to_logic_formula());
                let false_formula = self.false_branch.as_ref().map_or("false".to_string(), |branch| branch.to_logic_formula());

                format!(
                    "({} AND {}) OR (NOT ({} AND {}))",
                    predicate.to_string(),
                    true_formula,
                    predicate.to_string(),
                    false_formula
                )
            }
            None => "true".to_string(),
        }
    }
}


impl DecisionTree {
    pub fn new() -> Self {
        DecisionTree {
            split_predicate: None,
            true_branch: None,
            false_branch: None,
        }
    }

    pub fn add_branches(&mut self, true_branch: DecisionTree, false_branch: DecisionTree) {
        self.true_branch = Some(Box::new(true_branch));
        self.false_branch = Some(Box::new(false_branch));
    }

    pub fn get_used_predicate(&self, used_predicates: &mut HashSet<BasePredicate>) {
        if let Some(ref predicate) = self.split_predicate {
            used_predicates.insert(predicate.clone());
            used_predicates.insert(predicate.get_inverse());
        }
        if let Some(ref true_branch) = self.true_branch {
            true_branch.get_used_predicate(used_predicates);
        }
        if let Some(ref false_branch) = self.false_branch {
            false_branch.get_used_predicate(used_predicates);
        }
    }

    pub fn get_used_signals(
        &self,
        used_signals: &mut HashSet<Arc<str>>,
    ) {
        if let Some(ref predicate) = self.split_predicate {
            used_signals.extend(predicate.get_signal_names());
        }
        if let Some(ref true_branch) = self.true_branch {
            true_branch.get_used_signals(used_signals);
        }
        if let Some(ref false_branch) = self.false_branch {
            false_branch.get_used_signals(used_signals);
        }
    }
} 

pub fn decision_tree_to_logic_formula(tree: &DecisionTree) -> String {
    if let Some(ref predicate) = tree.split_predicate {
        let mut formula = predicate.to_string().to_string();
        if tree.true_branch.is_some() || tree.false_branch.is_some() {
            formula.push_str(" && (");
            if let Some(ref true_branch) = tree.true_branch {
                formula.push_str(&decision_tree_to_logic_formula(true_branch));
            }
            if let Some(ref false_branch) = tree.false_branch {
                formula.push_str(" || ");
                formula.push_str(&decision_tree_to_logic_formula(false_branch));
            }
            formula.push(')');
        }
        formula
    } else {
        String::from("true")
    }
}

pub fn format_tree_to_formula(tree: &DecisionTree) -> String {
    let mut formula = String::new();
    if let Some(ref predicate) = tree.split_predicate {
        formula.push_str(&predicate.to_string());
        formula.push_str(" && (");
    }
    if let Some(ref true_branch) = tree.true_branch {
        formula.push_str(&format_tree_to_formula(true_branch));
    }
    if let Some(ref false_branch) = tree.false_branch {
        formula.push_str(" || ");
        formula.push_str(&format_tree_to_formula(false_branch));
    }
    formula.push_str(")");
    formula
}

impl std::fmt::Display for DecisionTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn format_tree(tree: &DecisionTree, indent: usize, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let indentation = " ".repeat(indent);
            if let Some(ref predicate) = tree.split_predicate {
                write!(f, "{}Predicate: {}\n", indentation, predicate.to_string())?;
            } else {
                write!(f, "{}Predicate: None\n", indentation)?;
            }
            if let Some(ref true_branch) = tree.true_branch {
                write!(f, "{}True Branch:\n", indentation)?;
                format_tree(true_branch, indent + 4, f)?;
            }
            if let Some(ref false_branch) = tree.false_branch {
                write!(f, "{}False Branch:\n", indentation)?;
                format_tree(false_branch, indent + 4, f)?;
            }
            Ok(())
        }
        format_tree(self, 0, f)

        /*fn format_tree(tree: &DecisionTree, indent: usize, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let indentation = " ".repeat(indent);
            //Format like this: predicate && (format(tree.true_branch) || format(tree.false_branch))
            //write!(f, "{}{}", tree.to_string)
            if let Some(ref predicate) = tree.split_predicate {
                write!(f, "{} {}", predicate.to_string(None), "")?;
                //writeln!(f, "{}Predicate: {:?}", indentation, predicate)?;
            } else {
                {}
                //writeln!(f, "{}Predicate: None", indentation)?;
            }
            write!(" && ")?;
            if let Some(ref true_branch) = tree.true_branch {
                //writeln!(f, "{}True Branch:", indentation)?;
                //format_tree(true_branch, indent + 4, f)?;
                format_tree(&true_branch)
            }
            if let Some(ref predicate) = tree.split_predicate {
                writeln!(f, "{}Predicate: {:?}", indentation, predicate)?;
            } else {
                writeln!(f, "{}Predicate: None", indentation)?;
            }
            if let Some(ref true_branch) = tree.true_branch {
                writeln!(f, "{}True Branch:", indentation)?;
                format_tree(true_branch, indent + 4, f)?;
            }
            if let Some(ref false_branch) = tree.false_branch {
                writeln!(f, "{}False Branch:", indentation)?;
                format_tree(false_branch, indent + 4, f)?;
            }
            Ok(())
        }

        format_tree(self, 0, f)*/
    }
}

pub fn decision_tree_to_invariant_collection(
    tree: &DecisionTree,
    invariant_collection: &mut Vec<predicates::Invariant>,
    current_invariant: Option<&predicates::Invariant>,
)
{
    if tree.split_predicate.is_none() {
        invariant_collection.push(current_invariant.unwrap().clone());
    } else {
        let mut invariant = predicates::Invariant::new();
        invariant.add_predicate(tree.split_predicate.as_ref().unwrap().clone());    
        let lhs_invariant = if let Some(branch_invariant) = current_invariant {
            invariant.merge_invariant(branch_invariant)
        } else {
            invariant.clone()
        };
        let rhs_invariant = if let Some(branch_invariant) = current_invariant {
            let mut new_invariant = branch_invariant.clone();
            new_invariant.add_predicate(tree.split_predicate.as_ref().unwrap().clone().get_inverse());
            new_invariant
        } else {
            let mut new_invariant = predicates::Invariant::new();
            new_invariant.add_predicate(tree.split_predicate.as_ref().unwrap().clone().get_inverse());
            new_invariant
        };
        if let Some(ref true_branch) = tree.true_branch {
            decision_tree_to_invariant_collection(true_branch, invariant_collection, Some(&lhs_invariant));
        }
        if let Some(ref false_branch) = tree.false_branch {
            decision_tree_to_invariant_collection(false_branch, invariant_collection, Some(&rhs_invariant));
        }
    }

}

pub fn grow_decision_tree( 
    tree: &mut DecisionTree,
    current_teacher: &teacher::Teacher,
    scored_predicates: &Vec<predicates::InvariantWithScoreAndObjective>,
    branch_invariant: Option<&predicates::InvariantWithScoreAndObjective>,
    formula_score_weights: &general_data_types::FormulaScoreWeights
) {
    if current_teacher.cex_traces.len() == 0 {
        return;
    }
    if current_teacher.bex_samples.len() == 0 {
        //println!("No more samples to split on.");
        return;
    }
    log::info!("Growing decision tree with cex {} samples and {} bex samples len so far {}", current_teacher.cex_traces.len(), current_teacher.bex_samples.len(), 
                branch_invariant.as_ref().map_or(0, |inv| inv.invariant.predicate_set.predicates.len()));
    let next_predicate = get_next_predicate_gini_coeff(&scored_predicates, current_teacher, branch_invariant, formula_score_weights);
    if next_predicate.is_none() {
        log::debug!("No more predicates to grow the tree.");
        return;
    }
    let next_predicate = next_predicate.unwrap();
    
    tree.split_predicate = Some(next_predicate.invariant.predicate_set.predicates[0].clone());
    log::debug!("Splitting on predicate {}", next_predicate.invariant);
    let this_solver = thisSolver::new_from_teacher(&current_teacher);
    let mut invariant = predicates::Invariant::new();
    invariant.add_predicate(next_predicate.invariant.predicate_set.predicates[0].clone());
    let (_, lhs_teacher, rhs_teacher) = this_solver.calculate_examples_split(
        &invariant,
    );
    log::debug!("LHS Teacher has {} cex traces and {} bex samples", lhs_teacher.cex_traces.len(), lhs_teacher.bex_samples.len());
    log::debug!("RHS Teacher has {} cex traces and {} bex samples", rhs_teacher.cex_traces.len(), rhs_teacher.bex_samples.len());
    let lhs_invariant = if let Some(branch_invariant) = branch_invariant {
        this_solver.merge_invariant_and_score_from_with_objective(
            &branch_invariant,
            &next_predicate,
            formula_score_weights
        )
    } else {
        next_predicate.clone()
    };
    let rhs_invariant = if let Some(branch_invariant) = branch_invariant {
        let mut new_invariant = branch_invariant.invariant.clone();
        new_invariant.add_predicate(next_predicate.invariant.clone().predicate_set.predicates[0].clone().get_inverse());
        let score = this_solver.score_invariant_with_fulfilled_examples(&new_invariant);
        // let objective = predicates::InvariantObjective {
        //     objective: this_solver.calculate_invariant_objective_from_covered_states(&new_invariant, &score.cover_info.covered_states, true,
        //     formula_score_weights, false)
        // };
        predicates::InvariantWithScoreAndObjective {
            invariant: new_invariant,
            score: score,
            objective: predicates::InvariantObjective{ objective: 0.0}
        }
    } else {
        let mut new_invariant = predicates::Invariant::new();
        new_invariant.add_predicate(next_predicate.clone().invariant.predicate_set.predicates[0].clone().get_inverse());
        let score = this_solver.score_invariant_with_fulfilled_examples(&new_invariant);
        // let objective = predicates::InvariantObjective {
        //     objective: this_solver.calculate_invariant_objective_from_covered_states(&new_invariant, &score.cover_info.covered_states, true,
        //     formula_score_weights, false)
        // };
        predicates::InvariantWithScoreAndObjective {
            invariant: new_invariant,
            score: score,
            objective:  predicates::InvariantObjective{ objective: 0.0} //Not used
        }
    };
    log::debug!("LHS Invariant: {}", lhs_invariant.invariant);
    log::debug!("RHS Invariant: {}", rhs_invariant.invariant);

    
    let mut true_branch = DecisionTree::new();
    let mut false_branch = DecisionTree::new();
    let new_predicates_lhs = scored_predicates.iter().filter(|predicate| {
        lhs_invariant.invariant.should_add_other_invariant(&predicate.invariant) && lhs_invariant.invariant.should_add_predicate(&predicate.invariant.predicate_set.predicates[0].clone().get_inverse())
    }).cloned().collect::<Vec<predicates::InvariantWithScoreAndObjective>>();
    let new_predicates_rhs = scored_predicates.iter().filter(|predicate| {
        rhs_invariant.invariant.should_add_other_invariant(&predicate.invariant) && rhs_invariant.invariant.should_add_predicate(&predicate.invariant.predicate_set.predicates[0].clone().get_inverse())
    }).cloned().collect::<Vec<predicates::InvariantWithScoreAndObjective>>();
    if lhs_teacher.cex_traces.len() == current_teacher.cex_traces.len() && lhs_teacher.bex_samples.len() == current_teacher.bex_samples.len() {
        log::debug!("Decision tree algorithm converged, giving up");
        return;
    }
    if rhs_teacher.cex_traces.len() == current_teacher.cex_traces.len() && rhs_teacher.bex_samples.len() == current_teacher.bex_samples.len() {
        log::debug!("Decision tree algorithm converged, giving up");
        return;
    }
    if lhs_teacher.cex_traces.len() > 0 && lhs_teacher.bex_samples.len() > 0 {
        log::debug!("Left Growing on lhs num cex samples: {}, num bex samples: {}", lhs_teacher.cex_traces.len(), lhs_teacher.bex_samples.len());
        //lhs_teacher.calculate_values_per_signal();
        grow_decision_tree(&mut true_branch, &lhs_teacher, &new_predicates_lhs, Some(&lhs_invariant.clone()),
        formula_score_weights
    );
    } 
    if rhs_teacher.cex_traces.len() > 0 && rhs_teacher.bex_samples.len() > 0 {
        grow_decision_tree(&mut false_branch, &rhs_teacher, &new_predicates_rhs, Some(&rhs_invariant.clone()),
        formula_score_weights
    );
        log::debug!("Growing on rhs num cex samples: {}, num bex samples: {}", rhs_teacher.cex_traces.len(), rhs_teacher.bex_samples.len());
        //rhs_teacher.calculate_values_per_signal();
    }
    log::info!("Done, with branch invariant length {}", branch_invariant.as_ref().map_or(0, |inv| inv.invariant.predicate_set.predicates.len()));


  
    
    tree.add_branches(true_branch, false_branch);



    /* 
        let true_branch = DecisionTree::new(Some(predicate.clone()));
        let false_branch = DecisionTree::new(None);
        tree.add_branches(true_branch, false_branch);
    }

    let predicate = &predicates[current_depth];
    let true_branch = DecisionTree::new(Some(predicate.clone()));
    let false_branch = DecisionTree::new(None);

    tree.add_branches(true_branch, false_branch);

    // Recursively grow the branches
    self.grow_decision_tree(tree.true_branch.as_mut().unwrap(), predicates, current_depth + 1, max_depth);
    self.grow_decision_tree(tree.false_branch.as_mut().unwrap(), predicates, current_depth + 1, max_depth);
    */
}


fn get_next_predicate_gini_coeff(
    predicate_list: &Vec<predicates::InvariantWithScoreAndObjective>,
    teacher: &teacher::Teacher,
    this_branch_invariant: Option<&predicates::InvariantWithScoreAndObjective>,
    formula_score_weights: &general_data_types::FormulaScoreWeights
) -> Option<predicates::InvariantWithScoreAndObjective> {
    if this_branch_invariant.is_some() {
        log::debug!("get_next_predicate_gini_coeff Current branch invariant: {}", this_branch_invariant.as_ref().unwrap().invariant);
    } else {
        log::debug!("get_next_predicate_gini_coeff No current branch invariant, starting from scratch.");
    }
    //println!("Getting next predicate with gini coefficient for this_branch_invariant {:?}", this_branch_invariant.as_ref().map(|x| x.invariant.to_string()));
    let ret_vec: Vec<(predicates::InvariantWithScoreAndObjective, f64)> = get_gini_coefficient_from_predicate_list(predicate_list, teacher, this_branch_invariant,
        formula_score_weights
    );
    let mut min_predicate_score = None;
    let mut min_predicate = None;
    for (base_formula, score) in ret_vec.iter() {
        //println!("Got predicate scores for {:?} {:?}", base_formula, score);
        if score < min_predicate_score.unwrap_or(&constants_module::MAX_GINI_IMPURITY) {
            min_predicate_score = Some(score);
            let this_min_predicate = base_formula.clone();
            min_predicate = Some(this_min_predicate);
        }
    }
    if min_predicate_score.is_none() || min_predicate_score.unwrap() >= &constants_module::MAX_GINI_IMPURITY {
        log::debug!("No more predicates to add to the invariant, done.");
        return None;
    } else {
        log::debug!("Got next predicate {} with score {}", min_predicate.as_ref().unwrap().invariant, min_predicate_score.unwrap());
    }
    min_predicate
}

fn get_next_predicate_greedy(
    scored_predicates: &Box<Vec<predicates::InvariantWithScoreAndObjective>>,
    teacher: &teacher::Teacher,
    base_invariant: Option<&predicates::InvariantWithScoreAndObjective>,
    formula_score_weights: &general_data_types::FormulaScoreWeights 
) -> Option<predicates::InvariantWithScoreAndObjective> {
    let solver = thisSolver::new_from_teacher(&teacher);
    let mut current_best_objective: Option<predicates::InvariantObjective> = match base_invariant {
        Some(base_invariant) => Some(base_invariant.objective.clone()),
        None => None
    };
    // let current_best_score = match base_invariant {
    //     Some(base_invariant) => Some(base_invariant.score.score.clone()),
    //     None => None
    // };
    let ret = scored_predicates.par_iter()
        .progress()
        .filter_map(|predicate| {
        //predicate.score.cex_and_bex_score);
        let new_scored_invariant = if base_invariant.is_none(){
          let ret_score = teacher.get_score_from_covered_states(&predicate.score.cover_info.covered_states);
          let tmp = predicates::ScoreAndFulfilledExample {
            score: ret_score,
            cover_info: predicate.score.cover_info.clone()
          };
          let objective = predicates::InvariantObjective {
            objective: solver.calculate_invariant_objective_from_covered_states(&predicate.invariant, &tmp.cover_info.covered_states, false,
            formula_score_weights, false)
          };
          let tmp = predicates::InvariantWithScoreAndObjective {
            invariant: predicate.invariant.clone(),
            score: tmp,
            objective: objective
          };
          tmp
        } else {
           if base_invariant.as_ref().unwrap().invariant.should_add_other_invariant(&predicate.invariant) == false {
                return None;
           }
           solver.merge_invariant_and_score_from_with_objective(&base_invariant.as_ref().unwrap(), predicate, formula_score_weights)
        };
        //let this_objective = solver.calculate_invariant_objective_from_scored_invariant(&new_scored_invariant);
        return Some((predicate.clone(), new_scored_invariant.objective));
        // std::io::Write::flush(&mut std::io::stdout()).unwrap();
        // std::io::Write::flush(&mut std::io::stdout()).unwrap();
        // if (new_scored_invariant.score.score.cex_and_bex_score != predicates::ScoreResult::Unsat) && (current_best_score.is_none() || 
        //     new_scored_invariant.score.score > *(current_best_score.as_ref()).unwrap()) {
        //     current_best_score = Some(new_scored_invariant.score.score);
        //     max_predicate = Some(predicate.clone());
        // }
        // if current_best_objective.is_none() || this_objective.objective > current_best_objective.as_ref().unwrap().objective {
        //     current_best_objective = Some(this_objective);
        //     // current_best_score = Some(new_scored_invariant.score.score.clone());
        //     max_predicate = Some(predicate.clone());
        // }
    }).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    match ret {
        Some((predicate, objective)) => {
            if current_best_objective.is_none() || objective.objective > current_best_objective.as_ref().unwrap().objective {
                current_best_objective = Some(objective);
                log::debug!("Greedy -- Updating current best objective to {:?}", current_best_objective);
                return Some(predicate);
            } else {
                return None;
            }
        },
        None => {
            return None;
        }
    }
    // if max_predicate.is_none() {
    //     println!("No more predicates to add to the invariant, done.");
    //     return None;
    // } else {
    //     println!("Got next predicate {} with score {:?}", max_predicate.as_ref().unwrap().invariant, current_best_objective);
    //     return max_predicate;
    // }
   // return max_predicate;
}


pub fn next_best_algorithm(
    predicate_list: &Box<Vec<predicates::InvariantWithScoreAndObjective>>,
    teacher: &teacher::Teacher,
    formula_score_weights: &general_data_types::FormulaScoreWeights,
) -> Option<predicates::InvariantWithScoreAndObjective> {
    let mut current_invariant: Option<predicates::InvariantWithScoreAndObjective> = None;
    let mut _done: bool = false;
    let mut current_teacher = teacher.clone();
    let mut current_predicate_list: Box<Vec<predicates::InvariantWithScoreAndObjective>> = predicate_list.clone(); //utils::get_covered_for_predicates(predicate_list, teacher);
    let this_solver = thisSolver::new_from_teacher(&teacher);
    log::info!("Starting greedy algorithm with cex {} samples and {} bex samples", current_teacher.cex_traces.len(), current_teacher.bex_samples.len());
    while !_done {
        log::debug!("Iterating greedy with {} cex traces and {} bex states", current_teacher.cex_traces.len(), current_teacher.bex_samples.len());
        let _maybe_invariant = if current_invariant.is_some() {
            Some(current_invariant.clone().unwrap().invariant)
        } else {
            None
        };
        // current_predicate_list = utils::filter_predicate_list_wrt_invariant_and_teacher(
        //     &current_predicate_list,
        //     &current_teacher,
        //     &maybe_invariant,
        // );
        log::debug!("Calling next predicate greedy");
        let next_predicate = get_next_predicate_greedy(&current_predicate_list, &current_teacher, current_invariant.as_ref(),
    formula_score_weights);
        if next_predicate.is_none() {
            log::info!("No more predicates to add to the invariant, done.");
            _done = true;
            break;
        }
        log::debug!("Got next predicate {}", &next_predicate.as_ref().unwrap().invariant);
        current_invariant = if current_invariant.is_none() {
            Some(next_predicate.clone().unwrap())
        } else {
            let new_invariant = this_solver.merge_invariant_and_score_from_with_objective(
                &current_invariant.as_ref().unwrap(),
                &next_predicate.as_ref().unwrap(),
                formula_score_weights
            );
            Some(new_invariant)
        };
        log::debug!("new invariant {} total bex {}", current_invariant.as_ref().unwrap().invariant, &current_teacher.bex_samples.len());
        (current_teacher, _, _) = smt_solver::calculate_examples_split_from_scored_invariant(current_invariant.as_ref().unwrap(), &current_teacher);
        if current_teacher.bex_samples.len() == 0 {
            log::debug!("No more bex samples to split on.");
            _done = true;
            break;
        }
        current_predicate_list.retain_mut(|predicate| {
            //predicate.invariant.get_relevant_signal_idx() != next_predicate.as_ref().unwrap().invariant.get_relevant_signal_idx()
            current_invariant.as_ref().unwrap().invariant.should_add_other_invariant(&predicate.invariant)
        });
    }
    return current_invariant;


}
