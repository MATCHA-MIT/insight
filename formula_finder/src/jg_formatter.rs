use crate::{data_types, predicates};
use crate::predicates::Invariant;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::sync::Arc;
use rayon::prelude::*;

/// Checks if a sanitized signal name matches an original RTL name
/// with Verilog escaped identifiers (starting with '\' and ending with space).
pub fn match_signal_names(sanitized: &str, original: &str) -> bool {
    let s_parts: Vec<&str> = sanitized.split('.').collect();
    let mut o_parts = Vec::new();

    // Parse original into hierarchical parts, treating escaped identifiers as single units
    let mut tokens = original.split('.');
    while let Some(part) = tokens.next() {
        if part.starts_with('\\') {
            // Escaped identifier: keep consuming until we find a space
            let mut escaped = part.to_string();
            while !escaped.ends_with(' ') {
                if let Some(next) = tokens.next() {
                    escaped.push('.');
                    escaped.push_str(next);
                } else {
                    break; // malformed?
                }
            }
            // Strip the trailing space (Verilog requires it but it's not part of the name)
            if escaped.ends_with(' ') {
                escaped.pop();
            }
            o_parts.push(escaped);
        } else {
            o_parts.push(part.to_string());
        }
    }

    // Matching logic: step through both sequences
    let mut i = 0;
    let mut j = 0;
    while i < s_parts.len() && j < o_parts.len() {
        let o = &o_parts[j];
        if o.starts_with('\\') {
            // Escaped identifier may span multiple s_parts
            let escaped_unescaped = &o[1..]; // remove '\'
            let mut combined = String::new();
            let mut k = i;
            while k < s_parts.len() && combined.len() < escaped_unescaped.len() {
                if !combined.is_empty() {
                    combined.push('.');
                }
                combined.push_str(s_parts[k]);
                k += 1;
            }
            if combined == escaped_unescaped {
                i = k;
                j += 1;
            } else {
                return false;
            }
        } else {
            if s_parts[i] != o {
                return false;
            }
            i += 1;
            j += 1;
        }
    }

    // Successful match if both sequences consumed fully
    i == s_parts.len() && j == o_parts.len()
}

/// Split a JG-style hierarchical name into parts,
/// keeping escaped identifiers (starting with '\' and ending with a space) together,
/// even if they contain dots.
fn split_jg_parts(jg: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut in_esc = false;

    for seg in jg.split('.') {
        if !in_esc {
            if seg.starts_with('\\') {
                in_esc = true;
                cur.clear();
                cur.push_str(seg);
                if seg.ends_with(' ') {
                    parts.push(cur.clone());
                    cur.clear();
                    in_esc = false;
                }
            } else {
                parts.push(seg.to_string());
            }
        } else {
            cur.push('.');
            cur.push_str(seg);
            if seg.ends_with(' ') {
                parts.push(cur.clone());
                cur.clear();
                in_esc = false;
            }
        }
    }
    parts
}

/// Transform a JG signal path into a Verilator-like path.
/// - Escaped JG identifiers (`\... `) are unescaped and then split on '.' (since
///   Verilator would treat those dots as hierarchy separators).
/// - Everything is then joined by '.' to form the Verilator-style hierarchical name.
///
/// Example:
///   "ibex.dut.cpu.cpu.\\u_ibex_core.data_addr_o"
/// -> "ibex.dut.cpu.cpu.u_ibex_core.data_addr_o"
///
/// Example (dots inside escaped token):
///   "top.block.\\inner.with.dot .sig"
/// -> "top.block.inner.with.dot.sig"
pub fn transform_jg_to_verilator_like(jg: &str) -> String {
    let parts = split_jg_parts(jg);
    let mut out: Vec<String> = Vec::new();

    for p in parts {
        if let Some(rest) = p.strip_prefix('\\') {
            // drop trailing space if present (Verilog escape terminator)
            let core = rest.strip_suffix(' ').unwrap_or(rest);
            // Verilator would split any dots inside escaped token into hierarchy
            if core.contains('.') {
                out.extend(core.split('.').map(|s| s.to_string()));
            } else {
                out.push(core.to_string());
            }
        } else {
            out.push(p);
        }
    }

    out.join(".")
}


