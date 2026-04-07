use std::collections::{HashMap, HashSet};
use indicatif::ParallelProgressIterator;
use rayon::prelude::*;
use itertools::Itertools;
use dashmap::DashMap;

use crate::{constants, costs};
use crate::data_types::general_data_types::{self, BitSetWrapper, StateIdType};
use crate::set_cover_instance::{SetCoverPredicate, SetCoverBEXSample, SetCoverCEXTrace, SetCoverInstance, SetCoverBEXTrace};

#[derive(Clone)]
struct UFNode {
    parent: StateIdType,
    weight: u64,
    must_not_cover: bool,
}

pub struct UnionFind {
    nodes: HashMap<StateIdType, UFNode>,
}

impl UnionFind {
    fn new(bex: &HashMap<StateIdType, SetCoverBEXSample, general_data_types::DefaultScalarHasher>) -> Self {
        let mut nodes = HashMap::new();
        for (&sid, b) in bex {
            nodes.insert(
                sid,
                UFNode {
                    parent: sid,
                    weight: b.weight,
                    must_not_cover: b.must_not_cover,
                },
            );
        }
        UnionFind { nodes }
    }

    fn find(&mut self, x: StateIdType) -> StateIdType {
        let parent = self.nodes[&x].parent;
        if parent != x {
            let root = self.find(parent);
            self.nodes.get_mut(&x).unwrap().parent = root;
            root
        } else {
            x
        }
    }

    pub fn find_non_mut(&self, mut x: StateIdType) -> StateIdType {
        while self.nodes[&x].parent != x {
            x = self.nodes[&x].parent;
        }
        x
    }

    fn union(&mut self, dominator: StateIdType, dominated: StateIdType) {
        let root1 = self.find(dominator);
        let root2 = self.find(dominated);
        if root1 == root2 {
            return;
        }
        // Attach dominated to dominator
        self.nodes.get_mut(&root2).unwrap().parent = root1;
        let (w2, must2) = {
            let n2 = &self.nodes[&root2];
            (n2.weight, n2.must_not_cover)
        };
        let n1 = self.nodes.get_mut(&root1).unwrap();
        n1.weight += w2;
        if must2 {
            n1.must_not_cover = true;
        }
    }

    fn into_bex(
        self,
        inst: &SetCoverInstance,
    ) -> HashMap<StateIdType, SetCoverBEXSample, general_data_types::DefaultScalarHasher> {
        let mut reps: HashMap<StateIdType, SetCoverBEXSample, general_data_types::DefaultScalarHasher> =
            HashMap::default();
        for (&sid, node) in &self.nodes {
            let root = self.nodes[&sid].parent;
            if root == sid {
                // Representative → build sample
                let mut sample = inst.bex_samples[&sid].clone();
                sample.weight = node.weight;
                sample.must_not_cover = node.must_not_cover;
                reps.insert(sid, sample);
            }
        }
        reps
    }
}


/// Utility: build predicate index mapping (usize → u32).
fn build_pred_index(predicates: &HashMap<usize, SetCoverPredicate>) -> HashMap<usize, u32> {
    let mut pred_ids: Vec<usize> = predicates.keys().copied().collect();
    pred_ids.sort_unstable();
    pred_ids
        .into_iter()
        .enumerate()
        .map(|(i, pid)| (pid, i as u32))
        .collect()
}

// /// Compute support sets: for each trace, which predicates can cover it.
// fn compute_trace_supports(
//     inst: &SetCoverInstance,
//     pid_to_u: &HashMap<usize, u32>,
// ) -> HashMap<usize, BitSetWrapper> {
//     let mut support: HashMap<usize, BitSetWrapper> = HashMap::new();
//     for (tid, tr) in &inst.cex_traces {
//         let mut states_bs = BitSetWrapper::new();
//         states_bs.insert_all(tr.states.keys().map(|sid| *sid as u32));
//         let mut supp = BitSetWrapper::new();
//         for (pid, p) in &inst.predicates {
//             if !p.covered_states.intersection(&states_bs).collect().is_empty() {
//                 supp.insert(pid_to_u[pid]);
//             }
//         }
//         support.insert(*tid, supp);
//     }
//     support
// }

/// Compute support sets: for each B-state, which predicates cover it.
fn compute_b_supports(
    inst: &SetCoverInstance,
    pid_to_u: &HashMap<usize, u32>,
) -> HashMap<StateIdType, BitSetWrapper> {

    let support: HashMap<StateIdType, BitSetWrapper> = inst
        .bex_samples
        .keys()
        .par_bridge()
        .progress_count(inst.bex_samples.keys().len() as u64)

        .map(|sid| {
            let mut supp = BitSetWrapper::new();
            for (pid, p) in &inst.predicates {
                if p.covered_states.contains(&(*sid as u32)) {
                    supp.insert(pid_to_u[pid]);
                }
            }
            (*sid, supp)
        })
        .collect();
    support
}

