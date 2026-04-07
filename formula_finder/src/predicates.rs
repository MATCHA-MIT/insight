use crate::constants;
use crate::data_types;
use core::hash;
use serde;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::slice::Iter;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(
    PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize, hash::Hash, Default,
)]
pub enum Operator {
    #[default]
    Equal,
    NotEqual,
    GreaterEqual,
    SmallerEqual,
}
impl Operator {
    pub fn iterator() -> Iter<'static, Operator> {
        static OPERATIONS: [Operator; 4] = [
            Operator::Equal,
            Operator::NotEqual,
            Operator::GreaterEqual,
            Operator::SmallerEqual,
        ];
        OPERATIONS.iter()
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct BaseSignalToConstFormula {
    pub signal_name: Arc<str>,
    pub signal_idx: Option<u64>,
    pub operator: Operator,
    pub signal_length: u32,
    #[serde(default)]
    pub signal_types: data_types::general_data_types::SignalTypesSet,
    pub value: i64,
}

impl BaseSignalToConstFormula {
    pub fn get_value(&self) -> i64 {
        self.value
    }
}

impl PartialEq for BaseSignalToConstFormula {
    fn eq(&self, other: &Self) -> bool {
        (self.signal_name == other.signal_name || self.signal_idx == other.signal_idx)
            && self.operator == other.operator
            && self.signal_length == other.signal_length
            && self.value == other.value
    }
}

impl hash::Hash for BaseSignalToConstFormula {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.operator.hash(state);
        self.signal_length.hash(state);
    }
}

impl Eq for BaseSignalToConstFormula {}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct BaseSignalToConstSetFormula {
    pub signal_name: Arc<str>,
    pub signal_idx: Option<u64>,
    pub signal_length: u32,
    #[serde(default)]
    pub signal_types: data_types::general_data_types::SignalTypesSet,
    pub values: Vec<i64>,
}

impl BaseSignalToConstSetFormula {
    pub fn get_values(&self) -> &[i64] {
        &self.values
    }
}

impl PartialEq for BaseSignalToConstSetFormula {
    fn eq(&self, other: &Self) -> bool {
        (self.signal_name == other.signal_name || self.signal_idx == other.signal_idx)
            && self.signal_length == other.signal_length
            && self.values == other.values
    }
}

impl hash::Hash for BaseSignalToConstSetFormula {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.signal_length.hash(state);
    }
}

impl Eq for BaseSignalToConstSetFormula {}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct BaseFormulaTwoSignalCompare {
    pub signal_name1: Arc<str>,
    pub signal_name2: Arc<str>,
    pub signal_idx1: Option<u64>,
    pub signal_idx2: Option<u64>,
    pub operation: Operator,
}

impl BaseFormulaTwoSignalCompare {
    pub fn get_inverse(&self) -> Self {
        let mut new_formula = self.clone();
        new_formula.operation = match self.operation {
            Operator::Equal => Operator::NotEqual,
            Operator::NotEqual => Operator::Equal,
            Operator::GreaterEqual => Operator::SmallerEqual,
            Operator::SmallerEqual => Operator::GreaterEqual,
        };
        new_formula
    }
    pub fn get_signal_names(&self) -> Vec<Arc<str>> {
        vec![self.signal_name1.clone(), self.signal_name2.clone()]
    }
    pub fn get_signal_idx(&self) -> Vec<u64> {
        match (self.signal_idx1, self.signal_idx2) {
            (None, None) => return vec![],
            (Some(_), None) => return vec![],
            (None, Some(_)) => return vec![],
            (Some(sig1_idx_inner), Some(sig2_idx_inner)) => {
                return vec![sig1_idx_inner, sig2_idx_inner]
            }
        }
    }
}

impl PartialEq for BaseFormulaTwoSignalCompare {
    fn eq(&self, other: &Self) -> bool {
        let signal_name_equal = ((self.signal_name1 == other.signal_name1
            || self.signal_idx1 == other.signal_idx1)
            && (self.signal_name2 == other.signal_name2 || self.signal_idx2 == other.signal_idx2))
            || ((self.signal_name1 == other.signal_name2 || self.signal_idx1 == other.signal_idx2)
                && (self.signal_name2 == other.signal_name1
                    || self.signal_idx2 == other.signal_idx1));
        signal_name_equal && self.operation == other.operation
    }
}

impl hash::Hash for BaseFormulaTwoSignalCompare {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.operation.hash(state);
    }
}

impl Eq for BaseFormulaTwoSignalCompare {}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum BaseFormula {
    SignalToConst(BaseSignalToConstFormula),
    ValueNotIn(BaseSignalToConstSetFormula),
    ValueIn(BaseSignalToConstSetFormula),
    TwoSignalEqual(BaseFormulaTwoSignalCompare),
}

impl BaseFormula {
    pub fn get_signal_names(&self) -> Vec<Arc<str>> {
        match self {
            BaseFormula::SignalToConst(formula) => vec![formula.signal_name.clone()],
            BaseFormula::ValueNotIn(formula) => vec![formula.signal_name.clone()],
            BaseFormula::ValueIn(formula) => vec![formula.signal_name.clone()],
            BaseFormula::TwoSignalEqual(formula) => {
                vec![formula.signal_name1.clone(), formula.signal_name2.clone()]
            }
        }
    }
    pub fn get_inverse(&self) -> Self {
        match self {
            BaseFormula::SignalToConst(formula) => {
                BaseFormula::SignalToConst(formula.get_inverse())
            }
            BaseFormula::ValueNotIn(formula) => BaseFormula::ValueIn(formula.clone()),
            BaseFormula::ValueIn(formula) => BaseFormula::ValueNotIn(formula.clone()),
            BaseFormula::TwoSignalEqual(formula) => {
                BaseFormula::TwoSignalEqual(formula.get_inverse())
            }
        }
    }
}

impl Ord for BaseFormula {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let rank = |formula: &BaseFormula| -> u8 {
            match formula {
                BaseFormula::SignalToConst(_) => 0,
                BaseFormula::ValueNotIn(_) => 1,
                BaseFormula::ValueIn(_) => 2,
                BaseFormula::TwoSignalEqual(_) => 3,
            }
        };
        let rank_cmp = rank(self).cmp(&rank(other));
        if rank_cmp != std::cmp::Ordering::Equal {
            return rank_cmp;
        }
        match (self, other) {
            (BaseFormula::SignalToConst(formula1), BaseFormula::SignalToConst(formula2)) => {
                formula1.signal_length.cmp(&formula2.signal_length)
            }
            (BaseFormula::ValueNotIn(formula1), BaseFormula::ValueNotIn(formula2)) => {
                formula1.signal_length.cmp(&formula2.signal_length)
            }
            (BaseFormula::ValueIn(formula1), BaseFormula::ValueIn(formula2)) => {
                formula1.signal_length.cmp(&formula2.signal_length)
            }
            (BaseFormula::TwoSignalEqual(formula1), BaseFormula::TwoSignalEqual(formula2)) => {
                formula1.signal_name1.cmp(&formula2.signal_name1)
            }
            _ => std::cmp::Ordering::Equal,
        }
    }
}

