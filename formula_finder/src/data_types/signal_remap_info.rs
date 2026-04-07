use core::panic;
use std::{collections::HashSet, fmt, sync::Arc};
use crate::data_types::{general_data_types::{DefaultScalarHasher, SignalIndexSet}, signal_filters::{self}};
use serde::Deserialize;

pub type ConditionMap = std::collections::HashMap<String, Expr>;


//Information on which signals are present in JG
//and which signals are signal inserted
//by verilator. This is used to remap
//separator formulas found by insight to JG formulas
#[derive(Debug, Clone)]
pub struct SignalRemappingInfo {
    pub jg_signals: Option<Vec<String>>,
    pub cond_map: Option<ConditionMap>,
}

/// Represents a Verilog expression serialized from exprToJson.
#[derive(Debug, Clone)]
pub enum Expr {
    Var { name: ustr::Ustr },

    Const { value: String },

    Unary {
        op: String,
        arg: Box<Expr>,
    },

    Binary {
        op: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },


    Mux {
        cond: Box<Expr>,
        then: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Slice {
        value: Box<Expr>,
        lsb: Box<Expr>,
        width: Option<u32>,
    },
    Index {
        value: Box<Expr>,
        index: Box<Expr>,
    },
    Concat {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Replicate {
        count: Box<Expr>,
        value: Box<Expr>,
    },
    Unknown {
        node: String,
    },
}

// Custom deserialization for Expr
impl<'de> Deserialize<'de> for Expr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};

        struct ExprVisitor;

        impl<'de> Visitor<'de> for ExprVisitor {
            type Value = Expr;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a Verilog expression object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut type_field: Option<String> = None;
                let mut fields: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();

                while let Some(key) = map.next_key::<String>()? {
                    if key == "type" {
                        type_field = Some(map.next_value()?);
                    } else {
                        fields.insert(key, map.next_value()?);
                    }
                }

                let type_str = type_field.ok_or_else(|| de::Error::missing_field("type"))?;

                match type_str.as_str() {
                    "var" => {
                        let name = fields.get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| de::Error::missing_field("name"))?;
                        Ok(Expr::Var { name: ustr::Ustr::from(name) })
                    }
                    "const" => {
                        let value = fields.get("value")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| de::Error::missing_field("value"))?;
                        Ok(Expr::Const { value: value.to_string() })
                    }
                    "unary" => {
                        let op = fields.get("op")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| de::Error::missing_field("op"))?;
                        let arg: Expr = serde_json::from_value(fields.get("arg").cloned().ok_or_else(|| de::Error::missing_field("arg"))?)
                            .map_err(de::Error::custom)?;
                        Ok(Expr::Unary { op: op.to_string(), arg: Box::new(arg) })
                    }
                    "binary" => {
                        let op = fields.get("op")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| de::Error::missing_field("op"))?;
                        let lhs: Expr = serde_json::from_value(fields.get("lhs").cloned().ok_or_else(|| de::Error::missing_field("lhs"))?)
                            .map_err(de::Error::custom)?;
                        let rhs: Expr = serde_json::from_value(fields.get("rhs").cloned().ok_or_else(|| de::Error::missing_field("rhs"))?)
                            .map_err(de::Error::custom)?;
                        Ok(Expr::Binary { op: op.to_string(), lhs: Box::new(lhs), rhs: Box::new(rhs) })
                    }
                    "mux" => {
                        let cond: Expr = serde_json::from_value(fields.get("cond").cloned().ok_or_else(|| de::Error::missing_field("cond"))?)
                            .map_err(de::Error::custom)?;
                        let then_branch: Expr = serde_json::from_value(fields.get("then").cloned().ok_or_else(|| de::Error::missing_field("then"))?)
                            .map_err(de::Error::custom)?;
                        let else_branch: Expr = serde_json::from_value(fields.get("else").cloned().ok_or_else(|| de::Error::missing_field("else"))?)
                            .map_err(de::Error::custom)?;
                        Ok(Expr::Mux { cond: Box::new(cond), then: Box::new(then_branch), else_branch: Box::new(else_branch) })
                    }
                    "slice" => {
                        let value: Expr = serde_json::from_value(fields.get("value").cloned().ok_or_else(|| de::Error::missing_field("value"))?)
                            .map_err(de::Error::custom)?;
                        let lsb: Expr = serde_json::from_value(fields.get("lsb").cloned().ok_or_else(|| de::Error::missing_field("lsb"))?)
                            .map_err(de::Error::custom)?;
                        let width = fields.get("width").and_then(|v| v.as_u64()).map(|v| v as u32);
                        Ok(Expr::Slice { value: Box::new(value), lsb: Box::new(lsb), width })
                    }
                    "index" => {
                        let value: Expr = serde_json::from_value(fields.get("value").cloned().ok_or_else(|| de::Error::missing_field("value"))?)
                            .map_err(de::Error::custom)?;
                        let index: Expr = serde_json::from_value(fields.get("index").cloned().ok_or_else(|| de::Error::missing_field("index"))?)
                            .map_err(de::Error::custom)?;
                        Ok(Expr::Index { value: Box::new(value), index: Box::new(index) })
                    }
                    "concat" => {
                        let lhs: Expr = serde_json::from_value(fields.get("lhs").cloned().ok_or_else(|| de::Error::missing_field("lhs"))?)
                            .map_err(de::Error::custom)?;
                        let rhs: Expr = serde_json::from_value(fields.get("rhs").cloned().ok_or_else(|| de::Error::missing_field("rhs"))?)
                            .map_err(de::Error::custom)?;
                        Ok(Expr::Concat { lhs: Box::new(lhs), rhs: Box::new(rhs) })
                    }
                    "replicate" => {
                        let count: Expr = serde_json::from_value(fields.get("count").cloned().ok_or_else(|| de::Error::missing_field("count"))?)
                            .map_err(de::Error::custom)?;
                        let value: Expr = serde_json::from_value(fields.get("value").cloned().ok_or_else(|| de::Error::missing_field("value"))?)
                            .map_err(de::Error::custom)?;
                        Ok(Expr::Replicate { count: Box::new(count), value: Box::new(value) })
                    }
                    "unknown" => {
                        let node = fields.get("node")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        Ok(Expr::Unknown { node })
                    }
                    other => {
                        Ok(Expr::Unknown { node: format!("unknown type: {}", other) })
                    }
                }
            }
        }

        deserializer.deserialize_map(ExprVisitor)
    }
}