pub fn prune_b_states_unionfind_multiround(
    inst: &SetCoverInstance,
    b_support: &HashMap<StateIdType, BitSetWrapper>,
    must_cover_only: bool,
) -> (HashMap<StateIdType, SetCoverBEXSample, general_data_types::DefaultScalarHasher>, HashMap<StateIdType, StateIdType>) {
    let before = inst.bex_samples.len();

    // Build inverted index: predicate -> B-states
    let mut inverted: HashMap<u32, HashSet<StateIdType>> = HashMap::new();
    for (&sid, supp) in b_support {
        for pred in supp.clone() {
            inverted.entry(pred).or_default().insert(sid);
        }
    }

    let cex_covers: DashMap<StateIdType, BitSetWrapper> = DashMap::new();

    inst.predicates
        .par_iter() // rayon parallel iterator over HashMap values
        .for_each(|(_, predicate)| {
            for state_id in predicate.covered_states.clone() {
                let mut entry = cex_covers.entry(state_id as u64).or_insert_with(BitSetWrapper::new);
                entry.insert(predicate.id as u32);
            }
        });

    // Convert DashMap -> regular HashMap (optional)
    let cex_covers: HashMap<StateIdType, BitSetWrapper> = cex_covers.into_iter().collect();

        // -------------------------------------------------------------------------
    // HELPER: CEX-compatibility check using precomputed cex_covers
    // -------------------------------------------------------------------------
    let cex_compatible = |s1: StateIdType,
                          s2: StateIdType,
                          _inst: &SetCoverInstance,
                          b_support: &HashMap<StateIdType, BitSetWrapper>,
                          cex_covers: &HashMap<StateIdType, BitSetWrapper>|
     -> bool {
        let supp1 = &b_support[&s1];
        let supp2 = &b_support[&s2];
        let unsafe_merge = cex_covers.par_iter().any(|(_c_state, cov_c)| {
            // Optimize: use is_subset instead of counting
            let sep_from_s2 = !cov_c.is_subset(supp2);
            if !sep_from_s2 {
                return false; // not separable from s2, can't fail the check
            }
            //We can separate c by using p \in cov(c), \not \in supp(s2)
            let sep_from_s1 = !cov_c.is_subset(supp1);
            //but then if no p exists to separate c from s1, unsafe
            sep_from_s2 && !sep_from_s1 // unsafe merge condition
        });
        return !unsafe_merge;
    };

    // Initialize union-find
    let mut uf = UnionFind::new(&inst.bex_samples);

    let mut round = 0;
    let mut current_b_support = b_support.clone();
    // Before the round loop
    let subset_cache: DashMap<(StateIdType, StateIdType), bool> = DashMap::new();
    //(0) issubset of (1)
    let subset_check = |s2: StateIdType, s1: StateIdType, b_support: &HashMap<StateIdType, BitSetWrapper>| -> bool {
        if let Some(res) = subset_cache.get(&(s2, s1)) {
            return *res;
        }
        let supp1 = &b_support[&s1];
        let supp2 = &b_support[&s2];
        let res = supp2.is_subset(supp1);
        subset_cache.insert((s2, s1), res);
        res
    };


    loop {
        round += 1;

        // --- Phase 1: parallel candidate search ---
        let domination_pairs: Vec<(StateIdType, StateIdType)> = current_b_support
            .par_iter()
            .progress_count(b_support.len() as u64)
            .filter_map(|(&b2, supp2)| {
                let root2 = uf.find_non_mut(b2);
                let node2 = &uf.nodes[&root2];

                // if node2.must_not_cover || supp2.len() == 0 {
                //     return None;
                // }
                if supp2.len() == 0 {
                    return None;
                }
                let w2 = node2.weight;

                // Intersect inverted lists for preds in supp2
                let mut candidates: Option<HashSet<StateIdType>> = None;
                for pred in supp2.clone() {
                    if let Some(cset) = inverted.get(&pred) {
                        candidates = Some(match candidates {
                            None => cset.clone(),
                            Some(curr) => curr.intersection(cset).copied().collect(),
                        });
                    } else {
                        return None;
                    }
                }

                if let Some(cset) = candidates {
                    for b1 in cset {
                        if b1 == b2 {
                            continue;
                        }
                        let root1 = uf.find_non_mut(b1);
                        let node1 = &uf.nodes[&root1];
                        if node2.must_not_cover {
                            if !node1.must_not_cover {
                                continue;
                            }
                        }
                        let min_weight_w1 = if node1.must_not_cover {
                                0   
                            } else {
                                if must_cover_only {
                                    continue;
                                }
                                if constants::SOLVE_STATES_INSTEAD_OF_TRACES {
                                    w2
                                } else if constants::HEURISTIC_MERGE_STATES_OPTIMIZATION {
                                    w2*10
                                } else {
                                    w2
                                }
                            };
                        if !(node1.must_not_cover) && node1.weight < min_weight_w1 {
                            continue;
                        }
                        // let supp1 = &b_support[&root1];
                        // if supp2.is_subset(supp1) {
                        if subset_check(root2,root1 , b_support) {
                            if node1.must_not_cover {
                                return Some((root1, root2));
                            }
                            if must_cover_only {
                                continue;
                            }
                            let compatible = cex_compatible(root1, root2, inst, b_support, &cex_covers);
                            if !compatible && !(node1.must_not_cover) {
                                continue;
                            }
                            return Some((root1, root2)); // candidate pair
                        }
                    }
                }
                None
            })
            .collect();

        // --- Phase 2: sequential re-check + union ---
        let mut merges = 0;
        for (b1, b2) in domination_pairs {
            let r1 = uf.find(b1);
            let r2 = uf.find(b2);
            if r1 == r2 {
                continue; // already merged
            }

            let n1 = &uf.nodes[&r1];
            let n2 = &uf.nodes[&r2];

            if n2.must_not_cover {
                if !n1.must_not_cover {
                    continue;
                }
            }
            if !n1.must_not_cover && n1.weight < n2.weight {
                continue;
            }
            // if !n2.support.is_subset(&n1.support) {
            //     continue;
            // }
           
            // Re-check with original support
            if !subset_check(r2, r1, b_support) {
                continue;
            }
            let do_merge_flag;
            if n1.must_not_cover {
                do_merge_flag = true;
            } else {
                let compatible = cex_compatible(r1, r2, inst, b_support, &cex_covers);
                if !compatible && !(n1.must_not_cover) {
                    do_merge_flag = false;
                } else {
                    do_merge_flag = true;
                }
            }
            if do_merge_flag == false {
                continue;
            }
            uf.union(r1, r2);
            
            
            // Update support: merge r2's support into r1

            // uf.union(r1, r2);
            merges += 1;
            current_b_support.remove(&b2);
            
        }

        println!("Round {round}: performed {merges} merges");

        if merges == 0 {
            break; // fixpoint reached
        }
    }
    let old_to_new_map : HashMap<StateIdType, StateIdType> = inst
    .bex_samples
    .keys()
    .map(|&sid| (sid, uf.find_non_mut(sid)))
    .collect();

    // Collapse into final BEX map
    let after_map = uf.into_bex(inst);

    let after = after_map.len();

    let total_before: u64 = inst.bex_samples.values().map(|b| b.weight).sum();
    let total_after: u64 = after_map.values().map(|b| b.weight).sum();

    println!(
        "Pruned B-states (UF multi-round): {} → {} (removed {}), total weight {} → {}",
        before,
        after,
        before - after,
        total_before,
        total_after
    );

    (after_map, old_to_new_map)
}