impl PartialOrd for BaseFormula {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BasePredicateCandidate {
    pub base_predicate: BasePredicate,
    pub only_in_cycles: Option<Vec<usize>>, //If Some, then the predicate can only be paired with other predicates from this cycle or ''none''
}

impl BasePredicateCandidate {
    pub fn get_operator(&self) -> Operator {
        self.base_predicate.get_operator()
    }
    pub fn get_signal_names(&self) -> Vec<Arc<str>> {
        self.base_predicate.get_signal_names()
    }
    pub fn get_signal_idx(&self) -> HashSet<u64> {
        self.base_predicate.get_signal_idx()
    }

    pub fn to_invariant(&self) -> Invariant {
        Invariant {
            predicate_set: PredicateSet {
                predicates: vec![self.base_predicate.clone()],
            },
        }
    }
}

impl fmt::Display for BasePredicateCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.base_predicate)
    }
}

impl hash::Hash for BasePredicateCandidate {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        // Only hash field1
        self.base_predicate.hash(state);
        self.only_in_cycles.hash(state);
    }
}

impl Eq for BasePredicateCandidate {}

impl PartialEq for BasePredicateCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.base_predicate == other.base_predicate && self.only_in_cycles == other.only_in_cycles
    }
}

//Predicate struct: A struct that represent a predicate of the
//signal == CONST or signal != CONST
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BasePredicate {
    pub base_formula: BaseFormula,
}

impl BasePredicate {
    pub fn get_operator(&self) -> Operator {
        match &self.base_formula {
            BaseFormula::SignalToConst(formula) => formula.operator.clone(),
            BaseFormula::ValueNotIn(_) => Operator::NotEqual,
            BaseFormula::ValueIn(_) => Operator::Equal,
            BaseFormula::TwoSignalEqual(formula) => formula.operation.clone(),
        }
    }
    pub fn get_signal_names(&self) -> Vec<Arc<str>> {
        return self.base_formula.get_signal_names();
    }
    pub fn get_signal_idx(&self) -> HashSet<u64> {
        let mut signal_idx_set = HashSet::new();
        match &self.base_formula {
            BaseFormula::SignalToConst(formula) => {
                if let Some(idx) = formula.signal_idx {
                    signal_idx_set.insert(idx);
                    return signal_idx_set;
                } else {
                    return signal_idx_set;
                }
            }
            BaseFormula::ValueNotIn(formula) | BaseFormula::ValueIn(formula) => {
                if let Some(idx) = formula.signal_idx {
                    signal_idx_set.insert(idx);
                    return signal_idx_set;
                } else {
                    return signal_idx_set;
                }
            }
            BaseFormula::TwoSignalEqual(formula) => formula
                .get_signal_idx()
                .iter()
                .cloned()
                .collect::<HashSet<u64>>(),
        }
    }
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let operator_str = match self {
            Operator::Equal => "==",
            Operator::NotEqual => "!=",
            Operator::GreaterEqual => ">=",
            Operator::SmallerEqual => "<=",
        };
        write!(f, "{}", operator_str)
    }
}

impl fmt::Display for BaseSignalToConstFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let operator_str = match self.operator {
            Operator::Equal => "==",
            Operator::NotEqual => "!=",
            Operator::GreaterEqual => ">=",
            Operator::SmallerEqual => "<=",
        };
        write!(f, "{} {} {}", self.signal_name, operator_str, self.value)
    }
}

impl fmt::Display for BaseFormulaTwoSignalCompare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.signal_name1, self.operation, self.signal_name2
        )
    }
}

impl fmt::Display for BasePredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.base_formula {
            BaseFormula::SignalToConst(formula) => write!(f, "{}", formula),
            BaseFormula::ValueNotIn(formula) => {
                let values_str = formula
                    .values
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "{} not in {{{}}}", formula.signal_name, values_str)
            }
            BaseFormula::ValueIn(formula) => {
                let values_str = formula
                    .values
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "{} in {{{}}}", formula.signal_name, values_str)
            }
            BaseFormula::TwoSignalEqual(formula) => write!(f, "{}", formula),
        }
    }
}

impl hash::Hash for BasePredicate {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        // Only hash field1
        self.base_formula.hash(state);
    }
}

impl BaseSignalToConstFormula {
    pub fn get_inverse(&self) -> Self {
        let mut new_formula = self.clone();
        new_formula.operator = match self.operator {
            Operator::Equal => Operator::NotEqual,
            Operator::NotEqual => Operator::Equal,
            Operator::GreaterEqual => Operator::SmallerEqual,
            Operator::SmallerEqual => Operator::GreaterEqual,
        };
        new_formula
    }
}

impl PartialEq for BasePredicate {
    fn eq(&self, other: &Self) -> bool {
        if let (BaseFormula::SignalToConst(formula1), BaseFormula::SignalToConst(formula2)) =
            (&self.base_formula, &other.base_formula)
        {
            formula1 == formula2
        } else if let (
            BaseFormula::ValueNotIn(formula1),
            BaseFormula::ValueNotIn(formula2),
        ) = (&self.base_formula, &other.base_formula)
        {
            formula1 == formula2
        } else if let (BaseFormula::ValueIn(formula1), BaseFormula::ValueIn(formula2)) =
            (&self.base_formula, &other.base_formula)
        {
            formula1 == formula2
        } else if let (
            BaseFormula::TwoSignalEqual(formula1),
            BaseFormula::TwoSignalEqual(formula2),
        ) = (&self.base_formula, &other.base_formula)
        {
            formula1 == formula2
        } else {
            false
        }
    }
}

impl Eq for BasePredicate {}

impl BasePredicate {
    pub fn new_from_info_and_value(
        signal_info: &data_types::general_data_types::SignalInfo,
        operator: Operator,
        value: i64,
    ) -> Self {
        BasePredicate {
            base_formula: BaseFormula::SignalToConst(BaseSignalToConstFormula {
                signal_name: signal_info.get_signal_name().into(),
                signal_idx: signal_info.id.into(),
                operator,
                signal_length: signal_info.length as u32,
                value,
                signal_types: signal_info.signal_types.clone(),
            }),
        }
    }

    pub fn new_from_value(
        signal_name: Arc<str>,
        operator: Operator,
        signal_length: usize,
        value: i64,
        signal_idx: Option<u64>,
    ) -> Self {
        BasePredicate {
            base_formula: BaseFormula::SignalToConst(BaseSignalToConstFormula {
                signal_name: signal_name,
                signal_idx: signal_idx,
                operator,
                signal_length: signal_length as u32,
                value: value,
                signal_types: data_types::general_data_types::SignalTypesSet::default(), //We don't know the signal type here
            }),
        }
    }