/// Converts an internal operator name (as generated in exprToJson)
/// into a readable Verilog operator.
fn op_to_verilog(op: &str) -> &str {
    match op {
        "and" => "&",
        "or" => "|",
        "xor" => "^",
        "land" => "&&",
        "lor" => "||",
        "not" | "lnot" => "!",
        "add" => "+",
        "sub" => "-",
        "mul" => "*",
        "div" => "/",
        "mod" => "%",
        "lt" => "<",
        "lte" => "<=",
        "gt" => ">",
        "gte" => ">=",
        "eq" => "==",
        "ne" => "!=",
        "eqx" => "===",
        "nex" => "!==",
        "shl" => "<<",
        "shr" => ">>",
        "ashr" => ">>>",
        "redor" => "|",
        "redand" => "&",
        "redxor" => "^",
        _ => op, // fallback
    }
}

impl Expr {
    /// Reconstructs a Verilog-like expression string from the AST.
    pub fn to_verilog(&self, maybe_verilator_to_idx_mapping: Option<&std::collections::HashMap<ustr::Ustr, u64>>, idx_to_jg_mapping: Option<&std::collections::HashMap<u64,Arc<str>>>) -> String {
        match self {
            Expr::Var { name } => {
                match maybe_verilator_to_idx_mapping {
                    Some(verilator_to_idx_mapping) => {
                        if let Some(mapped_idx) = verilator_to_idx_mapping.get(name) {
                            if let Some(mapped_name) = idx_to_jg_mapping.as_ref().unwrap().get(mapped_idx) {
                                return mapped_name.to_string();
                            } else {
                                panic!("Warning: Signal index '{}' name '{}' not found in idx_to_jg_mapping", mapped_idx, name);
                            }
                        } else {
                            panic!("Warning: Signal name '{}' not found in verilator_to_idx_mapping", name);
                        }
                    }
                    None => {
                        return name.to_string();
                    }
                }
            },
            Expr::Const { value } => value.clone(),

            Expr::Unary { op, arg } => format!("({}{})", op_to_verilog(op), arg.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping)),

            Expr::Binary { op, lhs, rhs } => {
                format!("({} {} {})", lhs.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping), op_to_verilog(op), rhs.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping))
            }

            Expr::Slice { value, lsb, width, .. } => {
                if let Some(w) = width {
                    format!("{}[{} +: {}]", value.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping), lsb.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping), w)
                } else {
                    format!("{}[{}]", value.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping), lsb.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping))
                }
            }

            Expr::Index { value, index, .. } => {
                format!("{}[{}]", value.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping), index.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping))
            }

            Expr::Mux { cond, then, else_branch, .. } => {
                format!("({} ? {} : {})",
                    cond.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping),
                    then.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping),
                    else_branch.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping))
            }

            Expr::Concat { lhs, rhs, .. } => {
                format!("{{{}, {}}}", lhs.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping), rhs.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping))
            }

            Expr::Replicate { count, value, .. } => {
                format!("{{{}{{{}}}}}", count.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping), value.to_verilog(maybe_verilator_to_idx_mapping, idx_to_jg_mapping))
            }

            Expr::Unknown { node } => {format!("/* Unknown node: {} */", node)}
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_verilog(None, None))
    }
}