/// Step 2: Keep only traces with non-empty support.  
/// Returns an error if the required trace has empty support (→ infeasible).
pub fn keep_nonempty_traces(
    inst: &SetCoverInstance,
    trace_support: &HashMap<usize, BitSetWrapper>,
    _required_trace_id: usize,
) -> Result<HashSet<usize>, String> {
    let before = inst.cex_traces.len();
    let mut kept: HashSet<usize> = HashSet::new();

    for (&tid, supp) in trace_support {
        if supp.len() > 0 {
            kept.insert(tid);
        }
    }

    // if !kept.contains(&required_trace_id) {
    //     return Err(format!(
    //         "Required trace {} has empty support → instance infeasible",
    //         required_trace_id
    //     ));
    // }

    let after = kept.len();
    println!(
        "Traces (non-empty support): {} → {} (removed {})",
        before,
        after,
        before - after
    );
    Ok(kept)
}

/// Rule 3: merge identical traces.
pub fn merge_traces_identical_supports(
    inst: &SetCoverInstance,
    kept_traces: &HashSet<usize>,
    trace_support: &HashMap<usize, BitSetWrapper>,
    required_trace_id: usize,
) -> HashMap<usize, SetCoverCEXTrace,  general_data_types::DefaultScalarHasher> {
    let mut support_groups: HashMap<BitSetWrapper, Vec<usize>> = HashMap::new();
    for &tid in kept_traces {
        support_groups
            .entry(trace_support[&tid].clone())
            .or_default()
            .push(tid);
    }

    let before = kept_traces.len();
    let mut new_traces: HashMap<usize, SetCoverCEXTrace, general_data_types::DefaultScalarHasher> = HashMap::default();

    for (_supp, tids) in support_groups {
        if tids.len() == 1 {
            let tid = tids[0];
            new_traces.insert(tid, inst.cex_traces[&tid].clone());
        } else {
            // Pick representative
            let rep_id = if tids.contains(&required_trace_id) {
                required_trace_id
            } else {
                *tids.iter().min().unwrap()
            };
            let mut rep = inst.cex_traces[&rep_id].clone();

            // Sum weights
            rep.weight = tids.iter().map(|tid| inst.cex_traces[tid].weight).sum();

            // Propagate OriginalCex if any
            let any_original = tids.iter().any(|tid| {
                matches!(
                    inst.cex_traces[tid].file_source,
                    Some(general_data_types::WaveFormSource::OriginalCex)
                )
            });
            if any_original {
                rep.file_source = Some(general_data_types::WaveFormSource::OriginalCex);
            }

            new_traces.insert(rep_id, rep);
        }
    }

    let after = new_traces.len();
    let total_before: u64 = kept_traces.iter().map(|tid| inst.cex_traces[tid].weight).sum();
    let total_after: u64 = new_traces.values().map(|tr| tr.weight).sum();
    println!(
        "Merged identical trace supports: {} → {} (total weight {} → {})",
        before,
        after,
        total_before,
        total_after
    );

    new_traces
}