    pub fn new_from_value_set(
        signal_name: Arc<str>,
        signal_length: usize,
        values: Vec<i64>,
        signal_idx: Option<u64>,
        not_in: bool,
    ) -> Self {
        let base_formula = BaseSignalToConstSetFormula {
            signal_name,
            signal_idx,
            signal_length: signal_length as u32,
            values,
            signal_types: data_types::general_data_types::SignalTypesSet::default(), //We don't know the signal type here
        };
        BasePredicate {
            base_formula: if not_in {
                BaseFormula::ValueNotIn(base_formula)
            } else {
                BaseFormula::ValueIn(base_formula)
            },
        }
    }

    pub fn new_from_two_signals(
        signal_name1: Arc<str>,
        signal_name2: Arc<str>,
        signal_idx: Option<u64>,
        signal_idx2: Option<u64>,
        operation: Operator,
    ) -> Self {
        BasePredicate {
            base_formula: BaseFormula::TwoSignalEqual(BaseFormulaTwoSignalCompare {
                signal_name1: signal_name1,
                signal_name2: signal_name2,
                signal_idx1: signal_idx,
                signal_idx2: signal_idx2,
                operation: operation,
            }),
        }
    }

    pub fn get_inverse(&self) -> Self {
        let mut new_predicate = self.clone();
        new_predicate.base_formula = new_predicate.base_formula.get_inverse();
        new_predicate
    }

    pub fn to_string(&self) -> String {
        match &self.base_formula {
            BaseFormula::SignalToConst(formula) => {
                let operator_str = match formula.operator {
                    Operator::Equal => "==",
                    Operator::NotEqual => "!=",
                    Operator::GreaterEqual => ">=",
                    Operator::SmallerEqual => "<=",
                };
                format!("{} {} {}", formula.signal_name, operator_str, formula.value)
            }
            BaseFormula::ValueNotIn(formula) => {
                let values_str = formula
                    .values
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                format!("{} not in {{{}}}", formula.signal_name, values_str)
            }
            BaseFormula::ValueIn(formula) => {
                let values_str = formula
                    .values
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                format!("{} in {{{}}}", formula.signal_name, values_str)
            }
            BaseFormula::TwoSignalEqual(formula) => {
                format!(
                    "{} {} {}",
                    formula.signal_name1, formula.operation, formula.signal_name2
                )
            }
        }
    }

    pub fn to_jg_string_with_signal_mapping(
        &self,
        signal_mapping: &std::collections::HashMap<u64, Arc<str>>,
    ) -> String {
        match &self.base_formula {
            BaseFormula::SignalToConst(formula) => {
                let operator_str = match formula.operator {
                    Operator::Equal => "==",
                    Operator::NotEqual => "!=",
                    Operator::GreaterEqual => ">=",
                    Operator::SmallerEqual => "<=",
                };
                let signal_name = signal_mapping
                    .get(&formula.signal_idx.unwrap())
                    .unwrap_or_else(|| {
                        panic!(
                            "Signal name {} not found in mapping, for predicate {}",
                            formula.signal_name,
                            self.to_string()
                        )
                    });
                format!("{} {} {}", signal_name, operator_str, formula.value)
            }
            BaseFormula::ValueNotIn(formula) => {
                let signal_name = signal_mapping
                    .get(&formula.signal_idx.unwrap())
                    .unwrap_or_else(|| {
                        panic!(
                            "Signal name {} not found in mapping, for predicate {}",
                            formula.signal_name,
                            self.to_string()
                        )
                    });
                if formula.values.is_empty() {
                    return "true".to_string();
                }
                formula
                    .values
                    .iter()
                    .map(|value| format!("{} != {}", signal_name, value))
                    .collect::<Vec<String>>()
                    .join(" && ")
            }
            BaseFormula::ValueIn(formula) => {
                let signal_name = signal_mapping
                    .get(&formula.signal_idx.unwrap())
                    .unwrap_or_else(|| {
                        panic!(
                            "Signal name {} not found in mapping, for predicate {}",
                            formula.signal_name,
                            self.to_string()
                        )
                    });
                if formula.values.is_empty() {
                    return "false".to_string();
                }
                formula
                    .values
                    .iter()
                    .map(|value| format!("{} == {}", signal_name, value))
                    .collect::<Vec<String>>()
                    .join(" || ")
            }
            BaseFormula::TwoSignalEqual(formula) => {
                let signal_name1 = signal_mapping.get(&formula.signal_idx1.unwrap()).unwrap(); //.unwrap_or(&formula.signal_name1);
                let signal_name2 = signal_mapping.get(&formula.signal_idx2.unwrap()).unwrap(); //.unwrap_or(&formula.signal_name2);
                format!("{} {} {}", signal_name1, formula.operation, signal_name2)
            }
        }
    }
}

/*
struct OrPredicate {
    pub sub_repdicates: Vec<BasePredicate>
}
*/

/*
impl OrPredicate {
    pub fn new(sub_repdicates: Vec<BasePredicate>) -> Self {
        OrPredicate {
            sub_repdicates
        }
    }

    pub fn new_not_in_set_predicate(signal_name: String, signal_length: usize, num_constants: usize) -> Self {
        //Create num_constants predicates for signal_name != constant
        let sub_repdicates = (0..num_constants)
            .map(|i| BasePredicate::new(signal_name.clone(), Operator::NotEqual, signal_length))
            .collect();
        OrPredicate {
            sub_repdicates
        }
    }

    pub fn to_string_with_value(&self, signal_value:&Vec <i64>) -> String {
       //Convert predicate to string with value
        let res = self.sub_repdicates
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let value = signal_value[i];
                p.to_string_with_value(value)
            })
            .collect::<Vec<String>>()
            .join(" || ");
        res
    }
}
*/

pub trait PredicateLike: Send + Sync + Clone + Eq {
    fn to_base_predicate(&self) -> BasePredicate;
    fn to_candidate(&self) -> BasePredicateCandidate;
    fn is_two_signal_equal(&self) -> bool;
    fn to_invariant(&self) -> Invariant;
}

impl PredicateLike for BasePredicate {
    fn to_base_predicate(&self) -> BasePredicate {
        self.clone()
    }
    fn to_candidate(&self) -> BasePredicateCandidate {
        BasePredicateCandidate {
            base_predicate: self.clone(),
            only_in_cycles: None,
        }
    }

    fn is_two_signal_equal(&self) -> bool {
        match &self.base_formula {
            BaseFormula::TwoSignalEqual(_) => true,
            _ => false,
        }
    }

    fn to_invariant(&self) -> Invariant {
        Invariant {
            predicate_set: PredicateSet {
                predicates: vec![self.to_base_predicate().clone()],
            },
        }
    }
}

impl PredicateLike for BasePredicateCandidate {
    fn to_base_predicate(&self) -> BasePredicate {
        self.base_predicate.clone()
    }
    fn to_candidate(&self) -> BasePredicateCandidate {
        self.clone()
    }

    fn is_two_signal_equal(&self) -> bool {
        match &self.base_predicate.base_formula {
            BaseFormula::TwoSignalEqual(_) => true,
            _ => false,
        }
    }

