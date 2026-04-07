use crate::predicates;

// pub static BEX_MULTIPLIER: AtomicUsize = AtomicUsize::new(35);
// pub static PREDICATE_BASE_COST: AtomicUsize = AtomicUsize::new(10);

// pub fn get_predicate_base_cost() -> usize {
//     PREDICATE_BASE_COST.load(atomic::Ordering::SeqCst)
// }

// pub fn set_predicate_base_cost(new_cost: usize) {
//     PREDICATE_BASE_COST.store(new_cost, atomic::Ordering::SeqCst);
// }

// pub fn set_bex_multiplier(new_multiplier: usize) {
//     BEX_MULTIPLIER.store(new_multiplier, atomic::Ordering::SeqCst);
// }

// pub fn get_bex_multiplier() -> usize {
//     BEX_MULTIPLIER.load(atomic::Ordering::SeqCst)
// }

pub fn get_predicate_cost(total_num_cex: usize, total_bex_weight: f64, predicate_base_cost: usize) -> f64 {
    //return 0.0;
    let base_cost = predicate_base_cost;
    let base_cost_f64 = base_cost as f64;
    return (base_cost_f64 * total_num_cex as f64) / 1000.0;//(base_cost_f64 * total_num_cex as f64 + base_cost_f64 * total_bex_weight) / 1000.0;
    // match &predicate.base_formula {
    //     predicates::BaseFormula::SignalToConst(inner_formula) => {
    //         //let is_control_signal = inner_formula.signal_length <= 1 || inner_formula.signal_name.contains("opcode") || inner_formula.signal_name.contains("funct") || inner_formula.signal_name.contains("addr") || inner_formula.signal_name.contains("immediate") || inner_formula.signal_name.contains("imm_i");
    //         //let is_control_or_address_signal = inner_formula.signal_type == crate::data_types::SignalType::Control || inner_formula.signal_type == crate::data_types::SignalType::Address;
    //         // let is_control = inner_formula.signal_type == crate::data_types::SignalType::Control;
    //         // let is_address = inner_formula.signal_type == crate::data_types::SignalType::Address;
    //         let mut multiplier = if inner_formula.signal_types.contains(&crate::data_types::general_data_types::SignalType::Control) {
    //                 0.0
    //             } else if inner_formula.signal_types.contains(&crate::data_types::general_data_types::SignalType::Address) {
    //                 base_cost_f64*4.0
    //             } else if inner_formula.signal_types.contains(&crate::data_types::general_data_types::SignalType::Register) {
    //                 match inner_formula.value {
    //                     0 => 0.0,
    //                     _ => base_cost_f64*50.0,
    //                 }
    //             } else if inner_formula.signal_types.contains(&crate::data_types::general_data_types::SignalType::RegisterFileAddress) {
    //                 base_cost_f64*0.5
    //             } else {
    //                 base_cost_f64*50.0
    //             };
                
    //         //0.01+0.05+0.01+0.05 = 0.12
    //         //whereas
    //         //0.031 = 0.031
    //         //So it will be cheaper to use one signal equal than
    //         //two signal to const
    //         match inner_formula.operator {
    //             predicates::Operator::Equal => {
    //                 // For equality predicates, we assume a cost of 1.0
    //                 multiplier += base_cost_f64;
    //             },
    //             predicates::Operator::NotEqual => {
    //                 // For inequality predicates, we assume a cost of 2.0
    //                 multiplier += base_cost_f64*2.0;
    //             },
    //             predicates::Operator::GreaterEqual => {
    //                 // For greater than predicates, we assume a cost of 2.0
    //                 multiplier += base_cost_f64*2.0;
    //             },
    //             predicates::Operator::SmallerEqual => {
    //                 // For less than predicates, we assume a cost of 2.0
    //                 multiplier += base_cost_f64*2.0;
    //             },
    //         }
    //         multiplier * total_num_cex as f64
    //     }, 
    //     predicates::BaseFormula::TwoSignalEqual(_) => {
    //         total_num_cex as f64 * base_cost_f64*2.1
    //     },
    // }
}

pub fn get_invariant_cost(total_num_cex: usize, total_bex_weight: f64, invariant: &predicates::Invariant, predicate_base_cost: usize) -> f64 {
    let mut cost = 0.0;
    //return 0.0;
    for _ in invariant.predicate_set.predicates.iter() {
        cost += get_predicate_cost(total_num_cex, total_bex_weight, predicate_base_cost);
    }
    cost
}

pub fn get_separator_formula_cost(total_num_cex: usize, total_bex_weight: f64, formula: &predicates::SeparatorFormula, predicate_base_cost: usize) -> f64 {
    match formula {
        predicates::SeparatorFormula::Invariant(ref invariant) => get_invariant_cost(total_num_cex, total_bex_weight, invariant, predicate_base_cost),
        predicates::SeparatorFormula::InvariantDisjunction(ref invariant_disjunction) => {
            let mut cost = 0.0;
            for (idx,invariant) in invariant_disjunction.disjunctions.iter().enumerate() {
                if idx == 0 {
                    cost += get_invariant_cost(total_num_cex, total_bex_weight, invariant, predicate_base_cost);
                } else {
                    cost = cost.max(get_invariant_cost(total_num_cex, total_bex_weight, invariant, predicate_base_cost));
                }
            }
            cost
        },
    }
}

pub fn get_bex_weight_in_ilp(total_num_cex: usize, total_bex_weight: f64, bex_multiplier: usize) -> f64 {
    // return 1000.0;
    let ratio = if total_bex_weight == 0.0 {
        0.0
    } else {
        (((1.0+bex_multiplier as f64 / 100.0)) * (total_num_cex as f64 + total_bex_weight as f64)) / total_bex_weight as f64
    };
    ratio
    // let ratio = if total_bex_weight == 0.0 {
    //     0.0
    // } else {
    //     ((bex_multiplier as f64  / 100.0 as f64) * total_num_cex as f64) / total_bex_weight as f64
    // };
    // ratio
    // let weight = total_num_cex as f64 / 4000.0;
    //let weight = total_num_cex as f64 / (total_num_cex *2) as f64;
    // let total_num = total_num_cex + total_num_bex;
    // let bex_multiplier = get_bex_multiplier(); //7 works well
    // let each_bex_weight = (bex_multiplier*total_num_cex) as f64 / total_num_bex as f64;
    // each_bex_weight
    // let each_bex_weight = (bex_multiplier as f64/1000.0);
    // let each_bex_weight = each_bex_weight*total_num_cex as f64;
    // each_bex_weight
    /*
    if total_num > 0 {
        return (total_num_bex) as f64 * 0.001;
    }
    else {
        return 1.0;
    }
     */
    //return weight;
}

pub fn get_cex_weight_in_ilp(_total_num_cex: usize, _total_num_bex_weight: f64) -> f64 {
    1.0
}
