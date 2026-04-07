use std::sync::Arc;
use regex::Regex;

use crate::predicates;
use crate::predicates::Operator;
use crate::teacher;


#[derive(Debug, Clone)]
pub enum RuleTrigger {
    PredicateIsContained(predicates::BasePredicate),
    SignalIsContained(String),
    SignalFulfillsRegex(String),
}

#[derive(Debug, Clone)]
pub enum RuleConsequence {
    DisallowOtherSignals(Vec<String>),
    OnlyAllowOtherSignals(Vec<String>),
}
#[derive(Debug, Clone)]
pub struct Rule {
    pub trigger_operation: RuleTrigger,
    pub consequence: RuleConsequence
}

pub enum PredicateBanRule {
    DisallowSignalOperationCombinationRegex(String, predicates::Operator, usize),
}

/*
pub fn should_consider_predicate(signal_name: &Arc<str>, operation: &predicates::Operator, teacher_arg: &teacher::Teacher) -> bool {
    if *operation == predicates::Operator::Greater || *operation == predicates::Operator::Smaller {
        let aliases = teacher_arg.get_signal_aliases(&signal_name);
        if !(aliases.iter().any(|alias| alias.contains("addr") && (alias.contains("csr") || alias.contains("mem")))) {
            if !(aliases.iter().any(|alias| alias.contains("commit_counter_sodor"))) {
                return false; //continue;
            }
        }
    }
    return true;
}
     */