    fn to_invariant(&self) -> Invariant {
        Invariant {
            predicate_set: PredicateSet {
                predicates: vec![self.to_base_predicate().clone()],
            },
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct BasePredicateWithScoreAndObjective<P>
where
    P: PredicateLike + Clone + PartialEq,
{
    pub predicate: P,
    pub score: ScoreAndFulfilledExample,
    pub objective: InvariantObjective,
}

//PredicateSet struct: A struct that represent a set of predicates
#[derive(
    PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize, hash::Hash, Default,
)]
pub struct PredicateSet {
    pub predicates: Vec<BasePredicate>,
}

impl PredicateSet {
    pub fn new() -> Self {
        PredicateSet {
            predicates: Vec::new(),
        }
    }

    pub fn add_predicate(&mut self, predicate: BasePredicate) {
        self.predicates.push(predicate);
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct SeparatorFormulaWithPath {
    pub separator_formula: SeparatorFormula,
    pub path: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct InvariantWithPath {
    pub invariant: Invariant,
    pub path: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct Invariant {
    pub predicate_set: PredicateSet,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct InvariantDisjunction {
    pub disjunctions: Vec<Invariant>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SeparatorFormula {
    InvariantDisjunction(InvariantDisjunction),
    Invariant(Invariant),
}

impl Default for SeparatorFormula {
    fn default() -> Self {
        SeparatorFormula::InvariantDisjunction(InvariantDisjunction::new())
    }
}

impl fmt::Display for SeparatorFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeparatorFormula::InvariantDisjunction(inv_disj) => write!(f, "{}", inv_disj),
            SeparatorFormula::Invariant(inv) => write!(f, "{}", inv),
        }
    }
}

impl InvariantDisjunction {
    pub fn get_relevant_signal_idx(&self) -> HashSet<u64> {
        let mut signal_idx_set = HashSet::new();
        for inv in &self.disjunctions {
            let inv_signal_idx = inv.get_signal_idx();
            signal_idx_set.extend(inv_signal_idx);
        }
        signal_idx_set
    }

    pub fn get_relevant_signals(&self) -> HashSet<Arc<str>> {
        let mut signals = HashSet::new();
        for inv in &self.disjunctions {
            let inv_signals = inv.get_relevant_signals();
            signals.extend(inv_signals);
        }
        signals
    }
}

impl SeparatorFormula {
    pub fn to_jg_string_with_signal_mapping(
        &self,
        signal_mapping: &std::collections::HashMap<u64, Arc<str>>,
    ) -> String {
        match self {
            SeparatorFormula::InvariantDisjunction(inv_disj) => {
                inv_disj.to_jg_string_with_signal_mapping(signal_mapping)
            }
            SeparatorFormula::Invariant(inv) => {
                inv.to_jg_string_with_signal_mapping(signal_mapping)
            }
        }
    }

    pub fn get_relevant_signal_idx(&self) -> HashSet<u64> {
        let mut signal_idx_set = HashSet::new();
        match self {
            SeparatorFormula::InvariantDisjunction(inv_disj) => {
                for inv in &inv_disj.disjunctions {
                    let inv_signal_idx = inv.get_signal_idx();
                    signal_idx_set.extend(inv_signal_idx);
                }
            }
            SeparatorFormula::Invariant(inv) => {
                let inv_signal_idx = inv.get_signal_idx();
                signal_idx_set.extend(inv_signal_idx);
            }
        }
        signal_idx_set
    }

    pub fn get_relevant_signals(&self) -> HashSet<Arc<str>> {
        let mut signals = HashSet::new();
        match self {
            SeparatorFormula::InvariantDisjunction(inv_disj) => {
                for inv in &inv_disj.disjunctions {
                    let inv_signals = inv.get_signal_names();
                    signals.extend(inv_signals);
                }
            }
            SeparatorFormula::Invariant(inv) => {
                let inv_signals = inv.get_signal_names();
                signals.extend(inv_signals);
            }
        }
        signals
    }

    pub fn get_all_predicates(&self) -> Vec<BasePredicate> {
        let mut predicates = Vec::new();
        match self {
            SeparatorFormula::InvariantDisjunction(inv_disj) => {
                for inv in &inv_disj.disjunctions {
                    predicates.extend(inv.predicate_set.predicates.clone());
                }
            }
            SeparatorFormula::Invariant(inv) => {
                predicates.extend(inv.predicate_set.predicates.clone());
            }
        }
        predicates
    }
}

impl fmt::Display for InvariantDisjunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let disjunctions_str = self
            .disjunctions
            .iter()
            .map(|inv| format!("({})", inv.to_string()))
            .collect::<Vec<String>>()
            .join(" || ");
        write!(f, "{}", disjunctions_str)
    }
}

impl hash::Hash for InvariantDisjunction {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        // Only hash field1
        for inv in &self.disjunctions {
            inv.hash(state);
        }
    }
}

impl InvariantDisjunction {
    pub fn new() -> Self {
        InvariantDisjunction {
            disjunctions: Vec::new(),
        }
    }

    pub fn add_invariant(&mut self, invariant: Invariant) {
        self.disjunctions.push(invariant);
    }

    pub fn to_jg_string_with_signal_mapping(
        &self,
        signal_mapping: &std::collections::HashMap<u64, Arc<str>>,
    ) -> String {
        let disjunctions_str = self
            .disjunctions
            .iter()
            .map(|inv| format!("({})", inv.to_jg_string_with_signal_mapping(signal_mapping)))
            .collect::<Vec<String>>()
            .join(" || ");
        disjunctions_str
    }
}

impl hash::Hash for Invariant {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        // Only hash field1
        self.predicate_set.hash(state);
    }
}

impl fmt::Display for Invariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        /*let predicates_str = self.predicate_set.predicates
            .iter()
            .map(|p| {
                let res = p.to_string(None);
                res
            })
            .collect::<Vec<String>>()
            .join(" && ");
        */
        let predicates_str = self
            .predicate_set
            .predicates
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join(" && ");
        write!(f, "{}", predicates_str)
    }
}

impl Invariant {
    pub fn to_jg_string_with_signal_mapping(
        &self,
        signal_mapping: &std::collections::HashMap<u64, Arc<str>>,
    ) -> String {
        let predicates_str = self
            .predicate_set
            .predicates
            .iter()
            .map(|p| p.to_jg_string_with_signal_mapping(signal_mapping))
            .collect::<Vec<String>>()
            .join(" && ");
        predicates_str
    }
}

impl Invariant {
    pub fn new() -> Self {
        Invariant {
            predicate_set: PredicateSet::new(),
        }
    }

    pub fn merge_invariant(&self, other: &Invariant) -> Self {
        let mut new_invariant = self.clone();
        new_invariant
            .predicate_set
            .predicates
            .extend(other.predicate_set.predicates.clone());
        let mut seen = std::collections::HashSet::new();
        for predicate in &new_invariant.predicate_set.predicates {
            if !seen.insert(predicate) {
                panic!(
                    "Duplicate predicate found: {:?} self {} other {}",
                    predicate, self, other
                );
            }
        }
        //new_invariant.predicate_set.predicates.sort();
        //new_invariant.predicate_set.predicates.dedup();
        new_invariant
    }