pub fn get_jg_assume_command<H>(
    invariant: &predicates::SeparatorFormula,
    signal_remapping: Option<&data_types::signal_remap_info::SignalRemappingInfo>,
    name_to_code_mapping: &HashMap<ustr::Ustr, u64, H>,
) -> String
where
    H: std::hash::BuildHasher
{
    //     // If signal_mapping is None, we use the invariant's own signal names
//     // let signal_mapping = match signal_mapping {
//     //     Some(mapping) => mapping,
//     //     None => &invariant
//     //         .get_signal_names()
//     //         .iter()
//     //         .cloned()
//     //         .map(|s| (s.clone(), s))
//     //         .collect::<HashMap<Arc<str>, Arc<str>>>(),
//     // };
    let invariant_string = format_invariant_with_signal_mapping(invariant, signal_remapping, name_to_code_mapping);
    let assume_invariant = format!("assume {{!({invariant_string})}}");
    assume_invariant
}


pub fn get_jg_assert_command<H>(
    _invariant: &Invariant,
    _signal_mapping: Option<&data_types::signal_remap_info::SignalRemappingInfo>,
    _name_to_code_mapping: &HashMap<ustr::Ustr, u64, H>,
) -> String {
    let ret = format!("/* ASSERT COMMAND NOT IMPLEMENTED ANYMORE */");
    ret
    // let signal_mapping = match signal_mapping {
    //     Some(mapping) => mapping,
    //     None => &invariant
    //         .get_signal_names()
    //         .iter()
    //         .cloned()
    //         .map(|s| (s.clone(), s))
    //         .collect::<HashMap<Arc<str>, Arc<str>>>(),
    // };
    // let invariant_string = format_invariant_with_signal_mapping(invariant, signal_mapping);
    // let double_max_instruction_length = constants_module::MAX_INSTRUCTION_CYCLE_LENGTH * 2;
    // let assert_invariant = format!("assert {{@(posedge clk) disable iff (rst) ({invariant_string})|=> ##{double_max_instruction_length} !(correct)}}");
    // assert_invariant
}

pub fn matches_any_jg_signal<'a>(
    verilator_signal: &Arc<str>,
    jg_signals: &'a HashSet<Arc<str>>,
    jg_signals_transformed: Option<&HashSet<Arc<str>>>,
) -> Option<Arc<str>> {
    // println!("Trying to match signal: {}", verilator_signal);
    // Try to find a matching signal in the JG signals
    // println!("JG signals: {:?}", jg_signals_transformed);
    let verilator_search_signal_name =
        if let Some(stripped) = verilator_signal.strip_prefix("TOP.correctness.") {
            //Arc::from(format!("correctness.{}", stripped))
            Arc::from(stripped)
        } else if let Some(stripped) = verilator_signal.strip_prefix("correctness.") {
            Arc::from(stripped)
        } else {
         verilator_signal.clone()
        };
    if let Some(waveform_signals) = jg_signals.get(verilator_signal) {
        // Exact match
        let tmp_waveform_signal: Arc<str> = waveform_signals
            .strip_prefix("correctness.")
            .map_or_else(|| waveform_signals.clone(), |stripped| Arc::from(stripped));
        return Some(tmp_waveform_signal.clone());
        //signal_mapping.insert(invariant_signal.clone(), tmp_waveform_signal.clone());
    } else if let Some(waveform_signal) = jg_signals.get(&verilator_search_signal_name) {
        // Exact match
        let tmp_waveform_signal: Arc<str> = waveform_signal
            .strip_prefix("correctness.")
            .map_or_else(|| waveform_signal.clone(), |stripped| Arc::from(stripped));
        return Some(tmp_waveform_signal.clone());
        //signal_mapping.insert(invariant_signal.clone(), tmp_waveform_signal.clone());
    } else if jg_signals_transformed.is_some() {
        let is_matched_with_jg = jg_signals_transformed.unwrap().contains(&verilator_search_signal_name);
        if is_matched_with_jg {
            // let tmp_waveform_signal: Arc<str> = verilator_search_signal_name
            //         .strip_prefix("correctness.")
            //         .map_or_else(|| verilator_search_signal_name.clone(), |stripped| Arc::from(stripped));
            //     return Some(tmp_waveform_signal.clone());

            if let Some(waveform_signal) = jg_signals.par_iter().find_any(|&ws| {
               match_signal_names(&verilator_search_signal_name, ws)
                // Try matching with a prefixed backslash
                // match_signal_names(&verilator_search_signal_name, ws)
            }) {
                // If the signal matches after transformation, return it
                let tmp_waveform_signal: Arc<str> = waveform_signal
                    .strip_prefix("correctness.")
                    .map_or_else(|| waveform_signal.clone(), |stripped| Arc::from(stripped));
                return Some(tmp_waveform_signal.clone());
            } else {
                //println!("Line 231 Signal {} not found in JG signals", verilator_search_signal_name);
                return None;
            }
        } else {
            //println!("Line 234 Signal {} not found in JG signals", verilator_search_signal_name);
            return None;
        }
    } else {
        return None;
    }
}

