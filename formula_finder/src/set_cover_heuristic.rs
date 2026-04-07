use std::collections::{HashMap, HashSet};

use crate::data_types::general_data_types::{BitSetWrapper};
use crate::set_cover_instance::SetCoverInstance;
use crate::costs;


/// Result of the heuristic: selected predicate IDs and score
pub struct HeuristicResult {
    pub selected_predicates: Vec<usize>,
    pub score: i64,
    pub covered_traces: i64,
    pub b_penalty: i64,
}


/// Recomputes the current coverage sets and the score component (CEX gain - BEX penalty)
/// based on the current Intent (I).
fn recompute_from_currently_covered(
    cex_trace_states: &HashMap<usize, BitSetWrapper>,
    bex_trace_states: &HashMap<usize, BitSetWrapper>,
    cex_weights: &HashMap<usize, i64>,
    bex_weights: &HashMap<usize, i64>,
    bex_multiplier: i64,
    currently_covered: &BitSetWrapper,
    covered_cex: &mut HashSet<usize>,
    covered_bex_traces: &mut HashSet<usize>,
    score_coverage_component: &mut i64,
) {
    // Clear previous state for a fresh calculation
    covered_cex.clear();
    covered_bex_traces.clear();
    
    // Calculate CEX coverage (Positive Score)
    let cex_score_sum: i64 = cex_trace_states.iter()
       .filter_map(|(&tid, tbs)| {
            // A trace is covered if the intersection of its states and the intent I is not empty.
            if!tbs.intersection(currently_covered).collect().is_empty() {
                covered_cex.insert(tid);
                Some(cex_weights[&tid])
            } else {
                None
            }
        })
       .sum();

    // Calculate BEX coverage (Penalty, which is subtracted)
    let bex_penalty_sum: i64 = bex_trace_states.iter()
       .filter_map(|(&tid, tbs)| {
            if!tbs.intersection(currently_covered).collect().is_empty() {
                covered_bex_traces.insert(tid);
                Some(bex_weights[&tid] * bex_multiplier)
            } else {
                None
            }
        })
       .sum();

    *score_coverage_component = cex_score_sum - bex_penalty_sum;
}

/// Calculates the incremental gain (Delta Score) when tightening the current intent I 
/// with a new predicate P, resulting in J = I ∩ P.
/// The gain is (Lost CEX Penalty) + (Avoided BEX Penalty) - (Predicate Cost).
fn eval_gain(
    instance: &SetCoverInstance,
    cex_trace_states: &HashMap<usize, BitSetWrapper>,
    bex_trace_states: &HashMap<usize, BitSetWrapper>,
    cex_weights: &HashMap<usize, i64>,
    bex_weights: &HashMap<usize, i64>,
    bex_multiplier: i64,
    covered_cex: &HashSet<usize>,
    covered_bex: &HashSet<usize>,
    currently_covered: &BitSetWrapper,
    pid: usize,
    required_ids: &HashSet<usize>
) -> Option<(i64, BitSetWrapper)> {
    let predicate = &instance.predicates[&pid];


    // J = I ∩ P: The new, tighter intent
    // Generates the new intersection J by filtering states in I that are also in the predicate P
    // let J: BitSetWrapper = I.collect::<Vec<StateIdType>>().into_iter()
    //    .filter(|s| predicate.covered_states.contains(s))
    //    .collect();
    let mut next_covered: BitSetWrapper = BitSetWrapper::new();
    for s in currently_covered.clone().into_iter() {
        if predicate.covered_states.contains(&s) {
            // State s in I is not covered by predicate P, so it will be excluded from J
            next_covered.add(s);
        }
    }
    
    // If the new intent is empty, it covers nothing, resulting in score 0 (usually a bad move).
    if next_covered.is_empty() {
        return None;
    }

    // --- Constraint Check: Required Trace ---
    // If required_trace_id is currently covered by I, it must remain covered by J.
    for &required_trace_id in required_ids.iter() {
        if covered_cex.contains(&required_trace_id) {
            if cex_trace_states[&required_trace_id].intersection(&next_covered).collect().is_empty() {
                return None; // Cannot lose coverage of the required trace.
            }
        }
    }

    let mut gain: i64 = 0;

    // --- CEX Loss (Negative Gain) ---
    // Iterate only over traces CURRENTLY covered by I (those contributing to the current score).
    // If the tighter intent J no longer covers them, we lose their weight.
    let cex_loss: i64 = covered_cex.iter()
       .filter_map(|&tid| {
            // Exclude required trace from loss calculation as its loss is an outright rejection (handled above)
            if required_ids.contains(&tid) {
                return None;
            }

            // Check if coverage is lost (was covered by I, but not by J)
            if cex_trace_states[&tid].intersection(&next_covered).collect().is_empty() {
                Some(cex_weights[&tid])
            } else {
                None
            }
        })
       .sum();
    
    // CEX loss is a negative contribution to the total gain.
    gain -= cex_loss;


    // --- BEX Gain (Positive Gain / Penalty Avoidance) ---
    // Iterate only over traces CURRENTLY covered by I (those currently causing penalty).
    // If the tighter intent J no longer covers them, we avoid their penalty.
    let bex_avoided_penalty: i64 = covered_bex.iter()
       .filter_map(|&tid| {
            // Check if coverage is lost (was covered by I, but not by J)
            if bex_trace_states[&tid].intersection(&next_covered).collect().is_empty() {
                Some(bex_weights[&tid] * bex_multiplier)
            } else {
                None
            }
        })
       .sum();

    // BEX avoidance is a positive contribution to the total gain.
    gain += bex_avoided_penalty;

    // --- Predicate Cost (Negative Gain) ---
    let predicate_cost = costs::get_invariant_cost(
        instance.get_total_cex_weight() as usize,
        instance.get_total_bex_weight() as f64,
        &predicate.predicate.to_invariant(),
        instance.formula_score_weights.predicate_base_cost,
    ) as i64;

    gain -= predicate_cost;

    Some((gain, next_covered))
}