    pub fn add_predicate(&mut self, predicate: BasePredicate) {
        self.predicate_set.add_predicate(predicate);
    }

    pub fn contains_predicate(&self, other_predicate: &BasePredicate) -> bool {
        self.predicate_set
            .predicates
            .iter()
            .any(|p| p == other_predicate)
    }

    pub fn should_add_other_invariant(&self, other: &Invariant) -> bool {
        //Return true if the invariant is not already in the invariant
        for p in other.predicate_set.predicates.iter() {
            if self.should_add_predicate(p) == false {
                return false;
            }
        }
        return true;
    }

    pub fn get_signal_names(&self) -> HashSet<Arc<str>> {
        let mut signals = HashSet::new();
        for predicate in self.predicate_set.predicates.iter() {
            match &predicate.base_formula {
                BaseFormula::SignalToConst(formula) => {
                    signals.insert(formula.signal_name.clone());
                }
                BaseFormula::ValueNotIn(formula) | BaseFormula::ValueIn(formula) => {
                    signals.insert(formula.signal_name.clone());
                }
                BaseFormula::TwoSignalEqual(formula) => {
                    signals.insert(formula.signal_name1.clone());
                    signals.insert(formula.signal_name2.clone());
                }
            }
        }
        signals
    }

    pub fn get_signal_idx(&self) -> HashSet<u64> {
        let mut signals = HashSet::new();
        for predicate in self.predicate_set.predicates.iter() {
            match &predicate.base_formula {
                BaseFormula::SignalToConst(formula) => {
                    if let Some(idx) = formula.signal_idx {
                        signals.insert(idx);
                    }
                }
                BaseFormula::ValueNotIn(formula) | BaseFormula::ValueIn(formula) => {
                    if let Some(idx) = formula.signal_idx {
                        signals.insert(idx);
                    }
                }
                BaseFormula::TwoSignalEqual(formula) => {
                    if let Some(idx1) = formula.signal_idx1 {
                        signals.insert(idx1);
                    }
                    if let Some(idx2) = formula.signal_idx2 {
                        signals.insert(idx2);
                    }
                }
            }
        }
        signals
    }

    pub fn should_add_predicate(&self, other_predicate: &BasePredicate) -> bool {
        //Return true if the predicate is not already in the invariant
        //if self.contains_predicate(other_predicate) {//{} && (predicate.operator != Operator::Greater) && (predicate.operator != Operator::Smaller) {
        //    return false;
        // }
        let mut counter_other_not_equal = 0;
        for p in self.predicate_set.predicates.iter() {
            if p == other_predicate {
                //We should never add the exact same predicate
                return false;
            }
            match p.base_formula {
                BaseFormula::TwoSignalEqual(_) => {
                    if p.get_signal_idx() == other_predicate.get_signal_idx()
                        || p.get_signal_names() == other_predicate.get_signal_names()
                    {
                        return false;
                    }
                }
                BaseFormula::SignalToConst(_)
                | BaseFormula::ValueNotIn(_)
                | BaseFormula::ValueIn(_) => {
                    if p.get_operator() == Operator::NotEqual {
                        counter_other_not_equal += 1;
                    }
                    //Implement the following rules: Only limited number of !=
                    /*
                    let identifier_different = p.get_signal_idx() != other_predicate.get_signal_idx();
                    let identifier_same = !(identifier_different);
                    if other_predicate.get_operator() == Operator::Greater || other_predicate.get_operator() == Operator::Smaller {
                        if p.get_operator() == other_predicate.get_operator() {
                            return false;
                        }
                        if p.get_operator() == Operator::Smaller || p.get_operator() == Operator::Greater {
                            if identifier_different {
                                return false;
                            }
                        }
                    } else {
                        if identifier_same {
                            return false;
                        }
                    }
                     */
                    let predicate_limits = constants::PREDICATE_TAKE_LIMITS.load(Ordering::Relaxed);
                    if other_predicate.get_operator() == Operator::NotEqual
                        && counter_other_not_equal >= predicate_limits.not_equal_predicates
                    {
                        return false; //At most the configured number of not-equal operators
                    }
                }
            }
        }
        return true;
    }

    pub fn get_relevant_signals(&self) -> Vec<Arc<str>> {
        self.predicate_set
            .predicates
            .iter()
            .map(|p| p.base_formula.get_signal_names())
            .flatten()
            .collect::<Vec<Arc<str>>>()
    }

    pub fn get_relevant_signal_idx(&self) -> Vec<u64> {
        self.predicate_set
            .predicates
            .iter()
            .map(|p| p.get_signal_idx())
            .flatten()
            .collect::<Vec<u64>>()
    }

    pub fn subset_of(&self, other: &Invariant) -> bool {
        // Return true if all predicates in self are also in other
        self.predicate_set
            .predicates
            .iter()
            .all(|p: &BasePredicate| other.contains_predicate(p))
    }

    pub fn superset_of(&self, other: &Invariant) -> bool {
        // Return true if all predicates in other are also in self
        other.subset_of(self)
    }

    pub fn num_predicates(&self) -> usize {
        self.predicate_set.predicates.len()
    }
}

/*
How should we score invariants? If we score "projected onto the relevant signals only",
that is not going to work: There will be no CEX state that, projected onto the "opcode" signal,
does not also occur in a BEX state, for example.
Then, let's score according to: Try to find the assignment that allows as many CEX as possible.
If that is below 70%, adding a predicate won't help.
How else can cut the search tree?
We could: Think in terms of predicates with concrete assignments - that would help us cut the search space better,
but it would also make the search space much larger. I also don't know how to do that, although I could do a "predicate mining"
approach -- e.g., the predicates are all possible values for signals in each state. Then, could sieve out predicates very fast...
But how many predicates would that get us? And actually, would it not mean that the same invariant candidates are not filtered out?

The cex and bex score query takes too long, that is clear - and I don't know how to speed that up.

Another observation: What if invariant A is a subset of invariant B?
Let's say \phi = A and B and C, and \psi = A and B.
Let's say I know that \phi did not fulfill enough CEX - it might still be worth to try \psi (why? Taking away a predicate can increase number of fulfilled CEX)
Let's say I know that !(\phi) did not fulfill all BEX - That means, !\phi = !A or !B or !C did not fulfill some BEX. But then it cannot be the case
that !\psi = !A or !B fulfills all BEX.

What about the reverse? Let's say I know that \psi did not fulfill enough CEX - then adding C should not help right?
Let's assume \psi fulfills a 100% of the CEX (opcode=csr), but \phi (rd!=0) only fulfills 30%. Then adding (rd!=0) will not help..
Actually I can only combine invariants that itself get over 70%, right?

Okay, so I need a datastructure that for maps each formula to it's score -- and that provides fast "subset lookup".

I also noticed that a lot of the times I "double score" formulas. So I should have a global, multi-threaded lookup table:
Invariants to scores that I update everytime I compute the score of an invariant.
Then, before scoring an invariant, check that:
-> The invariant is not in that table
-> If a superset of that invariant with cex_with_bex_block = None is in that table, then abort..
// If a subset of the invariant is in the table with cex_count < 70% is in that table: Then I should actually also abort..
//
*/
#[derive(
    Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, hash::Hash,
)]
pub struct PriorityScores {
    pub cex_only_score: ScoreResult,
    pub cex_with_bex_allow_score: ScoreResult,
    pub cex_and_bex_score: ScoreResult,
}