impl Expr {

    pub fn get_signal_names(&self) -> Vec<String> {
        let mut signals = Vec::new();
        self.collect_signals_from_expr(&mut signals);
        signals.sort();
        signals.dedup();
        let signals = signals.into_iter().map(|s| s.to_string()).collect::<Vec<String>>();
        signals
    }
    /// Recursively collects all variable names used in the expression.
    fn collect_signals_from_expr(&self, signals: &mut Vec<ustr::Ustr>) {
        match self {
            Expr::Var { name } => {
                signals.push(*name);
            }
            Expr::Const { .. } => {}
            Expr::Unary { arg, .. } => {
                arg.collect_signals_from_expr(signals);
            }
            Expr::Binary { lhs, rhs, .. } => {
                lhs.collect_signals_from_expr(signals);
                rhs.collect_signals_from_expr(signals);
            }
            Expr::Slice { value, lsb, .. } => {
                value.collect_signals_from_expr(signals);
                lsb.collect_signals_from_expr(signals);
            }
            Expr::Index { value, index, .. } => {
                value.collect_signals_from_expr(signals);
                index.collect_signals_from_expr(signals);
            }
            Expr::Mux { cond, then, else_branch, .. } => {
                cond.collect_signals_from_expr(signals);
                then.collect_signals_from_expr(signals);
                else_branch.collect_signals_from_expr(signals);
            }
            Expr::Concat { lhs, rhs, .. } => {
                lhs.collect_signals_from_expr(signals);
                rhs.collect_signals_from_expr(signals);
            }
            Expr::Replicate { count, value, .. } => {
                count.collect_signals_from_expr(signals);
                value.collect_signals_from_expr(signals);
            }
            Expr::Unknown { node } => {
                println!("/* Unknown node  in cond_map. Will not generate predicates for it: {} */", node);
            }
                
        }
    }

    pub fn expression_okay_for_signal_list(&self, jg_signals: &HashSet<ustr::Ustr>) -> bool {
        let mut signals_in_expr = Vec::new();
        self.collect_signals_from_expr(&mut signals_in_expr);
        for signal in signals_in_expr {
            if !jg_signals.contains(&signal) {
                return false;
            }
        }
        true
    }
}

impl SignalRemappingInfo {
    fn new(jg_signals: Option<Vec<String>>, cond_map: Option<std::collections::HashMap<String, Expr>>) -> Self {
        SignalRemappingInfo { jg_signals, cond_map: cond_map }
    }
    pub fn load_from_files(jg_signals_file: &str, cond_map_file: Option<&str>) -> Self {
        let jg_signals_content = std::fs::read_to_string(jg_signals_file)
            .expect("Failed to read JG signals file");
        let jg_signals: Vec<String> = jg_signals_content
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        if cond_map_file.is_none() {
            return SignalRemappingInfo::new(Some(jg_signals), None);
        } else {
            let cond_map_content = std::fs::read_to_string(cond_map_file.unwrap())
                    .expect("Failed to read condition map file");
            let cond_map: std::collections::HashMap<String, Expr> = serde_json::from_str(&cond_map_content)
                    .expect("Failed to parse condition map JSON");

            SignalRemappingInfo::new(Some(jg_signals), Some(cond_map))
        }
        
    }

