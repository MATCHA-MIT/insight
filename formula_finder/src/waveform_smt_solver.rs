use std::collections::HashMap;

use crate::waveform;
use crate::predicates;
use z3;
use z3::ast::Dynamic;
use z3::Model;
use z3::ast::Ast;
use z3::SatResult;

pub struct SMTSolver<'a> {
    cex_waveforms: &'a Vec<waveform::WaveForm>,
    bex_waveforms: &'a Vec<waveform::WaveForm>,
}

/*

    fn construct_z3_formula_for_waveform_and_cycle<'b>(&mut self, waveform: &'b waveform::WaveForm, cycle:u64, z3_ctx: &'ctx z3::Context) -> z3::ast::Bool<'ctx> {
        if self.predicate_const.is_none() {
            self.predicate_const = Some(z3::ast::BV::new_const(z3_ctx, self.signal_name.clone()+"_CONST", self.signal_length));
        }
        let value = waveform.get_signal_value_at_cycle(&self.signal_name, cycle).unwrap();
        let this_const = z3::ast::BV::from_i64(&z3_ctx, value, 32);
        let this_cycle_formula = if self.operator == Operator::Equal {
            this_const._eq(self.predicate_const.as_ref().unwrap())
        } else {
            z3::ast::Bool::not(&(this_const._eq(self.predicate_const.as_ref().unwrap())))
        };
        this_cycle_formula
    } */

/*
fn construct_z3_formula_for_waveform_and_cycle(&mut self, waveform: &waveform::WaveForm, cycle:u64) {
    let mut this_formula = Vec::new();
    for predicate in self.predicate_set.predicates.iter_mut() {
        let formula = predicate.construct_z3_formula_for_waveform_and_cycle(waveform, cycle, &self.z3_ctx);
        this_formula.push(formula);
    }
    let binding = this_formula.iter().map(|x| x).collect::<Vec<_>>();
    let formula = z3::ast::Bool::and(&self.z3_ctx, binding.as_slice());
}

fn construct_z3_formula_for_waveform(&mut self, waveform: &waveform::WaveForm, is_cex: bool) {
    let this_formula = Vec::new();
    for cycle in 0...waveform.num_cycles {
        let this_cycle_formula = self.construct_z3_formula_for_waveform_and_cycle(waveform, cycle);
        if !(is_cex) {
            this_cycle_formula = this_cycle_formula.not();
        }
        this_formula.push(this_cycle_formula);
    }
    let binding = this_formula.iter().map(|x| x).collect::<Vec<_>>();
    let slice = binding.as_slice();
    if is_cex {
        let formula = z3::ast::Bool::or(&ctx, slice);
    } else {
        let formula = z3::ast::Bool::and(&ctx, slice);
    }
}
*/

fn construct_z3_formula_predicate_waveform_and_cycle<'ctx>(predicate: &'ctx predicates::Predicate, predicate_hashmap: &mut HashMap<&'ctx str, z3::ast::BV<'ctx>>, waveform: &waveform::WaveForm, cycle:u64, z3_ctx: &'ctx z3::Context) -> z3::ast::Bool<'ctx> {
    let predicate_z3_variable = predicate_hashmap
        .get(predicate.signal_name.as_str())
        .cloned()
        .unwrap_or_else(|| {
            let new_const = z3::ast::BV::new_const(z3_ctx, predicate.signal_name.clone() + "_CONST", predicate.signal_length);
            predicate_hashmap.insert(predicate.signal_name.as_str(), new_const.clone());
            new_const
        });
    let value = waveform.get_signal_value_at_cycle(&predicate.signal_name, cycle).unwrap();
    //if waveform.path == "/home/viniul/formal/cex-generator/waveforms/mutated_sequence_420_6130f.bin.vcd" {
    //    println!("Signal {} at cycle {} has value {:x}", predicate.signal_name, cycle, value);
    //}
    let this_const = z3::ast::BV::from_i64(&z3_ctx, value, predicate.signal_length);
    let this_cycle_formula = if predicate.operator == predicates::Operator::Equal {
        predicate_z3_variable._eq(&this_const)
    } else {
        
        let res = z3::ast::Bool::not(&predicate_z3_variable._eq(&this_const));
        res
    };
    this_cycle_formula
}

fn construct_z3_formula_for_waveform_and_cycle<'ctx>(waveform: &waveform::WaveForm, invariant: &'ctx predicates::Invariant, predicate_hashmap: &mut HashMap<&'ctx str, z3::ast::BV<'ctx>>, cycle:u64, z3_ctx: &'ctx z3::Context) -> z3::ast::Bool<'ctx> {
    let mut this_formula = Vec::new();
    for predicate in invariant.predicate_set.predicates.iter() {
        let formula = construct_z3_formula_predicate_waveform_and_cycle(predicate, predicate_hashmap, waveform, cycle, z3_ctx);
        this_formula.push(formula);
    }
    let binding = this_formula.iter().map(|x| x).collect::<Vec<_>>();
    let formula = z3::ast::Bool::and(z3_ctx, binding.as_slice());
    formula
}