pub fn get_signal_mapping_between_jg_and_verilator<H>(
    jg_signals: &HashSet<Arc<str>>,
    verilator_name_to_code: &HashMap<ustr::Ustr, u64, H>,
    cond_map: Option<data_types::signal_remap_info::ConditionMap>,
) -> HashMap<u64, Arc<str>> 
where 
    H: std::hash::BuildHasher,
{
    println!("### Getting signal mapping between JG and Verilator signals ###");
    // Create a mapping from invariant signal names to waveform signal names
    let verilator_signal_idx_to_alias: HashMap<u64, Vec<ustr::Ustr>> = verilator_name_to_code
    .iter()
    .fold(HashMap::new(), |mut acc, (name, code)| {
        acc.entry(*code).or_insert_with(Vec::new).push(name.clone());
        acc
    });
    let mut signal_mapping: HashMap<u64, Arc<str>> = HashMap::new();
    let mut matched_names_to_codes : HashMap<ustr::Ustr, u64> = HashMap::new();
    //First: Direct 1-to-1 mapping verilator to jg signals
    for (verilator_signal_id, verilator_aliases) in verilator_signal_idx_to_alias.iter() {
        let mut found_match = false;
        for alias in verilator_aliases {
            let verilator_signal_str: Arc<str> = Arc::from(alias.as_str());
            let maybe_match_name = matches_any_jg_signal(&verilator_signal_str, jg_signals, None);
            if let Some(matched_signal) = maybe_match_name {
                // If a match was found, insert it into the mapping
                signal_mapping.insert(*verilator_signal_id, matched_signal);
                found_match = true;
            } else {
                // println!("Could not find matching signal for {} in JG signals, skipping", verilator_signal);
            }
        }
        if found_match {
            for alias in verilator_aliases {
                matched_names_to_codes.insert(alias.clone(), *verilator_signal_id);
            }
        }
    }
    let jg_signal_mapping = signal_mapping.clone();
    // Now, handle the condition map entries. Map verilator signals to their expressions if not already mapped.
    println!("Handling condition map entries... cond_map length: {}", cond_map.as_ref().map_or(0, |cm| cm.len()));
    if let Some(cond_map) = &cond_map {
        // let signals_mapped_so_far: HashSet<Arc<str>> = signal_mapping.keys().cloned().collect();
        let signals_mapped_so_far: HashSet<ustr::Ustr> = matched_names_to_codes.keys().cloned().collect();
        for (verilator_signal, expr) in cond_map.iter() {
            let verilator_signal_code = match verilator_name_to_code.get(&ustr::Ustr::from(verilator_signal.as_str())) {
                Some(code) => *code,
                None => {
                    // println!("Skipping condition map entry for {} as it has no code index", verilator_signal);
                    continue; // Skip if we don't have a code index
                }
            };
            if expr.expression_okay_for_signal_list(&signals_mapped_so_far) == false {
                // println!("Skipping condition map entry for {} as it contains unmapped signals", verilator_signal);
                continue;
            }
            let expr_jg_str = Arc::from(expr.to_verilog(Some(&matched_names_to_codes), Some(&jg_signal_mapping)));
            signal_mapping.insert(verilator_signal_code, expr_jg_str);
            // if matched_names_to_codes.contains_key(&ustr::Ustr::from(verilator_signal.as_str())) {
            //     //Already mapped, can skip
            //     continue; // Skip if we don't have a code index
            // }
        }
    }

        // // } else {
        //     // println!("Could not find matching signal for {} in JG signals", verilator_signal);
        //     if let Some(cond_map) = &cond_map {
        //         if cond_map.contains_key(&verilator_signal.to_string()) {
        //             let expr_as_jg_str = Arc::from(format!("({})", cond_map.get(&verilator_signal.to_string()).unwrap().to_verilog(Some(&signal_mapping))));
        //             signal_mapping.insert(verilator_signal.clone(), expr_as_jg_str);
        //             continue;
        //         }
        //     }
        //     //     // panic!(
            //     "Could not find matching signal for {} in JG signals",
            //     verilator_signal
            // );
            // println!("JG signals:");
            // for signal in jg_signals {
            //     println!("{}", signal);
            // }
    println!("### Getting signal mapping between JG and Verilator signals done ###");
    // if signal_mapping.contains_key("TOP.correctness.sodor_core.core.d.regfile_ext.Added__Vcond_if_sodor_verilog_Vincent_regfile_32x32_sv_20_0") {
    //     println!("Final mapping contains TOP.correctness.sodor_core.core.d.regfile_ext.Added__Vcond_if_sodor_verilog_Vincent_regfile_32x32_sv_20_0");
    //     if let Some(matching_signal) = signal_mapping.get("TOP.correctness.sodor_core.core.d.regfile_ext.Added__Vcond_if_sodor_verilog_Vincent_regfile_32x32_sv_20_0") {
    //         println!("Matching signal: {}", matching_signal);
    //     }   
    // } else {
    //     println!("Final mapping does not contain TOP.correctness.sodor_core.core.d.regfile_ext.Added__Vcond_if_sodor_verilog_Vincent_regfile_32x32_sv_20_0");
    // }
    signal_mapping
}



