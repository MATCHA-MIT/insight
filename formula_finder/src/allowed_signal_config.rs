use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use crate::data_types::general_data_types::{self, SignalInfo};
use crate::data_types;


/// Filter closure that takes (signal_idx, value, signal_info) and returns whether to include this value
pub type SignalValueFilter = Arc<dyn Fn(u64, i64, &SignalInfo) -> bool + Send + Sync>;

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub enum PredicateType {
    Equal,
    NotEqual,
    GreaterEqual,
    SmallerEqual,
    TwoSignalEqual,
}

#[derive(Clone)]
pub struct SignalTypePredicateConfig {
    pub enabled_predicates: HashSet<PredicateType>,
    /// Per-predicate value filters: map from predicate type to filter function
    pub value_filters: HashMap<PredicateType, SignalValueFilter>,
}

impl SignalTypePredicateConfig {
    pub fn new() -> Self {
        Self {
            enabled_predicates: HashSet::new(),
            value_filters: HashMap::new(),
        }
    }

    pub fn with_predicates(predicates: Vec<PredicateType>) -> Self {
        Self {
            enabled_predicates: predicates.into_iter().collect(),
            value_filters: HashMap::new(),
        }
    }

    /// Set a value filter for a specific predicate type
    pub fn with_value_filter_for_predicate(
        mut self,
        pred_type: PredicateType,
        filter: SignalValueFilter,
    ) -> Self {
        self.enabled_predicates.insert(pred_type.clone());
        self.value_filters.insert(pred_type, filter);
        self
    }

    /// Set the same value filter for multiple predicate types
    pub fn with_value_filter_for_predicates(
        mut self,
        pred_types: Vec<PredicateType>,
        filter: SignalValueFilter,
    ) -> Self {
        let filter = Arc::clone(&filter);
        for pred_type in pred_types {
            self.enabled_predicates.insert(pred_type.clone());
            self.value_filters.insert(pred_type, Arc::clone(&filter));
        }
        self
    }

    pub fn allows_predicate(&self, pred_type: &PredicateType) -> bool {
        self.enabled_predicates.contains(pred_type)
    }

    pub fn allows_value(
        &self,
        pred_type: &PredicateType,
        signal_idx: u64,
        value: i64,
        signal_info: &SignalInfo,
    ) -> bool {
        if !self.allows_predicate(pred_type) {
            return false;
        }
        match self.value_filters.get(pred_type) {
            Some(filter) => filter(signal_idx, value, signal_info),
            None => true,
        }
    }
}

#[derive(Clone)]
pub struct PredicateGenerationConfig {
    pub signal_type_configs: HashMap<data_types::general_data_types::SignalType, SignalTypePredicateConfig>,
    pub track_cycle_info: bool,
}

impl PredicateGenerationConfig {
    pub fn new() -> Self {
        Self {
            signal_type_configs: HashMap::new(),
            track_cycle_info: true,
        }
    }

    pub fn allows_types(&self, types: &data_types::general_data_types::SignalTypesSet) -> bool {
        for signal_type in types.iter() {
            if self.allows_type(signal_type) {
                return true;
            }
        }
        false
    }

    pub fn allows_type(
        &self,
        signal_type: &data_types::general_data_types::SignalType,
    ) -> bool {
        self.signal_type_configs.contains_key(signal_type)
    }

    pub fn set_config_for_type(
        &mut self,
        signal_type: data_types::general_data_types::SignalType,
        config: SignalTypePredicateConfig,
    ) {
        self.signal_type_configs.insert(signal_type, config);
    }

    pub fn allows_predicate_type_set(
        &self,
        types: &data_types::general_data_types::SignalTypesSet,
        pred_type: &PredicateType,
    ) -> bool {
        for signal_type in types.iter() {
            if self.allows_predicate(signal_type, pred_type) {
                return true;
            }
        }
        false
    }

    pub fn allows_predicate(
        &self,
        signal_type: &data_types::general_data_types::SignalType,
        pred_type: &PredicateType,
    ) -> bool {
        self.signal_type_configs
            .get(signal_type)
            .map(|c| c.allows_predicate(pred_type))
            .unwrap_or(false)
    }

    pub fn allows_value_type_set(
        &self,
        types: &data_types::general_data_types::SignalTypesSet,
        pred_type: &PredicateType,
        signal_idx: u64,
        value: i64,
        signal_info: &SignalInfo,
    ) -> bool {
        for signal_type in types.iter() {
            if self.allows_value(signal_type, pred_type, signal_idx, value, signal_info) {
                return true;
            }
        }
        false
    }