fn construct_z3_formula_for_waveform_and_invariant<'ctx>(waveform: &waveform::WaveForm, invariant: &'ctx predicates::Invariant, is_cex: bool, z3_ctx: &'ctx z3::Context, predicate_hashmap: &mut HashMap<&'ctx str,z3::ast::BV<'ctx>>) -> z3::ast::Bool<'ctx> {
    let mut this_formula = Vec::new();
    for cycle in 0..waveform.num_cycles {
        let mut this_cycle_formula = construct_z3_formula_for_waveform_and_cycle(waveform,invariant, predicate_hashmap, cycle, z3_ctx);
        if !(is_cex) {
            this_cycle_formula = this_cycle_formula.not();
        }
        this_formula.push(this_cycle_formula);
    }
    let binding = this_formula.iter().map(|x| x).collect::<Vec<_>>();
    let slice = binding.as_slice();
    let formula = if is_cex {
        z3::ast::Bool::or(z3_ctx, slice)
    } else {
       z3::ast::Bool::and(z3_ctx, slice)
    };
    //if !(is_cex) {
    //    println!("Formula {}", formula);
    //}
    formula
}



impl<'a> SMTSolver<'a> {
    pub fn new(cex_waveforms: &'a Vec< waveform::WaveForm>, bex_waveforms: &'a Vec<waveform::WaveForm>) -> Self {
        SMTSolver {
            cex_waveforms,
            bex_waveforms
        }
    }

    pub fn get_score_from_model(&self, res: SatResult, optimizer: &z3::Optimize, num_soft_constraints: i64) -> Option<i64> {
        if res == z3::SatResult::Sat {
            let mut score = 0;
            let objective = optimizer.get_objectives();
            let model = optimizer.get_model().unwrap();
            //println!("Model for scoring {:?}", model);
            let unfulfilled_cex = model.eval(&objective[0].as_real().unwrap().to_int(), true).unwrap().as_i64().unwrap();
            score = num_soft_constraints - unfulfilled_cex;
            return Some(score);
        } else {
            None
        }
    }

    pub fn debug_invariant(&self, invariant: &predicates::Invariant, values_for_predicates: Option<HashMap<String,i64>>) {
        let cfg = z3::Config::new();
        let ctx = z3::Context::new(&cfg);
        let solver = z3::Solver::new(&ctx);
        let mut predefined_values = std::collections::HashMap::new();
        if let Some(ref values) = values_for_predicates {
            for predicate in invariant.predicate_set.predicates.iter() {
                if let Some(&val) = values.get(&predicate.signal_name) {
                    let bv_value = z3::ast::BV::from_i64(&ctx, val, predicate.signal_length);
                    predefined_values.insert(predicate.signal_name.clone(), bv_value);
                }
            }
            println!("Predefined predicate values: {:?}", predefined_values);
        }
        let mut predicate_hashmap: HashMap<&str,z3::ast::BV> = HashMap::new();

        for (key, value) in predefined_values.iter() {
            predicate_hashmap.insert(key.as_str(), value.clone());
        }
        //println!("Score Invariant {:?}", self.score_invariant(invariant));
        let model = self.get_model_for_invariant(invariant, &ctx, Some(&mut predicate_hashmap));
        println!("Model {:?}", model);
        for waveform in self.cex_waveforms.iter() {
            let formula = construct_z3_formula_for_waveform_and_invariant(waveform, &invariant,true,&ctx,&mut predicate_hashmap);
            let res= solver.check_assumptions(&[formula]);
            if res == SatResult::Unsat {
                println!("CEX waveform {:?} not fulffilled", waveform.path);
            } else {
                //println!("Fulfilled: CEX waveform {:?} fulfilled", waveform.path);
            }
        }
        for waveform in self.bex_waveforms.iter() {
            let formula = construct_z3_formula_for_waveform_and_invariant(waveform, &invariant,false,&ctx,&mut predicate_hashmap);
            let res= solver.check_assumptions(&[formula]);
            if res == SatResult::Unsat {
                println!("BEX waveform {:?} not fulffilled", waveform.path);
                for cycle in 0..waveform.num_cycles {
                    let cycle_formula = construct_z3_formula_for_waveform_and_cycle(waveform, invariant, &mut predicate_hashmap, cycle, &ctx);
                    let assumption_list = [cycle_formula.not()];
                    let res= solver.check_assumptions(&assumption_list);
                    if res == SatResult::Unsat {
                        println!("Cycle {} not fulfilled, formula {:?}", cycle, assumption_list[0]);
                    } else {
                        //println!("Fulfilled: CEX waveform {:?} fulfilled", waveform.path);
                    }
                    //println!("Cycle {} fulfilled, formula {:?}", cycle, formula);
                }
            } else {
                //println!("Fulfilled: BEX waveform {:?} fulfilled", waveform.path);
            }
        }
    }   