fn merge_traces_identical_bex_traces(
    inst: &mut SetCoverInstance,
) {
    let before = inst.bex_traces.len();
    let mut state_support_groups: HashMap<BitSetWrapper, Vec<usize>> = HashMap::new();
    for &tid in inst.bex_traces.keys() {
        let mut states_bs = BitSetWrapper::new();
        states_bs.insert_all(inst.bex_traces[&tid].states.keys().map(|sid| *sid as u32));
        state_support_groups
            .entry(states_bs)
            .or_default()
            .push(tid);
    }
    let mut new_traces: HashMap<usize, SetCoverBEXTrace, general_data_types::DefaultScalarHasher> = HashMap::default();
    for (_supp, tids) in state_support_groups {
        if tids.len() == 1 {
            let tid = tids[0];
            new_traces.insert(tid, inst.bex_traces[&tid].clone());
        } else {
            // Pick representative
            let rep_id = *tids.iter().min().unwrap();
            let mut rep = inst.bex_traces[&rep_id].clone();

            // Sum weights
            rep.weight = tids.iter().map(|tid| inst.bex_traces[tid].weight).sum();

            // Propagate OriginalCex if any
            let must_not_cover = tids.iter().any(|tid| {
                inst.bex_traces[tid].must_not_cover
            });
            if must_not_cover {
                rep.must_not_cover = true;
            }

            new_traces.insert(rep_id, rep);
        }
    }
    let after = new_traces.len();
    println!(
        "Merged identical BEX trace supports: {} → {} (merged {})",
        before,
        after,
        before - after
    );
    inst.bex_traces = new_traces;
}

pub fn merge_traces_identical_cex_traces(
    inst: &mut SetCoverInstance,
) {
    let before = inst.cex_traces.len();
    let mut state_support_groups: HashMap<BitSetWrapper, Vec<usize>> = HashMap::new();
    for &tid in inst.cex_traces.keys() {
        let mut states_bs = BitSetWrapper::new();
        states_bs.insert_all(inst.cex_traces[&tid].states.keys().map(|sid| *sid as u32));
        state_support_groups
            .entry(states_bs)
            .or_default()
            .push(tid);
    }
    let mut new_traces: HashMap<usize, SetCoverCEXTrace, general_data_types::DefaultScalarHasher> = HashMap::default();
    for (_supp, tids) in state_support_groups {
        if tids.len() == 1 {
            let tid = tids[0];
            new_traces.insert(tid, inst.cex_traces[&tid].clone());
        } else {
            // Pick representative
            let rep_id = *tids.iter().min().unwrap();
            let mut rep = inst.cex_traces[&rep_id].clone();

            // Sum weights
            rep.weight = tids.iter().map(|tid| inst.cex_traces[tid].weight).sum();

            // Propagate OriginalCex if any
            let any_original = tids.iter().any(|tid| {
                matches!(
                    inst.cex_traces[tid].file_source,
                    Some(general_data_types::WaveFormSource::OriginalCex)
                )
            });
            if any_original {
                rep.file_source = Some(general_data_types::WaveFormSource::OriginalCex);
            }

            new_traces.insert(rep_id, rep);
        }
    }
    inst.cex_traces = new_traces;
    let after = inst.cex_traces.len();
    println!(
        "Merged identical CEX trace supports: {} → {} (merged {})",
        before,
        after,
        before - after
    );
}

/// Rule 4: merge identical B-states.
fn merge_b_states(
    kept_b: &HashMap<StateIdType, SetCoverBEXSample, general_data_types::DefaultScalarHasher>,
    b_support: &HashMap<StateIdType, BitSetWrapper>,
    old_to_new_map: &HashMap<StateIdType, StateIdType>,
    inst: &mut SetCoverInstance,
) {
    let before = kept_b.len();
    let mut support_groups: HashMap<BitSetWrapper, Vec<StateIdType>> = HashMap::new();
    for sid in kept_b.keys() {
        support_groups
            .entry(b_support[sid].clone())
            .or_default()
            .push(*sid);
    }

    let mut new_bex: HashMap<u64, SetCoverBEXSample, general_data_types::DefaultScalarHasher> = HashMap::default();
    for (_supp, sids) in support_groups.iter() {
        if sids.len() == 1 {
            let sid = sids[0];
            new_bex.insert(sid, kept_b[&sid].clone());
        } else {
            let rep_sid = *sids.iter().min().unwrap();
            let mut rep = kept_b[&rep_sid].clone();
            rep.weight = sids.iter().map(|sid| kept_b[&sid].weight).sum();
            rep.must_not_cover = sids.iter().any(|sid| kept_b[&sid].must_not_cover);
            new_bex.insert(rep_sid, rep);
        }
    }
    inst.bex_samples = new_bex;
    for trace in inst.bex_traces.values_mut() {
        let mut new_states: HashMap<u64, crate::set_cover_instance::UnweightedBEXSample, general_data_types::DefaultScalarHasher> = HashMap::default();
        for (sid, sample) in &trace.states {
            let new_sid = if kept_b.get(sid).is_none() {
                old_to_new_map.get(sid).unwrap()
            } else {
                sid
            };
            let rep_sid = *new_sid;
            let rep_sid = *support_groups[&b_support[&rep_sid]]
                .iter()
                .min()
                .unwrap();
            let mut merged = sample.clone();
            merged.state_id = rep_sid; //UnweightedBEXSample does not have weights, it is enough if we set the state_id
            new_states.entry(rep_sid).or_insert(merged);
        }
        trace.states = new_states;
    }
    let after = inst.bex_samples.len();
    println!(
        "Merged identical B-states: {} → {} (merged {})",
        before,
        after,
        before - after
    );
}