    pub fn allows_value(
        &self,
        signal_type: &data_types::general_data_types::SignalType,
        pred_type: &PredicateType,
        signal_idx: u64,
        value: i64,
        signal_info: &SignalInfo,
    ) -> bool {
        self.signal_type_configs
            .get(signal_type)
            .map(|c| c.allows_value(pred_type, signal_idx, value, signal_info))
            .unwrap_or(false)
    }
}

pub fn get_sodor_predicate_generation_config() -> PredicateGenerationConfig {
    let mut config = PredicateGenerationConfig::new();
    let equality_non_equality_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::Equal,
            Arc::new(|_sig_idx, _value, _sig_info| true)
        )
        // For NotEqual predicates, allow all values
        .with_value_filter_for_predicate(
            PredicateType::NotEqual,
            Arc::new(|_sig_idx, _value, _sig_info| _sig_info.length > 1)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::Control, equality_non_equality_config.clone());
    let signal_equal_inequality_only_non_zero_config = equality_non_equality_config.clone()
        .with_value_filter_for_predicate(
            PredicateType::TwoSignalEqual,
            Arc::new(|_sig_idx, _value, _sig_info| true)
        )
        .with_value_filter_for_predicate(
            PredicateType::NotEqual,
            Arc::new(|_sig_idx, value, _sig_info| value == 0)
        )
        .with_value_filter_for_predicate(
            PredicateType::Equal,
            Arc::new(|_sig_idx, _value, _sig_info| true)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::RegisterFileAddress, signal_equal_inequality_only_non_zero_config.clone());
        // .with_value_filter_for_predicate(PredicateType::SignalEqual, Arc::new(|_sig_idx, _value, _sig_info| true));
    let signal_equal_inequality_only_non_zero_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::TwoSignalEqual,
            Arc::new(|_sig_idx, _value, _sig_info| true)
        )
        .with_value_filter_for_predicate(
            PredicateType::NotEqual,
            Arc::new(|_sig_idx, value, _sig_info| value == 0)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::Register, signal_equal_inequality_only_non_zero_config);
    let signal_equal_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::TwoSignalEqual,
            Arc::new(|_sig_idx, _value, _sig_info|true)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::Immediate, signal_equal_config.clone());
    // config.set_config_for_type(data_types::general_data_types::SignalType::Unknown, signal_equal_config.clone());
    config
}

pub fn get_kronos_predicate_generation_config() -> PredicateGenerationConfig {
    let mut config = PredicateGenerationConfig::new();
    let equality_non_equality_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::Equal,
            Arc::new(|_sig_idx, _value, _sig_info| true)
        )
        // For NotEqual predicates, allow all values
        .with_value_filter_for_predicate(
            PredicateType::NotEqual,
            Arc::new(|_sig_idx, _value, _sig_info| _sig_info.length > 1)
        );
        // .with_value_filter_for_predicate(
        //     PredicateType::GreaterEqual,
        //     Arc::new(|_sig_idx, _value, _sig_info|  _sig_info.length > 1)
        // )
        // .with_value_filter_for_predicate(
        //     PredicateType::SmallerEqual,
        //     Arc::new(|_sig_idx, _value, _sig_info|  _sig_info.length > 1)
        // );
    config.set_config_for_type(data_types::general_data_types::SignalType::Control, equality_non_equality_config);
    let signal_equal_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::TwoSignalEqual,
            Arc::new(|_sig_idx, value, sig_info| value != 0 && sig_info.length > 1)
        )
        .with_value_filter_for_predicate(
            PredicateType::NotEqual,
            Arc::new(|_sig_idx, value, _sig_info| value == 0)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::Register, signal_equal_config);
    let register_file_address_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::TwoSignalEqual,
            Arc::new(|_sig_idx, value, sig_info| value != 0 && sig_info.length > 1)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::Immediate, register_file_address_config.clone());
    config.set_config_for_type(data_types::general_data_types::SignalType::Funct7, register_file_address_config.clone());
    // config.set_config_for_type(data_types::general_data_types::SignalType::RegisterFileAddress, equality_non_equality_config.clone());
    config
}