#[derive(Debug, Default, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub struct InvariantObjective {
    pub objective: f64,
}

impl PartialOrd for InvariantObjective {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.objective.partial_cmp(&other.objective).unwrap())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct CoverInformation {
    pub covered_states: data_types::general_data_types::BitSetWrapper,
    pub blocked_cex_states: data_types::general_data_types::BitSetWrapper,
    pub allowed_bex_states: data_types::general_data_types::BitSetWrapper,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ScoreAndFulfilledExample {
    pub score: PriorityScores,
    pub cover_info: CoverInformation,
}

// impl Ord for ScoreAndFulfilledExample {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         self.score.cmp(&other.score)
//     }
// }

impl PartialOrd for ScoreAndFulfilledExample {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.score.cmp(&other.score))
    }
}

#[derive(
    Debug, Default, PartialEq, serde::Serialize, serde::Deserialize, Eq, Clone, hash::Hash, Copy,
)]
pub enum ScoreResult {
    #[default]
    NotEvaluated,
    Unsat,
    Sat(i64),
}
impl Ord for ScoreResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (ScoreResult::NotEvaluated, ScoreResult::NotEvaluated) => std::cmp::Ordering::Equal,
            (ScoreResult::NotEvaluated, _) => std::cmp::Ordering::Less,
            (_, ScoreResult::NotEvaluated) => std::cmp::Ordering::Greater,
            (ScoreResult::Unsat, ScoreResult::Unsat) => std::cmp::Ordering::Equal,
            (ScoreResult::Unsat, _) => std::cmp::Ordering::Less,
            (_, ScoreResult::Unsat) => std::cmp::Ordering::Greater,
            (ScoreResult::Sat(lhs), ScoreResult::Sat(rhs)) => lhs.cmp(rhs),
        }
    }
}

impl PartialOrd for ScoreResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl ScoreResult {
    pub fn get_inner_or_zero(&self) -> i64 {
        match self {
            ScoreResult::Sat(i) => *i,
            _ => 0,
        }
    }
    pub fn add_if_sat_or_ignore(&mut self, score_to_add: i64) {
        match self {
            ScoreResult::Sat(i) => {
                *i += score_to_add;
            }
            ScoreResult::NotEvaluated => {
                panic!("Should not add to non evaluated formula!")
            }
            _ => {}
        }
    }
}

impl fmt::Display for PriorityScores {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cex_only_score: {:?}, cex_with_bex_allow_score: {:?}, cex_and_bex_score: {:?}",
            self.cex_only_score, self.cex_with_bex_allow_score, self.cex_and_bex_score
        )
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct InvariantWithScoreAndObjective {
    pub invariant: Invariant,
    pub score: ScoreAndFulfilledExample,
    pub objective: InvariantObjective,
}

impl InvariantWithScoreAndObjective {
    pub fn get_scored_invariant(&self) -> ScoredInvariantWithFulfilledExample {
        ScoredInvariantWithFulfilledExample {
            invariant: self.invariant.clone(),
            score: self.score.clone(),
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ScoredInvariantWithFulfilledExample {
    pub invariant: Invariant,
    pub score: ScoreAndFulfilledExample,
}

// impl Ord for ScoredInvariantWithFulfilledExample {
//     /*fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         let lhs = self.score.cex_and_bex_score.unwrap_or_default();
//         let rhs = other.score.cex_and_bex_score.unwrap_or_default();
//         let cmp = lhs.cmp(&rhs);
//         if cmp == std::cmp::Ordering::Equal {
//             let lhs_rest = self.score.cex_only_score.unwrap() + self.score.cex_with_bex_block_score.unwrap_or_default();
//             let rhs_rest = other.score.cex_only_score.unwrap() + other.score.cex_with_bex_block_score.unwrap_or_default();
//             lhs_rest.cmp(&rhs_rest)
//         } else {
//             cmp
//         }
//     }*/
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {

//         let cex_and_bex_score_cmp = || {
//             let lhs = self.score.score.cex_and_bex_score.get_inner_or_zero() / ((self.invariant.num_predicates()) as i64);
//             let rhs = other.score.score.cex_and_bex_score.get_inner_or_zero() / ((other.invariant.num_predicates()) as i64);

//             lhs.cmp(&rhs)
//         };

//         let cex_only_score_cmp = || {
//             let lhs = self.score.score.cex_only_score.get_inner_or_zero() / ((self.invariant.num_predicates()) as i64);
//             let rhs = other.score.score.cex_only_score.get_inner_or_zero() / ((other.invariant.num_predicates()) as i64);
//             //println!("lhs: {:?}, rhs: {:?}", self.score.cex_only_score, other.score.cex_only_score);
//             //println!("Will return {:?}, lhs {:?}, rhs {:?}", lhs.cmp(&rhs), lhs, rhs);
//             lhs.cmp(&rhs)
//         };

//         let num_predicates_cmp = || {
//             let lhs = self.invariant.num_predicates();
//             let rhs = other.invariant.num_predicates();
//             lhs.cmp(&rhs).reverse()
//         };

//         let cex_with_bex_block_score_cmp = || {
//             let lhs = self.score.score.cex_with_bex_allow_score.get_inner_or_zero(); // (self.invariant.num_predicates() as i64);
//             let rhs = other.score.score.cex_with_bex_allow_score.get_inner_or_zero();
//             lhs.cmp(&rhs)
//         };

//         /*
//         let signal_length_sum_cmp = || {
//             let lhs = self.invariant.predicate_set.predicates.iter().map(|p| p.base_formula.signal_length).sum::<u32>();
//             let rhs = other.invariant.predicate_set.predicates.iter().map(|p| p.).sum::<u32>();
//             lhs.cmp(&rhs).reverse()
//         };
//          */
//         let in_between_operator_cmp = || {
//             let lhs = self.invariant.predicate_set.predicates.iter().any(|p| p.get_operator() == Operator::GreaterEqual || p.get_operator() == Operator::SmallerEqual);
//             let rhs = other.invariant.predicate_set.predicates.iter().any(|p| p.get_operator() == Operator::GreaterEqual || p.get_operator() == Operator::SmallerEqual);
//             lhs.cmp(&rhs).reverse()
//         };

//         let comparisons: Vec<Box<dyn Fn() -> std::cmp::Ordering>> = vec![
//             Box::new(cex_with_bex_block_score_cmp),
//             Box::new(cex_and_bex_score_cmp),
//             Box::new(cex_only_score_cmp),
//             Box::new(num_predicates_cmp),
//             Box::new(in_between_operator_cmp),

//         ];

//         for comparison in comparisons {
//             let result = comparison();
//             //println!("Result: {:?}", result);
//             if result != std::cmp::Ordering::Equal {
//                 //println!("Returning {:?}", result);
//                 return result;
//             }
//         }

//         std::cmp::Ordering::Equal
//     }
// }

// impl PartialOrd for ScoredInvariantWithFulfilledExample {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         Some(self.cmp(other))
//     }
// }

impl fmt::Display for ScoredInvariantWithFulfilledExample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invariant: {} score: {:?}",
            self.invariant, self.score.score
        )
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct ScoredInvariant {
    pub invariant: Invariant,
    pub score: PriorityScores,
}

impl fmt::Display for ScoredInvariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invariant: {} score: {}", self.invariant, self.score)
    }
}

impl Ord for PriorityScores {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let lhs_rest = self.cex_with_bex_allow_score.get_inner_or_zero();
        let rhs_rest = other.cex_with_bex_allow_score.get_inner_or_zero();
        let cmp = lhs_rest.cmp(&rhs_rest);
        if cmp == std::cmp::Ordering::Equal {
            let lhs = self.cex_and_bex_score.get_inner_or_zero();
            let rhs = other.cex_and_bex_score.get_inner_or_zero();
            lhs.cmp(&rhs)
        } else {
            cmp
        }
    }
}