pub fn format_invariant_with_signal_mapping<H>(
    invariant: &predicates::SeparatorFormula,
    signal_mapping: Option<&data_types::signal_remap_info::SignalRemappingInfo>,
    name_to_code_mapping: &HashMap<ustr::Ustr, u64, H>,
) -> String 
where 
    H: std::hash::BuildHasher,
{
    // Call invariant_to_string with the mapping
    invariant_to_string(invariant, signal_mapping, name_to_code_mapping)
}

fn invariant_to_string<H>(
    invariant: &predicates::SeparatorFormula,
    signal_mapping: Option<&data_types::signal_remap_info::SignalRemappingInfo>,
    name_to_code_mapping: &HashMap<ustr::Ustr, u64, H>,
) -> String
where 
    H: std::hash::BuildHasher,
{
    // Implement the logic to convert the invariant to a string using the signal mapping
    // This is a placeholder implementation
    let final_mapping: HashMap<u64, Arc<str>> = match signal_mapping {
        Some(signal_mapping) => {
            // let mut all_relevant_verilog_signals: Vec<Arc<str>> = invariant
            //     .get_signal_names()
            //     .iter()
            //     .cloned()
            //     .collect();
            // all_relevant_verilog_signals.extend(
            //     signal_mapping.cond_map.as_ref().map_or_else(|| vec![], |cm| {
            //         cm.values()
            //             .flat_map(|expr| expr.get_signal_names())
            //             .map(Arc::from)
            //             .collect::<HashSet<Arc<str>>>()
            //             .into_iter()
            //             .collect::<Vec<Arc<str>>>();
            //     }),
            // );
            // let all_relevant_verilog_signals = name_to_code_mapping
            //     .keys()
            //     .cloned()
            //     .collect::<HashSet<ustr::Ustr>>()
            //     .into_iter()
            //     .collect::<Vec<ustr::Ustr>>();
            // println!("All relevant verilog signals: {:?}", all_relevant_verilog_signals);

            // if !(name_to_code_mapping.contains_key(&ustr::Ustr::from("TOP.correctness.sodor_core.core.instruction_history_buffer.Added__Vcond_if_sodor_verilog_Vincent_CircularBufferNoReadOut_sv_122_0"))){
            //     println!("Name to code mapping does not contain TOP.correctness.sodor_core.core.instruction_history_buffer.Added__Vcond_if_sodor_verilog_Vincent_CircularBufferNoReadOut_sv_122_0");
            //     for (code, name) in verilator_signals_code_to_alias.iter() {
            //         if name.iter().any(|n| n.as_str() == "TOP.correctness.sodor_core.core.instruction_history_buffer.Added__Vcond_if_sodor_verilog_Vincent_CircularBufferNoReadOut_sv_122_0") {
            //             println!("Found it under code {}: {:?}", code, name);
            //         }
            //     }
            // } else {
            //     println!("Name to code mapping contains TOP.correctness.sodor_core.core.instruction_history_buffer.Added__Vcond_if_sodor_verilog_Vincent_CircularBufferNoReadOut_sv_122_0");
            //     for (code, name) in verilator_signals_code_to_alias.iter() {
            //         if name.iter().any(|n| n.as_str() == "TOP.correctness.sodor_core.core.instruction_history_buffer.Added__Vcond_if_sodor_verilog_Vincent_CircularBufferNoReadOut_sv_122_0") {
            //             println!("Found it also under code {}: {:?}", code, name);
            //         }
            //     }
            // }
            let non_cond_map_signals = match signal_mapping.jg_signals.as_ref() {
                Some(signals) => signals.iter().map(|s| Arc::from(s.as_str())).collect(),
                None => {
                    let name_to_code_signals: HashSet<Arc<str>> = name_to_code_mapping
                    .keys()
                    .map(|s| Arc::from(s.as_str()))
                    .collect();
                    name_to_code_signals
                }
                //     let cond_map_signals: HashSet<Arc<str>> = signal_mapping
                //         .cond_map
                //         .as_ref()
                //         .map_or_else(|| HashSet::new(), |cm| {
                //             cm.values()
                //                 .flat_map(|expr| expr.get_signal_names())
                //                 .map(Arc::from)
                //                 .collect::<HashSet<Arc<str>>>()
                //         });
                //     name_to_code_signals
                //         .difference(&cond_map_signals)
                //         .cloned()
                //         .collect()
                // },
            };
            io::stdout().flush().expect("Failed to flush stdout");
            get_signal_mapping_between_jg_and_verilator(
                &non_cond_map_signals,
                name_to_code_mapping,
                signal_mapping.cond_map.clone(),
            )
        },
        None => {
            invariant
                .get_relevant_signal_idx()
                .iter()
                .filter_map(|idx| {
                    if let Some(name) = name_to_code_mapping.iter().find_map(|(name, code)| if code == idx { Some(name) } else { None }) {
                        Some((*idx, Arc::from(name.as_str())))
                    } else {
                        panic!("Signal index {} not found in name_to_code_mapping", idx);
                    }
                })
                .collect()
        }
    };
    // if final_mapping.contains_key("TOP.correctness.sodor_core.core.d.regfile_ext.Added__Vcond_if_sodor_verilog_Vincent_regfile_32x32_sv_20_0") {
    //     println!("Final mapping before formatting contains TOP.correctness.sodor_core.core.d.regfile_ext.Added__Vcond_if_sodor_verilog_Vincent_regfile_32x32_sv_20_0");
    //     if let Some(matching_signal) = final_mapping.get("TOP.correctness.sodor_core.core.d.regfile_ext.Added__Vcond_if_sodor_verilog_Vincent_regfile_32x32_sv_20_0") {
    //         println!("Matching signal: {}", matching_signal);
    //     }   
    // } else {
    //     println!("Final mapping before formatting does not contain TOP.correctness.sodor_core.core.d.regfile_ext.Added__Vcond_if_sodor_verilog_Vincent_regfile_32x32_sv_20_0");
    // }
    invariant.to_jg_string_with_signal_mapping(&final_mapping)
}