    pub fn score_invariant(&self, invariant: &predicates::Invariant) -> predicates::PriorityScores {
        let mut score: predicates::PriorityScores = Default::default();

        let cfg = z3::Config::new();
        let ctx = z3::Context::new(&cfg);
        let optimizer = z3::Optimize::new(&ctx);
        let num_cex = self.cex_waveforms.len() as i64;
        let num_bex = self.bex_waveforms.len() as i64;
        let mut predicate_hashmap: HashMap<&str,z3::ast::BV> = HashMap::new();
        for waveform in self.cex_waveforms.iter() {
            let formula = construct_z3_formula_for_waveform_and_invariant(waveform, &invariant,true,&ctx, &mut predicate_hashmap);
            optimizer.assert_soft(&formula, 1, None);
        }
        //if !(cex_only) {
            //num_total_cex += self.bex_waveforms.len() as i64;
        let mut bex_formulas = Vec::new();
        for waveform in self.bex_waveforms.iter() {
            let formula = construct_z3_formula_for_waveform_and_invariant(waveform, &invariant,false,&ctx, &mut predicate_hashmap);
            bex_formulas.push(formula);
            //optimizer.assert_soft(&formula, 1, None);
        }
        let res = optimizer.check(&[]);
        score.cex_only_score = self.get_score_from_model(res, &optimizer, num_cex);
        //score.cex_with_bex_block_score = None;
        //return score;
        let res = optimizer.check(&bex_formulas.as_slice()); 
        score.cex_with_bex_block_score = self.get_score_from_model(res, &optimizer, num_cex);
        //Last: Check score with cex and bex allowed
        
        //let optimizer = z3::Optimize::new(&ctx);
        for formula in bex_formulas.iter() {
            optimizer.assert_soft(formula, 1, None);
        }
        let res = optimizer.check(&[]);
        score.cex_and_bex_score = self.get_score_from_model(res, &optimizer,num_cex + num_bex);
        score
    }


    pub fn get_model_for_invariant<'ctx>(&self, invariant: &'ctx predicates::Invariant, z3_ctx: &'ctx z3::Context, predicate_hashmap: Option<&mut HashMap<&'ctx str,z3::ast::BV<'ctx>>>) -> Option<Model<'ctx>> {
        let mut predicate_hashmap = match predicate_hashmap {
            Some(map) => map,
            None => &mut HashMap::new(),
        };
        let optimizer = z3::Optimize::new(&z3_ctx);
        for waveform in self.cex_waveforms.iter() {
            let formula = construct_z3_formula_for_waveform_and_invariant(waveform, &invariant,true,&z3_ctx, &mut predicate_hashmap);
            optimizer.assert_soft(&formula, 1, None);
        }
        //if !(cex_only) {
            //num_total_cex += self.bex_waveforms.len() as i64;
        let mut bex_formulas = Vec::new();
        for waveform in self.bex_waveforms.iter() {
            let formula = construct_z3_formula_for_waveform_and_invariant(waveform, &invariant,false,&z3_ctx, &mut predicate_hashmap);
            bex_formulas.push(formula);
            //optimizer.assert_soft(&formula, 1, None);
        }
        let res = optimizer.check(&bex_formulas.as_slice()); 
        if res == z3::SatResult::Sat {
            let model = optimizer.get_model().unwrap();
            return Some(model);
        }
        None
    }

    pub fn print_invariant_with_model(&self, invariant: &predicates::Invariant) {
        let cfg = z3::Config::new();
        let ctx = z3::Context::new(&cfg);
        let optimizer = z3::Optimize::new(&ctx);
        let mut predicate_hashmap: HashMap<&str,z3::ast::BV> = HashMap::new();
        for waveform in self.cex_waveforms.iter() {
            let formula = construct_z3_formula_for_waveform_and_invariant(waveform, &invariant,true,&ctx, &mut predicate_hashmap);
            optimizer.assert_soft(&formula, 1, None);
        }
        //if !(cex_only) {
            //num_total_cex += self.bex_waveforms.len() as i64;
            //
            //
            //

        let optimizer = z3::Optimize::new(&ctx);
        let mut bex_formulas = Vec::new();
        for waveform in self.bex_waveforms.iter() {
            let formula = construct_z3_formula_for_waveform_and_invariant(waveform, &invariant,false,&ctx, &mut predicate_hashmap);
            bex_formulas.push(formula);
            //optimizer.assert_soft(&formula, 1, None);
        }
        let res = optimizer.check(&bex_formulas.as_slice()); 
        if res == z3::SatResult::Sat {
            let model = optimizer.get_model().unwrap();
            println!("Model {:?}", model);
            
        }
    }
}