/// Rule 5: merge identical states across all_states.
fn merge_states(inst: &mut SetCoverInstance) {
    let before = inst.all_states.len();


    // Build support for each state - parallelized with progress tracking
    let state_support: HashMap<StateIdType, BitSetWrapper> = inst
        .all_states
        .par_iter()
        .progress_count(inst.all_states.len() as u64)
        .map(|sid| {
            let mut supp = BitSetWrapper::new();
            for (pid, p) in &inst.predicates {
                if p.covered_states.contains(&(*sid as u32)) {
                    supp.insert(*pid as u32);
                }
            }
            (*sid, supp)
        })
        .collect();

    // Group identical
    let mut groups: HashMap<BitSetWrapper, Vec<StateIdType>> = HashMap::new();
    for (sid, supp) in &state_support {
        groups.entry(supp.clone()).or_default().push(*sid);
    }

    // Build representative map
    let mut rep_map: HashMap<StateIdType, StateIdType> = HashMap::new();
    for (_supp, sids) in groups {
        let rep = *sids.iter().min().unwrap();
        for sid in sids {
            rep_map.insert(sid, rep);
        }
    }

    // Rewrite predicates
    for p in inst.predicates.values_mut() {
        let old_states: Vec<u32> = p.covered_states.collect();
        let mut new_bs = BitSetWrapper::new();
        for sid in old_states {
            let rep = rep_map.get(&(sid as StateIdType)).unwrap();
            new_bs.insert(*rep as u32);
        }
        p.covered_states = new_bs;
    }

    // Rewrite BEX
    let mut new_bex: HashMap<u64, SetCoverBEXSample, general_data_types::DefaultScalarHasher> = HashMap::default();
    for (sid, bex) in &inst.bex_samples {
        let rep = rep_map[sid];
        let mut merged = bex.clone();
        merged.state_id = rep;
        new_bex
            .entry(rep)
            .and_modify(|b: &mut SetCoverBEXSample| {
                b.weight += merged.weight;
                b.must_not_cover |= merged.must_not_cover;
            })
            .or_insert(merged);
    }
    inst.bex_samples = new_bex;

    // Rewrite traces
    let mut new_traces: HashMap<usize, crate::set_cover_instance::SetCoverCEXTrace, general_data_types::DefaultScalarHasher> = HashMap::default();
    for (tid, tr) in &inst.cex_traces {
        let mut new_states: HashMap<u64, crate::set_cover_instance::SetCoverCEXSample, general_data_types::DefaultScalarHasher> = HashMap::default();
        for (sid, sample) in &tr.states {
            let rep = rep_map[sid];
            let mut merged = sample.clone();
            merged.state_id = rep;
            new_states.entry(rep).or_insert(merged);
        }
        let mut new_tr = tr.clone();
        new_tr.states = new_states;
        new_traces.insert(*tid, new_tr);
    }
    inst.cex_traces = new_traces;
    let mut new_bex_traces : HashMap<usize, crate::set_cover_instance::SetCoverBEXTrace, general_data_types::DefaultScalarHasher> = HashMap::default();
    for (tid, tr) in &inst.bex_traces {
        let mut new_states: HashMap<u64, crate::set_cover_instance::UnweightedBEXSample, general_data_types::DefaultScalarHasher> = HashMap::default();
        for (sid, sample) in &tr.states {
            let rep = rep_map[sid];
            let mut merged = sample.clone();
            merged.state_id = rep;
            new_states.entry(rep).or_insert(merged);
        }
        let mut new_tr = tr.clone();
        new_tr.states = new_states;
        new_bex_traces.insert(*tid, new_tr);
    }
    inst.bex_traces = new_bex_traces;

    // Rewrite all_states
    inst.all_states = rep_map.values().copied().collect();

    let after = inst.all_states.len();
    println!(
        "Merged identical states: {} → {} (merged {})",
        before,
        after,
        before - after
    );
}

/// Rule 6: remove states that are not used in any BEX sample or CEX trace.
fn remove_unused_states(inst: &mut SetCoverInstance) {
    let before = inst.all_states.len();

    // Collect all states used in traces and BEX
    let mut used: HashSet<StateIdType> = HashSet::new();
    used.extend(inst.bex_samples.keys().copied());
    for tr in inst.cex_traces.values() {
        used.extend(tr.states.keys().copied());
    }

    // Filter predicates
    for p in inst.predicates.values_mut() {
        let old_states: Vec<u32> = p.covered_states.collect();
        let mut new_bs = BitSetWrapper::new();
        for sid in old_states {
            if used.contains(&(sid as StateIdType)) {
                new_bs.insert(sid);
            }
        }
        p.covered_states = new_bs;
    }

    // Shrink all_states
    inst.all_states = used.into_iter().collect();

    let after = inst.all_states.len();
    println!(
        "Removed unused states: {} → {} (removed {})",
        before,
        after,
        before - after
    );
}


