use grb;
use grb::expr;
use crate::teacher::MaybeSample;
use crate::waveform;
use crate::predicates;
use crate::teacher;
use crate::solver;
use std::collections::HashMap;
use std::hash::Hash;
use uuid::Uuid;
use grb::parameter;

/*

pub struct ILPSolver {
    teacher: teacher::Teacher,
}

fn construct_if_and_only_if_var(
    model: &mut grb::Model,
    lhs: grb::Var,
    rhs: i64,
) -> grb::Var {
    //https://math.stackexchange.com/questions/4029915/formulating-an-if-and-only-if-statement-with-linear-programming
    let x1 = grb::add_binvar!(model).unwrap();
    let x2 = grb::add_binvar!(model).unwrap();
    let x3 = grb::add_binvar!(model).unwrap();
    model.add_constr("sum_to_one", grb::c!(x1 + x2 + x3 == 1)).unwrap();
    let big_m: u64 = 1u64 << 32;
    //Add constraint (1-M)*x1+x3 <= lhs-rhs <= -x1 + (M-1)*x3
    model.add_constr("lhs_rhs_upper", grb::c!((1.0 - big_m as f64) * x1 + x3 <= lhs - rhs)).unwrap();
    model.add_constr("lhs_rhs_lower", grb::c!(-x1 + (big_m as f64 - 1.0) * x3 >= lhs - rhs)).unwrap();
    x2 //x2 = 1 <=> lhs == rhs
}

fn construct_grb_formula_predicate_and_sample(
    predicate: &predicates::BasePredicate,
    predicate_hashmap: &mut HashMap<String, grb::Var>,
    sample: &teacher::Sample,
    model: &mut grb::Model,
) -> grb::Var {
    let constants: Vec<String> = predicate.get_constant_names();
    for constant_name in constants.iter() {
        predicate_hashmap.entry(constant_name.clone()).or_insert_with(|| {
            let signal_length = predicate.signal_length;
            let max_value = (1 << signal_length); //it's actually max value +1
            let var = grb::add_var!(model, grb::VarType::Integer, bounds: 0..max_value);
            let var = var.unwrap();
            var
        });
    }

    let value: i64 = sample.get_signal_value(&predicate.get_signal_name());
    //let this_const = model.add_const(value).unwrap();
    let this_cycle_formula = match predicate.operator {
        predicates::Operator::Equal => {
            //https://math.stackexchange.com/questions/4029915/formulating-an-if-and-only-if-statement-with-linear-programming  
            let predicate_var = predicate_hashmap.get(&predicate.get_constant_names()[0]).unwrap();
            //predicate_var.clone() - this_const
            //let predicate_var = predicate_hashmap.get(&predicate.get_constant_names()[0]).unwrap();
            //let indicator_var = grb::add_binvar!(model, name: format!("equality_{}_{}_{}", sample.from_path, sample.and_cycle,predicate.get_signal_name()).as_str()).unwrap();
            //let constraint_name = format!("constraint_equality_{}_{}_{}", sample.from_path, sample.and_cycle,predicate.get_signal_name());
            //println!("Adding the constraint that {} (var: {:?}) = {} for {}_{}", predicate.get_constant_names()[0], *predicate_var, value, sample.from_path, sample.and_cycle);
            //model.add_genconstr_indicator(constraint_name.as_str(), indicator_var,true, grb::c!(predicate_var == value)).unwrap();
            //if sample.from_path == "/home/viniul/formal/cex-generator/output/benign_examples/waveforms/tmp0dnd1aei.bin.vcd" {
            //    println!("### Vincent Adding constraint {:?}, value {:?}, cycle {:?}, predicate {:?}", grb::c!(predicate_var == value), value, sample.and_cycle, predicate);
            //}
            //model.add_constr(format!("{} = {}", indicator_var.unwrap().get_name().unwrap(), value), grb::c!());
            //model.add_var(None, grb::VarType::Binary, 0.0, 1.0, None).unwrap();
            //model.add_constr(&indicator_var * value - predicate_var, "=", 0.0).unwrap();
            //indicator_var
            let indicator_var = construct_if_and_only_if_var(model, *predicate_var, value);
            indicator_var
        }
        predicates::Operator::NotEqual => {
            let predicate_var = predicate_hashmap.get(&predicate.get_constant_names()[0]).unwrap();
            let indicator_var = construct_if_and_only_if_var(model, *predicate_var, value);
            let inverse_indicator_var = grb::add_binvar!(model).unwrap();
            model.add_constr("inverse_indicator", grb::c!(inverse_indicator_var + indicator_var == 1.0)).unwrap();
            //let indicator_var_a = grb::add_var!(model, grb::VarType::Binary, bounds: 0..1).unwrap();
            //let indicator_var_b = grb::add_var!(model, grb::VarType::Binary, bounds: 0..1).unwrap();
            //let indicator_var = grb::add_binvar!(model).unwrap();
            //let constraint_name = format!("{} != {}", predicate.get_constant_names()[0], value);
            //model.add_genconstr_indicator(format!("inequailtiy_ind_{}",Uuid::new_v4()).as_str(), indicator_var_a, true, grb::c!(predicate_var >= value as f64 + 0.03f64)).unwrap();
            //model.add_genconstr_indicator(format!("inequailtiy_ind_{}",Uuid::new_v4()).as_str(), indicator_var_a, true, grb::c!(predicate_var <= value as f64 - 0.03f64)).unwrap();
            //model.add_genconstr_and(&constraint_name, indicator_var, vec![indicator_var_a, indicator_var_b]).unwrap();
            //model.add_genconstr_indicator(format!("inequailtiy_ind_{}",Uuid::new_v4()).as_str(), indicator_var,true, grb::c!(predicate_var == value)).unwrap();
            //let big_m = 1e6; // A large constant value for the big-M method
            //let inverse_sample_indicator = grb::add_binvar!(model).unwrap();
            // Add a constraint to enforce indicator_var2 = 1 - indicator_var1
            //model.add_constr(
            //    &constraint_name,
            //    grb::c!((inverse_sample_indicator + indicator_var) == 1.0),
            //).unwrap();
            // Add constraints for the big-M method
            //model.add_constr(&format!("{}_bigM1", constraint_name), grb::c!(*predicate_var - value <= big_m * (1 - indicator_var))).unwrap();
            //model.add_constr(&format!("{}_bigM2", constraint_name), grb::c!(*predicate_var - value >= (-1.0)*big_m * (1-indicator_var))).unwrap();
            // The above contraints ensure that if indicator_var = 1, then predicate_var == value
            // If indicator_var = 0, then predicate_var - value can be any value smaller than big_m
            // However, if indicator_var = 0, then the two values need to be equal
            inverse_indicator_var
        }
        predicates::Operator::InBetween => {   
            let indicator_var = grb::add_var!(model, grb::VarType::Binary, bounds: 0..1).unwrap();
            // Add binary variables for intermediate conditions
            let indicator_a = grb::add_var!(model, grb::VarType::Binary, bounds: 0..1).unwrap();
            let indicator_c = grb::add_var!(model, grb::VarType::Binary, bounds: 0..1).unwrap();
            let lower_predicate_var = predicate_hashmap.get(&predicate.get_constant_names()[0]).unwrap();
            let upper_predicate_var = predicate_hashmap.get(&predicate.get_constant_names()[1]).unwrap();
            let constraint_name1 = format!("{} <= {}", predicate.get_signal_name(), value);
            let constraint_name2 = format!("{} >= {}", predicate.get_signal_name(), value);
            // Add indicator constraints
            // indicator_a = 1 if a <= INTEGER_CONST
            model.add_genconstr_indicator(
                constraint_name1.as_str(), 
                indicator_a,
                true,
                 grb::c!(value >= lower_predicate_var)).unwrap();
            model.add_genconstr_indicator(
                constraint_name2.as_str(), 
                indicator_c,
                true,
                grb::c!(value <= upper_predicate_var)).unwrap();
            model.add_genconstr_and(format!("and_{}", constraint_name1).as_str(), indicator_var, [indicator_a, indicator_c]).unwrap();
            indicator_var

        }
    };

    this_cycle_formula
}

fn construct_grb_formula_for_sample_and_invariant(
    sample: &teacher::Sample,
    invariant: &predicates::Invariant,
    predicate_hashmap: &mut HashMap<String, grb::Var>,
    model: &mut grb::Model,
) -> grb::Var {
    let mut this_formula = Vec::new();
    for predicate in invariant.predicate_set.predicates.iter() {
        let formula = construct_grb_formula_predicate_and_sample(predicate, predicate_hashmap, sample, model);
        this_formula.push(formula);
    }
    //println!("Operand vars {:?}  predicates {:?}", this_formula, invariant.predicate_set.predicates);
    let this_sample_indicator = grb::add_binvar!(model).unwrap();
    let constraint_name = format!("this_sample_indicator_{}",Uuid::new_v4());
    model.add_genconstr_and(constraint_name.as_str(), this_sample_indicator, this_formula.into_iter()).unwrap();
    model.update().unwrap();
    //let x = grb::add_binvar!(model).unwrap();
    //model.add_constr("vincent_test", grb::c!(x == 1.0)).unwrap();
    //model.update().unwrap();
    //println!("This sample indicator {:?}", this_sample_indicator);
    //println!("Model constraints {:?}", model.get_genconstrs());
    //println!("constraint {:?}", model.get_con(constraint_name.as_str()).unwrap());
    this_sample_indicator
}

pub fn construct_grb_formula_for_bex_sample_and_invariant(
    sample: &teacher::Sample,
    invariant: &predicates::Invariant,
    predicate_hashmap: &mut HashMap<String, grb::Var>,
    model: &mut grb::Model,
) -> (grb::Var, grb::Var) {
    //println!("Constuing formula for bex sample {:?} hashmap {:?}", sample.from_path, predicate_hashmap);
    let this_sample_indicator = construct_grb_formula_for_sample_and_invariant(sample, invariant, predicate_hashmap, model);
    let inverse_sample_indicator = grb::add_binvar!(model, name: format!("inverse_indicator_{}_{}",sample.from_path, sample.and_cycle).as_str()).unwrap();
    // Add a constraint to enforce indicator_var2 = 1 - indicator_var1
    model.add_constr(
        format!("inverse_indicator_constraint_{}_{}",sample.from_path, sample.and_cycle).as_str(),
        grb::c!((inverse_sample_indicator + this_sample_indicator) == 1.0),
    ).unwrap();
    (this_sample_indicator, inverse_sample_indicator)
}

fn construct_grb_formula_for_maybesample_and_invariant(
    maybe_sample: &teacher::MaybeSample,
    invariant: &predicates::Invariant,
    model: &mut grb::Model,
    predicate_hashmap: &mut HashMap<String, grb::Var>,
) -> grb::Var {
    let this_maybesample_indicator = grb::add_binvar!(model).unwrap();
    let mut this_formula = Vec::new();
    for sample in maybe_sample.samples.iter() {
        let this_cycle_formula = construct_grb_formula_for_sample_and_invariant(sample, invariant, predicate_hashmap, model);
        this_formula.push(this_cycle_formula);
    }
    model.add_genconstr_or(format!("or_{}", maybe_sample.from_path).as_str(), this_maybesample_indicator, this_formula.into_iter()).unwrap();
    this_maybesample_indicator
    
}

pub fn check_for_maybe_sample(maybe_sample: &teacher::MaybeSample, invariant: &predicates::Invariant, values_for_predicates: &HashMap<String, i64>) -> Option<Vec<u64>> {
    let mut model = grb::Model::new("model1").unwrap();
    model.set_param(grb::parameter::IntParam::OutputFlag, 0).unwrap();
    let mut predicate_hashmap: HashMap<String, grb::Var> = HashMap::new();

    for predicate in invariant.predicate_set.predicates.iter() {
        let constants: Vec<String> = predicate.get_constant_names();
        for constant_name in constants.iter() {
            if let Some(&val) = values_for_predicates.get(constant_name) {
                let var = grb::add_var!(model, grb::VarType::Integer, bounds: val..val).unwrap();
                predicate_hashmap.insert(constant_name.to_owned(), var);
            }
        }
    }
    let formula = construct_grb_formula_for_maybesample_and_invariant(&maybe_sample, &invariant, &mut model, &mut predicate_hashmap);
    model.add_constr("waveform_constraint",grb::c!(formula == 1.0)).unwrap();
    //model.add_constr(&formula, "=", 0.0).unwrap();
    //model.optimize().unwrap();
    model.optimize().unwrap();

    let status = model.status().unwrap();
    if status == grb::Status::Optimal {
        let mut fulfilled_cycles = Vec::new();
        for sample in maybe_sample.samples.iter() {
            let this_cycle_formula = construct_grb_formula_for_sample_and_invariant(sample, invariant, &mut predicate_hashmap, &mut model);
            model.update().unwrap();
            if let Some(inner) = model.get_constr_by_name("waveform_constraint").unwrap() {
                model.remove(inner).unwrap(); 
            }
            model.update().unwrap();
            model.add_constr("waveform_constraint", grb::c!(this_cycle_formula == 1.0)).unwrap();
            model.update().unwrap();
            model.optimize().unwrap();
            let status_this_cycle = model.status().unwrap();
            if status_this_cycle == grb::Status::Optimal {
                fulfilled_cycles.push(sample.and_cycle);
            }
        }
        return Some(fulfilled_cycles);
    }
    None
}

pub fn check_for_waveform(
    waveform: &waveform::WaveForm,
    invariant: &predicates::Invariant,
    values_for_predicates: &HashMap<String, i64>,
) -> Option<Vec<u64>> {
    let mut samples: Vec<teacher::Sample> = Vec::new();
    for cycle in 0..waveform.num_cycles {
        let mut sample = HashMap::new();
        for signal in invariant.get_relevant_signals() {
            let value = waveform.get_signal_value_at_cycle(signal, cycle).unwrap();
            sample.insert(signal.to_string(), value);
        }
        samples.push(teacher::Sample {
            sample,
            from_path: waveform.path.clone(),
            and_cycle: cycle,
        });
    }
    let maybe_sample = teacher::MaybeSample {
        samples,
        from_path: waveform.path.clone(),
    };
    check_for_maybe_sample(&maybe_sample, invariant, values_for_predicates)   
}

impl ILPSolver {

    pub fn get_values_for_predicates_from_var_mapping(&self, predicate_hashmap: &HashMap<String, grb::Var>, model: &grb::Model) -> HashMap<String, i64> {
        let mut res_hashmap: HashMap<String, i64> = HashMap::new();
        for (key, var) in predicate_hashmap {
            let value = model.get_obj_attr(grb::attr::X, var).unwrap();
            res_hashmap.insert(key.to_string(), value as i64);
        }
        res_hashmap
    }


    pub fn get_score_from_model(&self, model: &grb::Model) -> predicates::ScoreResult {
        let status: grb::Status = model.status().unwrap();
        if status == grb::Status::Optimal {
            let score = model.get_attr(grb::attr::ObjVal).unwrap();
            return predicates::ScoreResult::Sat(score as i64);
        } else {
            return predicates::ScoreResult::Unsat;
        }
    }



}

impl solver::Solver for ILPSolver {
    
    fn new_from_teacher(teacher: teacher::Teacher) -> Self {
        ILPSolver   { teacher }
    }
    fn debug_invariant(&self, invariant: &predicates::Invariant, values_for_predicates_arg: Option<HashMap<String, i64>>) {
        let values_for_predicates = match values_for_predicates_arg {
            Some(val) => val,
            None => {if let Some(inner) = self.get_model_hashmap(invariant) {
                inner
            } else {
                println!("Model not satisfiable");
                return;
            }},
        };
        println!("Values for predicates {:?}", values_for_predicates);
        for maybe_sample in self.teacher.cex_samples.iter() {
            if let Some(fulfilled_cycles) = check_for_maybe_sample(maybe_sample, invariant, &values_for_predicates) {
                {} //println!("CEX sample {:?} fulfilled at cycles {:?}", maybe_sample.from_path, fulfilled_cycles);
            } else {
                println!("CEX sample {:?} not fulfilled", maybe_sample.from_path);
            }
        }
        for sample in self.teacher.bex_samples.iter() {
            let maybe_sample = MaybeSample{samples: vec![sample.clone()], from_path: sample.from_path.clone()};
            if let Some(fulfilled_cycles) = check_for_maybe_sample(&maybe_sample, invariant, &values_for_predicates) {
                println!("BEX sample {:?} is blocked because of cycles {:?}", sample.from_path, sample.and_cycle);
            } else {
                {} //println!("BEX sample {:?} not fulfilled", sample.from_path);
            }
        }

    }

    fn score_invariant(&self, invariant: &predicates::Invariant, abort_on_threshold: Option<i64>) -> predicates::PriorityScores {
        let mut score: predicates::PriorityScores = Default::default();
        let mut model = grb::Model::new("Invariatn score").unwrap();
        model.set_param(grb::parameter::IntParam::OutputFlag, 0).unwrap();
        let mut predicate_hashmap: HashMap<String, grb::Var> = HashMap::new();
        let mut objective = grb::expr::LinExpr::new();
        for maybe_sample in self.teacher.cex_samples.iter() {
            if maybe_sample.samples.len() == 0 {
                continue;
            }
            let formula = construct_grb_formula_for_maybesample_and_invariant(maybe_sample, &invariant, &mut model, &mut predicate_hashmap);
            objective.add_term(1.0, formula);
        }
        model.set_objective(objective, grb::ModelSense::Maximize).unwrap();
        model.update().unwrap();
        model.optimize().unwrap();
        //let status = model.get(attr::Status).unwrap();
        score.cex_only_score = self.get_score_from_model(&model);

        if let Some(threshold) = abort_on_threshold {
            if let predicates::ScoreResult::Sat(inner_score) = score.cex_only_score {
                if inner_score < threshold {
                    return score;
                }
            }
        }
        //println!("CEX only score {:?}", score.cex_only_score);
        //println!("Now scroing with constraints");

        //let mut model = Model::new(&env).unwrap();
        //for maybe_sample in self.teacher.cex_samples.iter() {
        //    let formula = construct_grb_formula_for_maybesample_and_invariant(maybe_sample, &invariant, &mut model, &mut predicate_hashmap);
        //    model.add_constr(&formula, "=", 0.0).unwrap();
        //}
        
        let mut bex_formulas: Vec<(grb::Var, grb::Var)> = Vec::new();
        for bex_sample in self.teacher.bex_samples.iter() {
            let (this_sample_indicator, formula) = construct_grb_formula_for_bex_sample_and_invariant(bex_sample, &invariant, &mut predicate_hashmap, &mut model);
            bex_formulas.push((this_sample_indicator,formula));
            model.add_constr(format!("{}_{}_{}",bex_sample.from_path,bex_sample.and_cycle, Uuid::new_v4()).as_str(),grb::c!(this_sample_indicator == 0.0)).unwrap();
        }
        model.update().unwrap();
        model.optimize().unwrap();
        model.update().unwrap();
        //println!("Cex with bex block score {:?}", self.get_score_from_model(&model));
        //let status = model.get(attr::Status).unwrap();
        score.cex_with_bex_block_score = self.get_score_from_model(&model);
        /*match score.cex_with_bex_block_score {
            predicates::ScoreResult::Sat(inner) => {
                
                    for (key, var) in &predicate_hashmap {
                        let value = model.get_obj_attr(grb::attr::X, var).unwrap();
                        println!("Cex with bex block score Variable {} (id: {:?}): {}", key, var, value);
                    }
                    for (this_sample_indicator, bex_indicator_variable) in bex_formulas {
                        continue;
                        let value = model.get_obj_attr(grb::attr::X, &bex_indicator_variable).unwrap();
                        let variable_name = model.get_obj_attr(grb::attr::VarName, &bex_indicator_variable).unwrap();
                        //Variable is of the format inverse_indicator_/home/viniul/formal/cex-generator/output/benign_examples/waveforms/tmp1oxuu2r6.bin.vcd_0
                        //Get variable with name equality_inverse_indicator_/home/viniul/formal/cex-generator/output/benign_examples/waveforms/tmp1oxuu2r6.bin.vcd_0_{invariant.predicates.predicates[0].get_signal_name()}
                        let equality_var_name = format!("equality_{}_{}", &variable_name["inverse_indicator_".len()..], invariant.predicate_set.predicates[0].get_signal_name());
                        let equality_constraint_name = format!("constraint_equality_{}_{}", &variable_name["inverse_indicator_".len()..], invariant.predicate_set.predicates[0].get_signal_name());
                        if let Some(equality_var) = model.get_var_by_name(&equality_var_name).unwrap() {
                            let equality_value = model.get_obj_attr(grb::attr::X, &equality_var).unwrap();
                            println!("Equality variable {}: {}", equality_var_name, equality_value);
                            //println!("Gen constraints {:?}",model.get_genconstrs());
                        }
                        println!("Value of bex_indicator_variable: {:?}, variable {:?}", value, variable_name);
                    }
                    let res_hashmap = self.get_values_for_predicates_from_var_mapping(&predicate_hashmap, &model);
                    println!("Values for predicates {:?}", res_hashmap)
                
            },
            _ => {},
        };*/