pub fn run_heuristic_trace_level(
    instance: &SetCoverInstance,
    required_ids: &HashSet<usize>,
) -> Option<HeuristicResult> {
    
    // --- Data Preprocessing: Convert traces to BitSets (Intent elements) ---
    let cex_trace_states: HashMap<usize, BitSetWrapper> = instance
       .cex_traces
       .iter()
       .map(|(&tid, tr)| {
            let mut bs = BitSetWrapper::new();
            bs.insert_all(tr.states.keys().map(|sid| *sid as u32));
            (tid, bs)
        })
       .collect();

    let bex_trace_states: HashMap<usize, BitSetWrapper> = instance
       .bex_traces
       .iter()
       .map(|(&tid, tr)| {
            let mut bs = BitSetWrapper::new();
            bs.insert_all(tr.states.keys().map(|sid| *sid as u32));
            (tid, bs)
        })
       .collect();

    // --- Initialization: Universal Intent (I) ---
    // Start with the most general rule: the set of all relevant states.
    let mut currently_covered: BitSetWrapper = BitSetWrapper::from_vec(instance.all_states.iter().map(|sid| *sid as u32).collect());


    // --- Bookkeeping ---
    let mut selected: HashSet<usize> = HashSet::new();
    let mut covered_cex: HashSet<usize> = HashSet::new();
    let mut covered_bex_traces: HashSet<usize> = HashSet::new();
    
    // Score tracking:
    // score_coverage: CEX_Gain - BEX_Penalty
    // score_total: CEX_Gain - BEX_Penalty - Total_Predicate_Cost
    let mut score_coverage: i64 = 0;
    // let mut score_total: i64 = 0; 

    // --- Weight Lookups ---
    let cex_weights: HashMap<usize, i64> = instance
       .cex_traces
       .iter()
       .map(|(&tid, tr)| (tid, tr.weight as i64))
       .collect();

    let bex_weights: HashMap<usize, i64> = instance
       .bex_traces
       .iter()
       .map(|(&tid, tr)| (tid, tr.weight as i64))
       .collect();

    let bex_multiplier = costs::get_bex_weight_in_ilp(
        instance.get_total_cex_weight() as usize,
        instance.get_total_bex_weight() as f64,
        instance.formula_score_weights.bex_multiplier,
    ) as i64;


    // --- Phase 0: Initial Score Calculation ---
    recompute_from_currently_covered(
        &cex_trace_states,
        &bex_trace_states,
        &cex_weights,
        &bex_weights,
        bex_multiplier,
        &currently_covered,
        &mut covered_cex,
        &mut covered_bex_traces,
        &mut score_coverage,
    );
    // score_total = score_coverage; // Cost is 0 initially


    // ------------------ Phase 1: Greedy General-to-Specific Search ------------------
    loop {
        let mut best_pid: Option<usize> = None;
        let mut best_gain: i64 = i64::MIN;
        let mut best_next_covered: Option<BitSetWrapper> = None;

        for (&pid, _) in &instance.predicates {
            if selected.contains(&pid) { continue; }
            
            // Check viability and calculate gain for tightening I to J = I ∩ P
            if let Some((gain, this_next_covered)) = eval_gain(
                instance,
                &cex_trace_states,
                &bex_trace_states,
                &cex_weights,
                &bex_weights,
                bex_multiplier,
                &covered_cex,
                &covered_bex_traces,
                &currently_covered,
                pid,
                &required_ids,
            ) {
                if gain > best_gain {
                    best_gain = gain;
                    best_pid = Some(pid);
                    best_next_covered = Some(this_next_covered);
                }
            }
        }

        // Stopping condition: Only proceed if adding a predicate is strictly beneficial (gain > 0)
        // or if no predicate was found. The required_trace constraint is handled within eval_gain.
        if best_pid.is_none() || best_gain <= 0 {
            break;
        }

        // Accept the best predicate
        let pid = best_pid.unwrap();
        currently_covered = best_next_covered.expect("J must be computed for best predicate");
        selected.insert(pid);
        
        // Update the total score with the calculated gain (which includes the cost)
        // score_total += best_gain; 

        // Recompute coverage sets and coverage score for the new intent I
        recompute_from_currently_covered(
            &cex_trace_states,
            &bex_trace_states,
            &cex_weights,
            &bex_weights,
            bex_multiplier,
            &currently_covered,
            &mut covered_cex,
            &mut covered_bex_traces,
            &mut score_coverage,
        );

        // Early exit if the intent becomes null
        if currently_covered.is_empty() { break; }
    }

    // println!("Heuristic after phase 1 selected {} predicates, score {} covered BEX traces {}", 
    //     selected.len(), score_total, covered_bex_traces.len());

    // ------------------ Phase 2: Force Must-Not-Cover Constraints ------------------
    
    // Identify BEX traces currently violating the must_not_cover constraint
    let mut violating_must_not_bex: Vec<usize> = instance.bex_traces.iter()
       .filter(|(bid, btr)| btr.must_not_cover && covered_bex_traces.contains(bid))
       .map(|(bid, _)| *bid)
       .collect();

    while!violating_must_not_bex.is_empty() {
        let mut best_fix: Option<(usize, BitSetWrapper, i64)> = None;

        for (&pid, _) in &instance.predicates {
            if selected.contains(&pid) { continue; }
            
            // Evaluate viability and gain
            if let Some((gain, next_covered)) = eval_gain(
                instance,
                &cex_trace_states,
                &bex_trace_states,
                &cex_weights,
                &bex_weights,
                bex_multiplier,
                &covered_cex,
                &covered_bex_traces,
                &currently_covered,
                pid,
                &required_ids
            ) {
                // Check if this predicate addition *fixes* any currently violated MUST_NOT constraints
                let fixes_must_not = violating_must_not_bex.iter()
                   .any(|&bid| {
                        // A fix occurs if the old intent I covered the BEX trace, but the new intent J does not.
                        // (I ∩ T!= ∅) AND (J ∩ T == ∅)
                        bex_trace_states[&bid].intersection(&next_covered).collect().is_empty()
                    });

                if fixes_must_not {
                    // Choose the predicate that fixes a violation AND provides the highest score gain (less degradation).
                    if best_fix.as_ref().map(|(_,_,g)| gain > *g).unwrap_or(true) {
                        best_fix = Some((pid, next_covered, gain));
                    }
                }
            }
        }
        
        if best_fix.is_none() {
            // Cannot find any predicate that fixes a violation without violating other constraints.
            break;
        }

        // Accept the best fix
        let (pid, next_covered, _gain) = best_fix.unwrap();
        currently_covered = next_covered;
        selected.insert(pid);
        // score_total += gain;
        
        // Recalculate full coverage status based on the fix
        recompute_from_currently_covered(
            &cex_trace_states,
            &bex_trace_states,
            &cex_weights,
            &bex_weights,
            bex_multiplier,
            &currently_covered,
            &mut covered_cex,
            &mut covered_bex_traces,
            &mut score_coverage,
        );
        
        // Update the list of violating traces for the next iteration
        violating_must_not_bex = instance.bex_traces.iter()
           .filter(|(bid, btr)| btr.must_not_cover && covered_bex_traces.contains(bid))
           .map(|(bid, _)| *bid)
           .collect();
    }

    // println!("Heuristic selected {} predicates, final score {} covered BEX traces {}", 
    //     selected.len(), score_total, covered_bex_traces.len());

    // --- Final Safety Check ---
    if violating_must_not_bex.iter().next().is_some() {
        // If violations still exist, the heuristic failed to find a valid rule.
        println!("Failed must_not safety check: {} BEX traces still covered.", violating_must_not_bex.len());
        return None;
    }

    // --- Final Result Compilation ---
    let covered_traces: i64 = covered_cex.iter().map(|tid| cex_weights[tid]).sum();
    let bex_penalty_total: i64 = covered_bex_traces.iter().map(|tid| bex_weights[tid]).sum();
    
    // Recalculate total invariant cost for the final report struct integrity
    let total_invariant_cost: i64 = selected.iter().map(|&pid| {
        costs::get_invariant_cost(
            instance.cex_traces.len(),
            instance.get_total_bex_weight(),
            &instance.predicates[&pid].predicate.to_invariant(),
            instance.formula_score_weights.predicate_base_cost,
        ) as i64
    }).sum();
    
    let final_score: i64 = covered_traces - bex_penalty_total * bex_multiplier - total_invariant_cost;
    
    Some(HeuristicResult {
        selected_predicates: selected.into_iter().collect(),
        score: final_score,
        covered_traces,
        b_penalty: bex_penalty_total,
    })
}