/// Rule 7: merge identical predicates, keeping only the cheapest one.
fn merge_predicates(inst: &mut SetCoverInstance) {
    let before = inst.predicates.len();

    // Group by covered_states
    let mut groups: HashMap<BitSetWrapper, Vec<usize>> = HashMap::new();
    for (pid, pred) in &inst.predicates {
        groups.entry(pred.covered_states.clone()).or_default().push(*pid);
    }

    let mut new_preds = HashMap::new();
    let total_cex_weight = inst.get_total_cex_weight() as usize;
    let total_bex_weight = inst.get_total_bex_weight();
    for (_supp, pids) in groups {
        if pids.len() == 1 {
            let pid = pids[0];
            new_preds.insert(pid, inst.predicates[&pid].clone());
        } else {
            // Pick cheapest predicate
            let rep_id = pids
                .iter()
                .min_by_key(|pid| costs::get_invariant_cost(total_cex_weight, total_bex_weight, &inst.predicates[pid].predicate.to_invariant(), inst.formula_score_weights.predicate_base_cost).round() as i64)
                .unwrap();
            let mut new_predicate = inst.predicates[rep_id].clone();
            let mut new_only_in_cycles: Option<Vec<usize>> = None;
            for pid in &pids {
                let p = &inst.predicates[pid];
                let this_oic = &p.predicate.only_in_cycles;
                if this_oic.is_none() {
                    new_only_in_cycles = None;
                    break;
                } else {
                    if let Some(curr) = new_only_in_cycles.as_mut() {
                        curr.extend(this_oic.as_ref().unwrap());
                    } else {
                        new_only_in_cycles = Some(this_oic.as_ref().unwrap().clone());
                    }
                }
            }
            new_predicate.predicate.only_in_cycles = new_only_in_cycles;
            new_preds.insert(*rep_id, new_predicate);
        }
    }
    inst.predicates = new_preds;
    let after = inst.predicates.len();
    println!(
        "Merged identical predicates: {} → {} (merged {})",
        before,
        after,
        before - after
    );
}

/// Classify all states into:
/// - CEX-only
/// - BEX-only
/// - Overlapping (in both)
pub fn classify_states(
    inst: &SetCoverInstance,
) -> (HashSet<StateIdType>, HashSet<StateIdType>, HashSet<StateIdType>) {
    // --- Collect all CEX states
    let cex_states: HashSet<StateIdType> = inst
        .cex_traces
        .iter()
        .flat_map(|(_, tr)| tr.states.keys().copied())
        .collect();

    // --- Collect all BEX states
    let bex_states: HashSet<StateIdType> = inst.bex_samples.keys().copied().collect();

    // --- Compute intersections
    let overlapping: HashSet<_> = cex_states
        .intersection(&bex_states)
        .copied()
        .collect();

    // --- Compute exclusive subsets
    let cex_only: HashSet<_> = cex_states
        .difference(&bex_states)
        .copied()
        .collect();

    let bex_only: HashSet<_> = bex_states
        .difference(&cex_states)
        .copied()
        .collect();

    println!(
        "State classification: CEX-only = {}, BEX-only = {}, overlapping = {}",
        cex_only.len(),
        bex_only.len(),
        overlapping.len()
    );

    (cex_only, bex_only, overlapping)
}

pub fn remove_states_from_predicates(
    inst: &mut SetCoverInstance,
    states_to_remove: &HashSet<StateIdType>,
) {
    // Remove from predicates
    for p in inst.predicates.values_mut() {
        let old_states: Vec<u32> = p.covered_states.collect();
        let mut new_bs = BitSetWrapper::new();
        for sid in old_states {
            if !states_to_remove.contains(&(sid as StateIdType)) {
                new_bs.insert(sid);
            }
        }
        p.covered_states = new_bs;
    }
}

/// Remove CEX states that are covered by *all* predicates,
/// then remove CEX traces that become empty.
///
/// Safe because such states are always covered (for any non-empty predicate set),
/// and thus redundant. BEX states are not modified.
pub fn remove_already_covered_cex_states_and_empty_traces(
    inst: &mut SetCoverInstance,
) -> (usize, usize) {
    let total_preds = inst.predicates.len();
    assert!(total_preds > 0, "Expected at least one predicate.");

    // Precompute coverage counts for each state 
    let mut coverage_count: HashMap<u64, usize> = HashMap::new();
    let (cex_only_states, _, _) = classify_states(inst);
    for sid in cex_only_states {
        coverage_count.insert(sid as u64, 0);
    }
    for (_pid, pred) in &inst.predicates {
        for sid in pred.covered_states.clone() {
            if coverage_count.contains_key(&(sid as u64)) {
                *coverage_count.get_mut(&(sid as u64)).unwrap() += 1;
            }
        }
    }
    let states_to_remove: HashSet<u64> = coverage_count
        .iter()
        .filter_map(|(&sid, &count)| if count == total_preds { Some(sid) } else { None })
        .collect();
    let len_before = inst.all_states.len();
    inst.all_states.retain(|sid| {
        if let Some(count) = coverage_count.get(&(*sid as u64)) {
            *count < total_preds
        } else {
            true // keep BEX-only and overlapping states
        }
    });
    let len_after = inst.all_states.len();

    let removed_states = len_before - len_after;
    let mut traces_to_remove = Vec::new();


    for (tid, tr) in inst.cex_traces.iter_mut() {
        let has_universally_covered_state = tr
            .states
            .keys()
            .any(|sid| coverage_count.get(sid).copied().unwrap_or(0) == total_preds);
        if has_universally_covered_state {
            traces_to_remove.push(*tid);
        }
    }

    for tid in &traces_to_remove {
        inst.cex_traces.remove(tid);
    }

    let removed_traces = traces_to_remove.len();
    remove_states_from_predicates(inst, &states_to_remove);

    println!(
        "Removed {} (sanity: {}) universally-covered CEX states and {} empty traces.",
        removed_states,states_to_remove.len(), removed_traces
    );

    (removed_states, removed_traces)
}