impl PartialOrd for PriorityScores {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoredInvariant {
    /*fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let lhs = self.score.cex_and_bex_score.unwrap_or_default();
        let rhs = other.score.cex_and_bex_score.unwrap_or_default();
        let cmp = lhs.cmp(&rhs);
        if cmp == std::cmp::Ordering::Equal {
            let lhs_rest = self.score.cex_only_score.unwrap() + self.score.cex_with_bex_block_score.unwrap_or_default();
            let rhs_rest = other.score.cex_only_score.unwrap() + other.score.cex_with_bex_block_score.unwrap_or_default();
            lhs_rest.cmp(&rhs_rest)
        } else {
            cmp
        }
    }*/
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let cex_and_bex_score_cmp = || {
            let lhs = self.score.cex_and_bex_score.get_inner_or_zero()
                / ((self.invariant.num_predicates()) as i64);
            let rhs = other.score.cex_and_bex_score.get_inner_or_zero()
                / ((other.invariant.num_predicates()) as i64);

            lhs.cmp(&rhs)
        };

        let cex_only_score_cmp = || {
            let lhs = self.score.cex_only_score.get_inner_or_zero()
                / ((self.invariant.num_predicates()) as i64);
            let rhs = other.score.cex_only_score.get_inner_or_zero()
                / ((other.invariant.num_predicates()) as i64);
            //println!("lhs: {:?}, rhs: {:?}", self.score.cex_only_score, other.score.cex_only_score);
            //println!("Will return {:?}, lhs {:?}, rhs {:?}", lhs.cmp(&rhs), lhs, rhs);
            lhs.cmp(&rhs)
        };

        let num_predicates_cmp = || {
            let lhs = self.invariant.num_predicates();
            let rhs = other.invariant.num_predicates();
            lhs.cmp(&rhs).reverse()
        };

        let cex_with_bex_block_score_cmp = || {
            let lhs = self.score.cex_with_bex_allow_score.get_inner_or_zero(); // (self.invariant.num_predicates() as i64);
            let rhs = other.score.cex_with_bex_allow_score.get_inner_or_zero();
            lhs.cmp(&rhs)
        };

        /*
        let signal_length_sum_cmp = || {
            let lhs = self.invariant.predicate_set.predicates.iter().map(|p| p.base_formula.signal_length).sum::<u32>();
            let rhs = other.invariant.predicate_set.predicates.iter().map(|p| p.base_formula.signal_length).sum::<u32>();
            lhs.cmp(&rhs).reverse()
        };
         */

        let in_between_operator_cmp = || {
            let lhs = self.invariant.predicate_set.predicates.iter().any(|p| {
                p.get_operator() == Operator::GreaterEqual
                    || p.get_operator() == Operator::SmallerEqual
            });
            let rhs = other.invariant.predicate_set.predicates.iter().any(|p| {
                p.get_operator() == Operator::GreaterEqual
                    || p.get_operator() == Operator::SmallerEqual
            });
            lhs.cmp(&rhs).reverse()
        };

        let comparisons: Vec<Box<dyn Fn() -> std::cmp::Ordering>> = vec![
            Box::new(cex_with_bex_block_score_cmp),
            Box::new(cex_and_bex_score_cmp),
            Box::new(cex_only_score_cmp),
            Box::new(num_predicates_cmp),
            Box::new(in_between_operator_cmp),
        ];

        for comparison in comparisons {
            let result = comparison();
            //println!("Result: {:?}", result);
            if result != std::cmp::Ordering::Equal {
                //println!("Returning {:?}", result);
                return result;
            }
        }

        std::cmp::Ordering::Equal
    }
}

impl PartialOrd for ScoredInvariant {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Invariant {
    fn eq(&self, other: &Self) -> bool {
        if self.predicate_set.predicates.len() != other.predicate_set.predicates.len() {
            return false;
        }
        self.predicate_set
            .predicates
            .iter()
            .all(|p| other.predicate_set.predicates.contains(p))
    }
}

impl Eq for Invariant {}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    pub fn test_should_add_predicate() {
        let mut inv = Invariant::new();
        let pred1 = BasePredicate::new_from_value(
            Arc::from("sig1".to_string()),
            Operator::GreaterEqual,
            8,
            5,
            Some(5),
        );
        let pred2 = BasePredicate::new_from_value(
            Arc::from("sig1".to_string()),
            Operator::GreaterEqual,
            8,
            5,
            Some(6),
        );
        let pred3 = BasePredicate::new_from_value(
            Arc::from("sig1".to_string()),
            Operator::GreaterEqual,
            8,
            6,
            Some(6),
        );
        // let pred3 = BasePredicate::new_from_value(Arc::from("sig1".to_string()), Operator::Greater, 8, 5, Some(5));
        //let pred3 = BasePredicate::new_from_value(Arc::from("sig1".to_string()), Operator::Greater, 8, 7, Some(7));
        assert!((pred1 == pred2));

        inv.add_predicate(pred1.clone());
        assert!(!(inv.should_add_predicate(&pred2)));
        assert!(!(inv.should_add_predicate(&pred1)));
        println!(
            "Will add predicate 3 {:?}",
            inv.should_add_predicate(&pred3)
        );
        //assert!(!inv.should_add_predicate(&pred1));
        //assert!(inv.should_add_predicate(&pred3));
    }