;        

        score
    }

    fn print_invariant_with_model(&self, invariant: &predicates::Invariant) {
        let hashmap = self.get_model_hashmap(invariant).unwrap();
        println!("Model for invariant: {:?}", invariant);
        for (key, value) in hashmap.iter() {
            println!("{}: {}", key, value);
        }
    }
    
    fn get_model_hashmap(
            &self,
            invariant: &predicates::Invariant,
        ) -> Option<HashMap<String, i64>> {
            let mut model = grb::Model::new("Invariatn score").unwrap();
            model.set_param(grb::parameter::IntParam::OutputFlag, 0).unwrap();
            let mut predicate_hashmap: HashMap<String, grb::Var> = HashMap::new();
            let mut res_hashmap: HashMap<String, i64> = HashMap::new();
            let mut objective = grb::expr::LinExpr::new();
            for maybe_sample in self.teacher.cex_samples.iter() {
                let formula = construct_grb_formula_for_maybesample_and_invariant(maybe_sample, &invariant, &mut model, &mut predicate_hashmap);
                objective.add_term(1.0, formula);
            }
            model.set_objective(objective, grb::ModelSense::Maximize).unwrap();
            model.update().unwrap();
            let mut bex_formulas: Vec<grb::Var> = Vec::new();
            for bex_sample in self.teacher.bex_samples.iter() {
                let (this_sample_indicator, formula) = construct_grb_formula_for_bex_sample_and_invariant(bex_sample, &invariant, &mut predicate_hashmap, &mut model);
                bex_formulas.push(formula);
                model.add_constr(format!("{}_{}",bex_sample.from_path,bex_sample.and_cycle).as_str(),grb::c!(formula == 1.0)).unwrap();
            }
    
            model.optimize().unwrap();
            let status = model.status().unwrap();
            if status != grb::Status::Optimal {
                return None;
            } else {
                println!("Model status: {:?} score {:?}", status, self.get_score_from_model(&model));
                let res_hashmap = self.get_values_for_predicates_from_var_mapping(&predicate_hashmap, &model);
                return Some(res_hashmap);
            }
    }



}