/// Removes dominated predicates efficiently using BitSet inclusion checks.
/// A predicate p1 is dominated by p2 if:
///   - CEX(p1) ⊆ CEX(p2)
///   - BEX(p1) ⊇ BEX(p2)
///   - cost(p1) ≥ cost(p2)
pub fn remove_dominated_predicates_fast(this_instance: &mut SetCoverInstance) -> usize {
    println!("Removing dominated predicates (fast BitSet version)...");

    // Collect IDs of BEX and CEX states
    let bex_states: HashSet<u32> = this_instance.bex_samples.keys().map(|&s| s as u32).collect();
    let cex_states: HashSet<u32> = this_instance
        .cex_traces
        .values()
        .flat_map(|t| t.states.keys().map(|&sid| sid as u32))
        .collect();

    // Precompute per-predicate cost and separated supports
    #[derive(Clone)]
    struct PredInfo {
        pid: usize,
        cex_cov: BitSetWrapper,
        bex_cov: BitSetWrapper,
        cost: f64,
    }

    let infos: Vec<PredInfo> = this_instance
        .predicates
        .iter()
        .map(|(&pid, p)| {
            let cost = costs::get_invariant_cost(
                100, 100.0,
                &p.predicate.to_invariant(),
                this_instance.formula_score_weights.predicate_base_cost,
            );

            let mut cex_bs = BitSetWrapper::new();
            let mut bex_bs = BitSetWrapper::new();
            for s in p.covered_states.clone().into_iter() {
                if cex_states.contains(&s) {
                    cex_bs.insert(s);
                }
                if bex_states.contains(&s) {
                    bex_bs.insert(s);
                }
            }

            PredInfo {
                pid,
                cex_cov: cex_bs,
                bex_cov: bex_bs,
                cost,
            }
        })
        .sorted_by_key(|pi| pi.cex_cov.len()) // smaller coverage first
        .collect();

    let mut dominated = BitSetWrapper::new(); // using BitSet for efficiency if possible

    // Pairwise dominance check
    for i in 0..infos.len() {
        let p1 = &infos[i];
        if dominated.contains(&(p1.pid as u32)) {
            continue;
        }

        for j in (i + 1)..infos.len() {
            let p2 = &infos[j];
            if dominated.contains(&(p2.pid as u32)) {
                continue;
            }

            // // Skip if sizes make domination impossible
            // if p1.cex_cov.len() > p2.cex_cov.len() || p1.bex_cov.len() < p2.bex_cov.len() {
            //     continue; // p1 can't be dominated by p2
            // }

            // p1 dominated by p2 ?
            if p1.cex_cov.is_subset(&p2.cex_cov)
                && p1.bex_cov.is_superset(&p2.bex_cov)
                && p1.cost >= p2.cost
            {
                dominated.insert(p1.pid as u32);
                break;
            }

            // p2 dominated by p1 ?
            if p2.cex_cov.is_subset(&p1.cex_cov)
                && p2.bex_cov.is_superset(&p1.bex_cov)
                && p2.cost >= p1.cost
            {
                dominated.insert(p2.pid as u32);
            }
        }
    }

    let dom_ids: Vec<usize> = dominated.clone().into_iter().map(|u| u as usize).collect();
    for pid in &dom_ids {
        this_instance.predicates.remove(pid);
    }

    println!(
        "Removed {} dominated predicates ({} remaining).",
        dom_ids.len(),
        this_instance.predicates.len()
    );

    dom_ids.len()
}

fn remove_predicates_below_upper_bound(
    inst: &mut SetCoverInstance,
    upper_bound: f64,
) -> usize {
    let before = inst.predicates.len();
    let new_predicates = inst.predicates.iter().filter(|(&_pid, p)| {
        inst.get_predicate_upper_bound(p) >= upper_bound
    });
    inst.predicates = new_predicates
        .map(|(&pid, p)| (pid, p.clone()))
        .collect();
    let after = inst.predicates.len();
    println!(
        "Removed predicates above upper bound {}: {} → {} (removed {})",
        upper_bound,
        before,
        after,
        before - after
    );
    before - after
}