pub fn init_predicate_rules() -> Vec<PredicateBanRule> {
    let rule_vec: Vec<PredicateBanRule> = vec![
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instr$"#.to_string(), Operator::NotEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instr$"#.to_string(), Operator::GreaterEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instr$"#.to_string(), Operator::SmallerEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instruction$"#.to_string(), Operator::NotEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instruction$"#.to_string(), Operator::GreaterEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instruction$"#.to_string(), Operator::SmallerEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instr_data$"#.to_string(), Operator::NotEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instr_data$"#.to_string(), Operator::GreaterEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*instr_data$"#.to_string(), Operator::SmallerEqual, 32),
        PredicateBanRule::DisallowSignalOperationCombinationRegex(r#".*data.*"#.to_string(), Operator::NotEqual, 8),
        // Add more rules as needed
    ];
    rule_vec
}

pub fn predicate_matches_ban_rules(predicate: &predicates::BasePredicate, rules: &Vec<PredicateBanRule>, teacher_arg: &teacher::Teacher) -> bool {
    for rule in rules.iter() {
        match rule {
            PredicateBanRule::DisallowSignalOperationCombinationRegex(regex, operation, signal_length) => {
                if predicate.get_operator() != *operation {
                    continue;
                }
                for signal in predicate.get_signal_names() {

                    let this_signal_length = teacher_arg.get_signal_length(&signal);
                    if this_signal_length != *signal_length {
                        continue;
                    }
                    let signal_alias = teacher_arg.get_signal_aliases(&signal);
                    for alias in signal_alias.iter() {
                        let signal = alias.as_ref();
                        let re = Regex::new(regex).unwrap();
                        if re.is_match(signal.as_ref()) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    return false;
}

pub fn should_consider_predicate(predicate: &predicates::BasePredicate, teacher_arg: &teacher::Teacher) -> bool {
    let rules = init_predicate_rules();
    for signal_name in predicate.get_signal_names() {
        let aliases = teacher_arg.get_signal_aliases(&signal_name);
        if aliases.iter().any(|alias| 
            alias.as_ref() == crate::constants::INCORRECTNESS_SIGNAL ||
            alias.as_ref() == crate::constants::COUNTER_SIGNAL ||
            alias.as_ref() == crate::constants::MISMATCH_CYCLE_REF_CORE_SIGNAL ||
            alias.as_ref() == crate::constants::MISMATCH_CYCLE_DUT_CORE_SIGNAL
        ) {
            return false;
        }
    }
    if predicate_matches_ban_rules(predicate, &rules, teacher_arg){
        // println!("Predicate matches ban rules, returning false for predicate: {}", predicate);
        return false;
    }
    return true;
    
    // if predicate.get_operator() == predicates::Operator::GreaterEqual || predicate.get_operator() == predicates::Operator::SmallerEqual {
    //     for signal_name in predicate.get_signal_names() {
    //         let aliases = teacher_arg.get_signal_aliases(&signal_name);
    //         if !(aliases.iter().any(|alias| (alias.contains("addr") && alias.contains("csr")) || alias.contains("mem") || alias.contains("imm_i"))) {
    //             if !(aliases.iter().any(|alias| alias.contains("commit_counter_sodor"))) {
    //                 // if signal_name.contains("imm_i") {
    //                 //     println!("Aliases for signal name {}: {:?}", signal_name, aliases);
    //                 // }
    //                 // println!("Returning false for signal: {}, aliases: {:?}", signal_name, aliases);
    //                 return false; //continue;
    //             }
    //         }
    //     }
    // }
    // return true; 
}


//https://devopedia.org/risc-v-instruction-sets
pub fn initialize_rules() -> Vec<Rule> {
    let rule_vec: Vec<Rule> = vec![
        Rule { //If opcode is set to CSR, then do not combine that with imm_u signal
            trigger_operation: RuleTrigger::PredicateIsContained(predicates::BasePredicate::new_from_value(Arc::from("TOP.correctness.opcode"), predicates::Operator::Equal, 7, 0x73, None)),
            consequence: RuleConsequence::DisallowOtherSignals(vec!["TOP.correctness.imm_s".to_string(),"TOP.correctness.funct7".to_string(),"TOP.correctness.rs2".to_string()])
        },
        Rule {
            trigger_operation: RuleTrigger::SignalIsContained("TOP.correctness.imm_u".to_string()),
            consequence: RuleConsequence::DisallowOtherSignals(vec!["TOP.correctness.imm_s".to_string(), "TOP.correctness.funct7".to_string(), "TOP.correctness.funct3".to_string()])
        },
        Rule {
            trigger_operation: RuleTrigger::SignalIsContained("TOP.correctness.imm_i".to_string()),
            consequence: RuleConsequence::DisallowOtherSignals(vec!["TOP.correctness.imm_s".to_string(), "TOP.correctness.imm_b".to_string(), "TOP.correctness.imm_j".to_string(), "TOP.correctness.imm_u".to_string()])
        },
        //Rule { //Only do >=, <= if the signal contains addr, address or imm
        //    trigger_operation:RuleTrigger::SignalFulfillsRegex(".*^(.(?!addr|address|imm))*$.*".to_string()),
        //    consequence: RuleConsequence::DisallowPredicateOperations(vec![predicates::Operator::Greater, predicates::Operator::Smaller,])
        //}
        // Add more rules as needed
    ];
    rule_vec
}

fn rule_is_fullfilled(invariant: &predicates::Invariant, rule: &Rule, main_teacher: &teacher::Teacher) -> bool {
    let get_alias_callback: Box<dyn Fn(&str) -> Vec<Arc<str>>> = Box::new(|signal_name| {
        main_teacher.get_signal_aliases(signal_name)
    });
    let mut trigger_fulfilled = false;
    for predicate in invariant.predicate_set.predicates.iter() {
        match rule.trigger_operation {
            RuleTrigger::PredicateIsContained(ref trigger_predicate) => {
                let mut trigger_predicate: predicates::BasePredicate = trigger_predicate.clone();
                if main_teacher.fill_in_indexes_for_predicate(&mut trigger_predicate).is_err() {
                    //Predicate symbol is not even in the teacher!
                    //println!("Predicate sgi is not even in the teacher!");
                    continue;
                }

                if &trigger_predicate == predicate {
                    trigger_fulfilled = true;
                    break;
                }
            },
            RuleTrigger::SignalIsContained(ref signal) => {
                //Get the idx for *signal, then get idx of predicate.get_signal_name()
                let signal: Arc<str> = signal.clone().into();
                let predicate_signals = predicate.get_signal_names();
                for predicate_signal in predicate_signals.iter() {
                    let signal_idx = main_teacher.get_maybe_signal_idx(&signal);
                    if signal_idx.is_none() {
                        //println!("Signal is not in the teacher!");
                        continue;
                    }
                    let signal_idx = signal_idx.unwrap();
                    let predicate_signal_idx = main_teacher.get_maybe_signal_idx(&predicate_signal);
                    if predicate_signal_idx.is_none() {
                        //println!("Predicate signal is not in the teacher!");
                        continue;
                    }
                    let predicate_signal_idx = predicate_signal_idx.unwrap();
                    if predicate_signal_idx == signal_idx {
                        trigger_fulfilled = true;
                        break;
                    }
                }

            },
            RuleTrigger::SignalFulfillsRegex(ref regex) => {
                for signal in predicate.get_signal_names() {
                    let signal_alias = main_teacher.get_signal_aliases(&signal);
                    for alias in signal_alias.iter() {
                        let signal = alias.as_ref();
                        let re = Regex::new(regex).unwrap();
                        if re.is_match(signal.as_ref()) {
                            trigger_fulfilled = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    //println!("Trigger fulfilled: {:?} for rule {:?}", trigger_fulfilled, rule);
    if !trigger_fulfilled {
        return true;
    }
    match rule.consequence {
        RuleConsequence::DisallowOtherSignals(ref signals) => {
            for predicate in invariant.predicate_set.predicates.iter() {
                for signal in predicate.get_signal_names() {
                    let aliases = get_alias_callback(&signal);
                    if aliases.iter().any(|alias| signals.iter().any(|signal| signal.as_str() == alias.as_ref())) {
                        return false;
                    }
                }
             }
            //return true;
        },
        RuleConsequence::OnlyAllowOtherSignals(ref signals) => {
            for predicate in invariant.predicate_set.predicates.iter() {
                let mut signal_found = false;
                for signal in predicate.get_signal_names() {
                    let aliases = get_alias_callback(&signal);
                    if aliases.iter().any(|alias| signals.iter().any(|signal| signal.as_str() == alias.as_ref())) {
                        signal_found = true;
                    }
                }
                if signal_found == false {
                    return false;
                } 
            }
        },
        /*RuleConsequence::DisallowPredicateOperations(ref operations) => {
            for predicate in invariant.predicate_set.predicates.iter() {
                if operations.iter().any(|operation| operation == &predicate.get_operator()) {
                    return false;
                }
            }
        }*/
    }
    return true;
}

pub fn architectural_invariant_check(invariant: &predicates::Invariant, main_teacher: &teacher::Teacher) -> bool {

    let rule_vec = initialize_rules();
    for rule in rule_vec.into_iter() {
        //println!("Checking rule: {:?}", rule);
        //Return false if rule is not fulfilled, otherwise do nothing
        if rule_is_fullfilled(invariant, &rule, main_teacher) == false {
            // println!("Rule is not fulfilled: {:?} for invariant {}", rule, invariant);
            return false;
        }
    }
    for predicate in invariant.predicate_set.predicates.iter() {
        if should_consider_predicate(&predicate, main_teacher) == false {
            println!("Predicate should not be considered: {} for invariant {}", predicate, invariant);
            return false;
        }
    }

    return true;
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_basepredicate_with_value_xyz_returns_true() {
        // Create a BasePredicate with a specific value
        let predicate = predicates::BasePredicate::new_from_value(
            Arc::from("TOP.correctness.opcode"),
            predicates::Operator::Equal,
            7,
            0x73,
        None);
        let imm_s_predicate = predicates::BasePredicate::new_from_value(
            Arc::from("TOP.correctness.imm_s"),
            predicates::Operator::Equal,
            12,
            1801,
            None
        );
        



        // Create an invariant containing the predicate
        let invariant = predicates::Invariant {
            predicate_set: predicates::PredicateSet {
                predicates: vec![predicate.clone(), imm_s_predicate.clone()],
            },
        };

        // Create a mock values_for_constants map
        let mut name_to_id_map = HashMap::new();
        name_to_id_map.insert(ustr::ustr("TOP.correctness.opcode"), 0x0);
        let mut id_counter = 0x1; // Start ID counter after 0x73
        for signal in &[
            "TOP.correctness.commit_counter_sodor",
            "TOP.correctness.funct3",
            "TOP.correctness.rs1",
            "TOP.correctness.rd",
            "TOP.correctness.imm_i",
            "TOP.correctness.imm_u",
            "TOP.correctness.imm_s",
            "TOP.correctness.imm_b",
            "TOP.correctness.imm_j",
        ] {
            name_to_id_map.insert(ustr::ustr(signal), id_counter);
            id_counter += 1;
        }
        let name_to_id = Arc::new(name_to_id_map);
        let mock_teacher = teacher::Teacher{
            name_to_id: name_to_id,
            cex_traces: Vec::new(),
            bex_traces: Vec::new(),
            bex_samples: Vec::new(),
            values_per_signal: Some(HashMap::new()),
            id_to_signal_info: Arc::new(HashMap::new()),
            states: HashMap::new(),
        };
        // Call the architectural_invariant_check function
        let result = architectural_invariant_check(
            &invariant,
            &mock_teacher,
        );

        // Assert that the result is true
        assert_eq!(result, false);
    }
}