    pub fn load_from_json_strings(jg_signals_json: Option<&serde_json::Value>, cond_map_json: Option<&serde_json::Value>) -> Self {
        if jg_signals_json.is_none() && cond_map_json.is_none() {
            panic!("Both JG signals JSON and condition map JSON are None");
        }
        let jg_signals = if jg_signals_json.is_none() {
            None
        } else {
            Some(serde_json::from_value(jg_signals_json.unwrap().clone())
                .expect("Failed to parse JG signals JSON"))
        };
        if cond_map_json.is_none() {
            return SignalRemappingInfo::new(jg_signals, None);
        } else {
            // println!("Condition map JSON: {}", cond_map_json.unwrap());
            
            // match serde_json::from_str::<Expr>(&cond_map_json.unwrap()) {
            //     Ok(expr) => println!("Parsed: {:?}", expr),
            //     Err(e) => {
            //         println!("Failed to parse JSON: {}", e);
            //         if let Ok(val) = serde_json::from_str::<serde_json::Value>(&cond_map_json.unwrap()) {
            //             println!("JSON shape: {}", val);
            //         }
            //     }
            // }
            println!("Parsing condition map JSON...");
            let cond_map_json = cond_map_json.unwrap();
            if let Some(obj) = cond_map_json.as_object() {
                for (key, val) in obj {
                    let res: Result<Expr, _> = serde_json::from_value(val.clone());
                    match res {
                        Ok(_expr) => {
                            // println!("OK  -> {}", key),
                        },
                        Err(e) => {
                            eprintln!("❌ Failed for key '{}': {}", key, e);
                            eprintln!("Offending JSON: {}", val);
                            panic!("Failed to parse condition map JSON");
                        }
                    }
                }
            } else {
                    eprintln!("Root of cond_map is not an object, but {:?}", cond_map_json);
            }
            let cond_map: std::collections::HashMap<String, Expr> = match serde_json::from_value(cond_map_json.clone()) {
                Ok(map) => map,
                Err(e) => {
                    eprintln!("Failed to parse condition map JSON: {}", e);
                    eprintln!("Condition map JSON value: {}", cond_map_json);
                    panic!("Failed to parse condition map JSON");
                }
            };


            SignalRemappingInfo::new(jg_signals, Some(cond_map))
        }
    }

    pub fn create_allowed_signal_list_from_name_to_code_map(&self, name_to_code_map: &std::collections::HashMap<ustr::Ustr, u64, DefaultScalarHasher>) -> SignalIndexSet {
        if self.jg_signals.is_none() {
            return name_to_code_map.values().cloned().collect();
        }
        let jg_signals_arc_set: HashSet<Arc<str>> = self.jg_signals.as_ref().unwrap().iter().map(|s| Arc::from(s.as_str())).collect();
        let jg_filter_info = signal_filters::JGFilterInfo::new(jg_signals_arc_set);
        let jg_filter = signal_filters::SignalFilter::JGFilter(jg_filter_info);
        //First: Build a list of all signals (and respective aliases) that are allowed under JG filter
        let mut allowed_signal_codes = SignalIndexSet::default();
        for (signal_name, signal_code) in name_to_code_map.iter() {
            if jg_filter.check_signal(signal_name, signal_code) {
                allowed_signal_codes.insert(*signal_code);
            } else {
                //println!("Signal {} with code {} not in JG signals", signal_name, signal_code);
            }
        }
        let jg_allowed_signals = allowed_signal_codes.clone();
        //Now: If we have cond map, we need to add those signals as well
        if self.cond_map.is_some() {
            for (cond_signal, expr) in self.cond_map.as_ref().unwrap().iter() {
                let mut signals_in_expr = Vec::new();
                expr.collect_signals_from_expr(&mut signals_in_expr);
                let mut found_all_signals = true;
                for signal in signals_in_expr {
                    if !name_to_code_map.contains_key(&ustr::Ustr::from(&signal)) {
                        found_all_signals = false;
                        break;
                    }
                    let signal_code = name_to_code_map.get(&ustr::Ustr::from(&signal)).unwrap();
                    if jg_allowed_signals.contains(signal_code) {
                        continue;
                    } else {
                        found_all_signals = false;
                        break;
                    }
                }
                if found_all_signals {
                    if let Some(cond_signal_code) = name_to_code_map.get(&ustr::Ustr::from(&cond_signal)) {
                        if !allowed_signal_codes.contains(cond_signal_code) {
                            allowed_signal_codes.insert(*cond_signal_code);
                        }
                    }
                }
            }
        }
        allowed_signal_codes
    }

}