pub fn get_boom_predicate_generation_config() -> PredicateGenerationConfig {
    // Example 1: Different filters for Equal vs NotEqual predicates on Control signals
    let mut config = PredicateGenerationConfig::new();
    let equality_non_equality_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::Equal,
            Arc::new(|_sig_idx, _value, _sig_info| true)
        )
        // For NotEqual predicates, allow all values
        .with_value_filter_for_predicate(
            PredicateType::NotEqual,
            Arc::new(|_sig_idx, _value, sig_info| sig_info.length > 1)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::Control, equality_non_equality_config.clone());
    let signal_equal_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::TwoSignalEqual,
            Arc::new(|_sig_idx, value, sig_info| value != 0 && sig_info.length > 1)
        )
        .with_value_filter_for_predicate(
            PredicateType::NotEqual,
            Arc::new(|_sig_idx, value, _sig_info| value == 0)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::Register, signal_equal_config);
    let register_file_address_config = SignalTypePredicateConfig::new()
        .with_value_filter_for_predicate(
            PredicateType::TwoSignalEqual,
            Arc::new(|_sig_idx, value, sig_info| value != 0 && sig_info.length > 1)
        );
    config.set_config_for_type(data_types::general_data_types::SignalType::Immediate, register_file_address_config);
    return config;
    // let signal_equal_config = SignalTypePredicateConfig::new()
    //     .with_value_filter_for_predicate(
    //         PredicateType::TwoSignalEqual,
    //         Arc::new(|_sig_idx, value, sig_info|true)
    //     );
    // config.set_config_for_type(data_types::general_data_types::SignalType::Immediate, signal_equal_config.clone());
    // let signal_equal_config = SignalTypePredicateConfig::new()
    //     .with_value_filter_for_predicate(
    //         PredicateType::TwoSignalEqual,
    //         Arc::new(|_sig_idx, value, sig_info|true)
    //     );
    // config.set_config_for_type(data_types::general_data_types::SignalType::Immediate, signal_equal_config.clone());
    // let only_equality_config = SignalTypePredicateConfig::new()
    //     .with_value_filter_for_predicate(
    //         PredicateType::Equal,
    //         Arc::new(|_sig_idx, value, sig_info| sig_info.length > 1)
    //     );
    // config
}

pub fn get_decoder_config_predicate_generation_config() -> PredicateGenerationConfig {
    get_sodor_predicate_generation_config()
}

pub fn get_default_predicate_generation_config() -> PredicateGenerationConfig {
    // Example 1: Different filters for Equal vs NotEqual predicates on Control signals
    return get_sodor_predicate_generation_config();
}

pub fn get_config_from_stage_filter(stage_filter: &general_data_types::StageFilter) -> PredicateGenerationConfig {
    let mut string_count = HashMap::new();
    string_count.insert("sodor", 0);
    string_count.insert("boom", 0);
    string_count.insert("kronos", 0);
    string_count.insert("decoder", 0);
    if stage_filter.stage_name.to_lowercase().contains("decoder") {
        return get_decoder_config_predicate_generation_config();
    }
    let core_name = string_count.keys().cloned().collect::<Vec<&str>>();
    for re_string in stage_filter.include.iter() {
        for key in core_name.iter() {
            if re_string.as_str().to_lowercase().contains(key) {
                *string_count.get_mut(key).unwrap() += 1;
            }
        }
    }
    let mut max_core = "default";
    let mut max_count = 0;
    for (core, count) in string_count.iter() {
        if *count > max_count {
            max_core = core;
            max_count = *count;
        }
    }
    let config = match max_core {
        "sodor" => get_sodor_predicate_generation_config(),
        "boom" => get_boom_predicate_generation_config(),
        "kronos" => get_kronos_predicate_generation_config(),
        _ => get_default_predicate_generation_config(),
    };
    println!("Using predicate generation config for core: {}", max_core);
    config
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_equals_predicate_allowed() {
        let config = get_default_predicate_generation_config();
        let signal_type = data_types::general_data_types::SignalType::Register;
        assert_eq!(config.allows_predicate(&signal_type, &PredicateType::TwoSignalEqual), false);
        assert_eq!(config.allows_predicate(&signal_type, &PredicateType::NotEqual), true);
        assert_eq!(config.allows_predicate(&signal_type, &PredicateType::Equal), false);
        assert_eq!(config.allows_predicate(&data_types::general_data_types::SignalType::Data, &PredicateType::Equal), false);
    }
}