    #[test]
    pub fn hashset_deduplication() {
        let mut set = std::collections::HashSet::new();
        let pred1 = BasePredicate::new_from_two_signals(
            Arc::from("sig1".to_string()),
            Arc::from("sig2".to_string()),
            Some(5),
            Some(6),
            Operator::Equal,
        );
        let pred2 = BasePredicate::new_from_two_signals(
            Arc::from("sig2".to_string()),
            Arc::from("sig1".to_string()),
            Some(6),
            Some(5),
            Operator::NotEqual,
        );
        set.insert(pred1.clone());
        set.insert(pred2.clone());
        assert_eq!(set.len(), 2);
        assert_ne!(pred1, pred2);
        /*
        let mut set = std::collections::HashSet::new();
        let mut pred1 = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Equal, 8);
        pred1.base_formula.signal_idx = Some(5);
        let mut pred2 = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Equal, 8);
        pred2.base_formula.signal_idx = Some(6);
        pred1.set_value(5);
        pred2.set_value(5);
        pred2.allowed_values = Some(vec![5, 6, 7]);
        set.insert(pred1.clone());
        set.insert(pred2.clone());
        assert_eq!(set.len(), 1);
        let mut pred3: BasePredicate = BasePredicate::new(Arc::from("sig1_alias".to_string()), Operator::Equal, 8);
        pred3.base_formula.signal_idx = Some(5);
        pred3.set_value(5);
        pred3.allowed_values = Some(vec![5]);
        set.insert(pred3.clone());
        assert_eq!(set.len(), 1);
         */
    }
    #[test]
    pub fn test_should_add() {
        /*
        let mut inv1 = Invariant::new();
        let pred1 = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Greater, 8);
        let pred2 = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Smaller, 8);
        let pred3 = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Equal, 8);
        inv1.add_predicate(pred1.clone());
        assert_eq!(inv1.should_add_predicate(&pred2), true);

        assert_eq!(inv1.should_add_predicate(&pred1), false);
        assert_eq!(inv1.should_add_predicate(&pred3), false);
        let mut inv2 = Invariant::new();
        inv2.add_predicate(pred1.clone());
        inv2.add_predicate(pred2.clone());
        assert_eq!(inv2.should_add_predicate(&pred2), false);
         */
    }

    #[test]
    fn test_is_subset() {
        /*
        let mut base_predicate = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Equal, 8);
        base_predicate.set_value(8);
        let mut inv1 = Invariant::new();
        inv1.add_predicate(base_predicate.clone());
        let mut base_predicate2 = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Equal, 8);
        base_predicate2.clear_value();
        base_predicate2.allowed_values = Some(vec![0,1,2,3,4,5,6,7,8]);
        let mut inv2 = Invariant::new();
        inv2.add_predicate(base_predicate2.clone());
        assert_eq!(inv1.subset_of(&inv2), false);
         */
    }

    #[test]
    fn test_invariant_equality_different_order() {
        /*
        println!("Different orer invariant test");
        let mut inv1 = Invariant::new();
        let mut inv2 = Invariant::new();

        let pred1 = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Equal, 8);
        let pred2 = BasePredicate::new(Arc::from("sig2".to_string()), Operator::NotEqual, 16);

        inv1.add_predicate(pred1.clone());
        inv1.add_predicate(pred2.clone());

        inv2.add_predicate(pred2);
        inv2.add_predicate(pred1);

        assert_eq!(inv1, inv2);
        assert!(&inv1 == &inv2);
         */
    }

    #[test]
    fn test_invariant_score_ordering() {
        /*
        let mut inv1 = Invariant::new();
        let mut inv2 = Invariant::new();

        let pred1 = BasePredicate::new(Arc::from("sig1".to_string()), Operator::Equal, 8);
        let pred2 = BasePredicate::new(Arc::from("sig2".to_string()), Operator::NotEqual, 16);
        let pred3 = BasePredicate::new(Arc::from("sig3".to_string()), Operator::Greater, 8);
        let pred4 = BasePredicate::new(Arc::from("sig4".to_string()), Operator::Smaller, 8);
        let pred5: BasePredicate = BasePredicate::new(Arc::from("sig5".to_string()), Operator::Equal, 8);
        let pred6: BasePredicate = BasePredicate::new(Arc::from("sig6".to_string()), Operator::Equal, 8);
        let pred7: BasePredicate = BasePredicate::new(Arc::from("sig7".to_string()), Operator::Equal, 8);

        inv1.add_predicate(pred1.clone());
        inv1.add_predicate(pred2.clone());

        inv2.add_predicate(pred2);
        inv2.add_predicate(pred1.clone());

        let scored_inv1 = ScoredInvariant{invariant: inv1.clone(), score: PriorityScores{cex_only_score: ScoreResult::Sat(100), cex_with_bex_allow_score: ScoreResult::Unsat, cex_and_bex_score: ScoreResult::NotEvaluated}};
        let scored_inv2 = ScoredInvariant{invariant: inv2.clone(), score: PriorityScores{cex_only_score: ScoreResult::Sat(102), cex_with_bex_allow_score: ScoreResult::Unsat, cex_and_bex_score: ScoreResult::NotEvaluated}};
        //println!("Scored inv1: {}", scored_inv1 > scored_inv2);
        let mut scored_inv3 = Invariant::new();
        scored_inv3.add_predicate(pred1.clone());
        let scored_inv3 = ScoredInvariant{invariant: scored_inv3, score: PriorityScores{cex_only_score: ScoreResult::Sat(100), cex_with_bex_allow_score: ScoreResult::Unsat, cex_and_bex_score: ScoreResult::NotEvaluated}};
        assert!(scored_inv1 < scored_inv2);
        assert!(scored_inv3 > scored_inv2);
        let scored_inv4 = ScoredInvariant{invariant: inv1.clone(), score: PriorityScores{cex_only_score: ScoreResult::Sat(1), cex_with_bex_allow_score: ScoreResult::Sat(1), cex_and_bex_score: ScoreResult::NotEvaluated}};
        assert!(scored_inv4 < scored_inv3);

        inv1.add_predicate(pred3.clone());
        inv1.add_predicate(pred4.clone());
        inv1.add_predicate(pred5.clone());
        inv1.add_predicate(pred6.clone());
        inv1.add_predicate(pred7.clone());

        inv2.add_predicate(pred3.clone());
        inv2.add_predicate(pred4.clone());
        inv2.add_predicate(pred5.clone());
        inv2.add_predicate(pred6.clone());

        let scored_inv1 = ScoredInvariant{invariant: inv1.clone(), score: PriorityScores{cex_only_score: ScoreResult::Sat(137), cex_with_bex_allow_score: ScoreResult::NotEvaluated, cex_and_bex_score: ScoreResult::Sat(1080)}};
        let scored_inv2 = ScoredInvariant{invariant: inv2.clone(), score: PriorityScores{cex_only_score: ScoreResult::Sat(137), cex_with_bex_allow_score: ScoreResult::NotEvaluated, cex_and_bex_score: ScoreResult::Sat(1075)}};
        assert!(scored_inv2 > scored_inv1);
         */
    }
}