#[cfg(test)]
mod tests {
    use std::env;
    #[test]
    fn test_gurobi() -> Result<(), Box<dyn std::error::Error>>{
        // Retrieve the PATH environment variable
        match env::var("PATH") {
            Ok(path) => println!("PATH: {}", path),
            Err(e) => println!("Couldn't read PATH: {}", e),
        }
        //let rust_log_value = env::var("LD_LIBRARY_PATH").unwrap_or_default();
        //println!("{}", rust_log_value); // Or use for some logic
        //return Ok(());
        use grb::prelude::*;
        use crate::ilp_solver;
        let mut model = Model::new("model1")?;
        let x1 = add_binvar!(model)?;
        let x2 = add_binvar!(model)?;
        let x3 = add_binvar!(model)?;
        let y = add_binvar!(model)?;
        let z = add_binvar!(model)?;
        //let max_value = (1 << signal_length); //it's actually max value +1
        let var = grb::add_var!(model, grb::VarType::Integer, bounds: 0..10.0);
        let var: Var = var.unwrap();
        let indicator_var = crate::ilp_solver::construct_if_and_only_if_var(&mut model, var, 5);
        model.add_constr("test",c!(indicator_var == 0))?;
        model.add_genconstr_indicator("c1", z, true, c!(var == 5))?;
        //model.add_genconstr_and("c1", y, [x1, x2, x3])?;
        //model.add_constr("vincent1", grb::c!(x1 == 0))?;
        //model.add_constr("vincent2", grb::c!(x2 == 0))?;
        //model.add_constr("vincent3", grb::c!(x3 == 0))?;

        println!("Modle constraints {:?}", model.get_genconstrs());
        model.set_objective(1.0 * z, grb::ModelSense::Maximize)?;
        model.optimize()?;
        println!("Objective value: {}", model.get_attr(attr::ObjVal)?);
        println!("Value of y: {}", model.get_obj_attr(attr::X, &y)?);
        println!("Value of var: {}", model.get_obj_attr(attr::X, &var)?);
        return Ok(());


        // add decision variables with no bounds
        let x1 = add_ctsvar!(model, name: "x1", bounds: ..)?;
        let x2 = add_intvar!(model, name: "x2", bounds: ..)?;

        // add linear constraints
        let c0 = model.add_constr("c0", c!(x1 + 2*x2 >= -14))?;
        let c1 = model.add_constr("c1", c!(-4 * x1 - x2 <= -33))?;
        let c2 = model.add_constr("c2", c!(2* x1 <= 20 - x2))?;

        // model is lazily updated by default
        assert_eq!(model.get_obj_attr(attr::VarName, &x1).unwrap_err(), grb::Error::ModelObjectPending);
        assert_eq!(model.get_attr(attr::IsMIP)?, 0);

        // set the objective function, which updates the model objects (variables and constraints).
        // One could also call `model.update()`
        model.set_objective(8*x1 + x2, Minimize)?;
        assert_eq!(model.get_obj_attr(attr::VarName, &x1)?, "x1");
        assert_eq!(model.get_attr(attr::IsMIP)?, 1);

        // write model to the file.
        model.write("model.lp")?;

        // optimize the model
        model.optimize()?;
        assert_eq!(model.status()?, Status::Optimal);

        // Querying a model attribute
        assert_eq!(model.get_attr(attr::ObjVal)? , 59.0);

        // Querying a model object attributes
        assert_eq!(model.get_obj_attr(attr::Slack, &c0)?, -34.5);
        let x1_name = model.get_obj_attr(attr::VarName, &x1)?;

        // Querying an attribute for multiple model objects
        let val = model.get_obj_attr_batch(attr::X, vec![x1, x2])?;
        assert_eq!(val, [6.5, 7.0]);
        println!("Val {:?}", val);

        // Querying variables by name
        assert_eq!(model.get_var_by_name(&x1_name)?, Some(x1));
        Ok(())
    }
} */