/// Full preprocessing pipeline with logging.
pub fn preprocess_instance(
    inst: &SetCoverInstance,
    min_objective_value: Option<f64>,
) -> Option<SetCoverInstance> {
    // let required_trace_ids : HashSet<usize> = inst.cex_traces
    //     .iter()
    //     .filter_map(|(&tid, tr)| {
    //         if matches!(
    //             tr.file_source,
    //             Some(general_data_types::WaveFormSource::OriginalCex)
    //         ) {
    //             Some(tid)
    //         } else {
    //             None
    //         }
    //     })
    //     .collect();
    // Build predicate index first
    let _total_bex_start = inst.get_total_bex_weight();
    // --- Step 1: merge states ---
    let mut merged_inst = inst.clone();
    //Removing any trace (empty or non-empty) distorts the weighting function!
    // remove_already_covered_cex_states_and_empty_traces(&mut merged_inst);
    remove_dominated_predicates_fast(&mut merged_inst);
    merge_predicates(&mut merged_inst);
    merge_states(&mut merged_inst);
    merge_traces_identical_bex_traces(&mut merged_inst);
    merge_traces_identical_cex_traces(&mut merged_inst);
    let pid_to_u = build_pred_index(&merged_inst.predicates);
    let b_support: HashMap<u64, BitSetWrapper> = compute_b_supports(&merged_inst, &pid_to_u);
    let must_cover_only = !(constants::HEURISTIC_MERGE_STATES_OPTIMIZATION);
    let (kept_b, old_to_new_map) = prune_b_states_unionfind_multiround(&merged_inst, &b_support, must_cover_only);
    merge_b_states(&kept_b, &b_support, &old_to_new_map, &mut merged_inst);
    remove_unused_states(&mut merged_inst);
    remove_predicates_below_upper_bound(&mut merged_inst, min_objective_value.unwrap_or(f64::MIN));
    // merge_traces_identical_cex_traces(&mut merged_inst);
    return Some(merged_inst);
    // // compute_predicate_components(&merged_inst);

    // let total_bex_after_merge_states = merged_inst.get_total_bex_weight();
    // let pid_to_u = build_pred_index(&merged_inst.predicates);
    // // --- Step 2: prune B-states ---
    // // println!("Vincent1 Before Total B-states before pruning: {}", merged_inst.get_total_bex_weight());
    // let b_support = compute_b_supports(&merged_inst, &pid_to_u);
    // let kept_b = prune_b_states_unionfind_multiround(&merged_inst, &b_support);
    // let total_after_prune: u64 = kept_b.values().map(|b| b.weight).sum();
    // let new_bex = merge_b_states( &kept_b, &b_support);

    // // let total_after_merge_b: u64 = new_bex.values().map(|b| b.weight).sum();
    // let dropped_empty: u64 = merged_inst.bex_samples
    //     .iter()
    //     .filter(|(sid, _)| b_support[sid].len() == 0)
    //     .map(|(_, b)| b.weight)
    //     .sum();

    // println!(
    //     "BEX total weight inst: {} before: {}, after pruning: {} (dropped empty-support weight: {})",
    //     total_bex_start, total_bex_after_merge_states, total_after_prune, dropped_empty
    // );
    // let mut new_all_states : HashSet<StateIdType, general_data_types::DefaultScalarHasher> = new_bex.keys().copied().collect();
    // new_all_states.extend(merged_inst.cex_traces.iter().flat_map(|(_,tr)| tr.states.keys().copied()));
    // let mut current_instance = SetCoverInstance {
    //     predicates: merged_inst.predicates.clone(),
    //     bex_samples: new_bex.clone(),
    //     cex_traces: merged_inst.cex_traces.clone(),
    //     all_states: new_all_states,
    //     formula_score_weights: inst.formula_score_weights.clone(),
    // };
    // // return Some(temp_instance);
    // // --- Step 3: prune traces ---
    // println!("Total traces before pruning: {}", current_instance.get_total_cex_weight());
    // let trace_support = compute_trace_supports(&current_instance, &pid_to_u);
    // let kept_traces = keep_nonempty_traces(&current_instance, &trace_support, required_trace_id);
    // let kept_traces = match kept_traces {
    //     Ok(kt) => kt,
    //     Err(e) => {
    //         println!("Preprocessing error: {}", e);
    //         return None;
    //     }
    // };

    // let new_traces: HashMap<usize, SetCoverCEXTrace, general_data_types::DefaultScalarHasher> =
    //     merge_traces_identical_supports(&current_instance, &kept_traces, &trace_support, required_trace_id);
    // println!("Total traces weight after pruning: {}", new_traces.values().map(|tr| tr.weight).sum::<u64>());
    // // --- Build new instance ---
    // let mut new_inst = SetCoverInstance {
    //     predicates: current_instance.predicates.clone(),
    //     bex_samples: new_bex,
    //     cex_traces: new_traces,
    //     all_states: current_instance.all_states.clone(),
    //     formula_score_weights: inst.formula_score_weights.clone(),
    // };
    // // return new_inst;
    // // merge_states(&mut new_inst); //Doing it twice won't help much

    // // --- Step 4: cleanup unused states ---
    // remove_unused_states(&mut new_inst);
    // //return new_inst; // TEMPORARY DISABLE FURTHER STEPS
    // // --- Step 5: merge predicates (keep cheapest) ---
    // //merge_predicates(&mut new_inst);
    // // Step 5: Remove dominated predicates again
    // remove_dominated_predicates_fast(&mut new_inst);
    

    // Some(new_inst)
}
