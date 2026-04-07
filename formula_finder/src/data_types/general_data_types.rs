use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer};
use hibitset::{BitIter, BitSet, BitSetLike};
use rustc_hash;
use std::collections::HashSet;




pub type StateIdType = u64;
pub type DefaultScalarHasher = std::hash::BuildHasherDefault<rustc_hash::FxHasher>;
pub type DefaultVectorHasher = std::hash::BuildHasherDefault<rustc_hash::FxHasher>;
pub type SignalIndexSet =   HashSet<u64, DefaultScalarHasher>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FormulaScoreWeights {
    pub bex_multiplier: usize,
    pub predicate_base_cost: usize,
}

#[derive(Debug, Clone)]
pub struct SignalInfo {
    pub aliases: Vec<ustr::Ustr>,
    pub id: u64,
    pub length: usize,
    pub signal_types: SignalTypesSet,
}

impl SignalInfo {
    pub fn get_signal_name(&self) -> String {
        if !self.aliases.is_empty() {
            self.aliases[0].to_string()
        } else {
            format!("signal_{}", self.id)
        }
    }
    pub fn any_alias_contains(&self, substring: &str, case_sensitive: bool) -> bool {
        if case_sensitive {
            self.aliases.iter().any(|alias| alias.contains(substring))
        } else {
            self.aliases.iter().any(|alias| alias.to_lowercase().contains(&substring.to_lowercase()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreType {
    RefCore,
    DutCore
}
#[derive(Debug, Clone)]
pub struct StageFilter {
    pub include: Vec<regex::Regex>,
    pub exclude: Vec<regex::Regex>,
    pub core_type: CoreType,
    pub stage_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaveFormSource {
    Seed,
    OldBenignExamples,
    Mutations,
    PreDeterminedGenerated,
    Unknown,
    OriginalCex,
    MustFulfill,
    // FromImmediateMutation
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default, Hash)]
pub enum SignalType {
    ClockReset,
    Control,
    Data,
    Funct7,
    Address,
    Immediate,
    RegisterFileAddress,
    Register,
    Counter,
    Instruction,
    #[serde(other)]
    #[default] Unknown,
}


#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct SignalTypesSet {
    pub types: HashSet<SignalType>,
}
impl SignalTypesSet {
    pub fn new() -> Self {
        SignalTypesSet {
            types: HashSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    pub fn insert(&mut self, signal_type: SignalType) {
        self.types.insert(signal_type);
    }

    pub fn contains(&self, signal_type: &SignalType) -> bool {
        self.types.contains(signal_type)
    }

    pub fn new_from_type(signal_type: SignalType) -> Self {
        let mut set = SignalTypesSet::new();
        set.insert(signal_type);
        set
    }

    pub fn with_type(mut self, signal_type: SignalType) -> Self {
        self.insert(signal_type);
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = &SignalType> {
        self.types.iter()
    }

    pub fn contains_any_type(&self, signal_types: &Vec<SignalType>) -> bool {
        signal_types.iter().any(|signal_type| self.types.contains(signal_type))
    }
}


impl PartialOrd for WaveFormSource {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WaveFormSource {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use WaveFormSource::*;
        let rank = |source: &WaveFormSource| match source {
            MustFulfill => 0,
            OriginalCex => 1,
            Mutations => 2,
            PreDeterminedGenerated => 3,
            Seed => 4,
            OldBenignExamples => 5,
            Unknown => 6,
        };
        rank(self).cmp(&rank(other))
    }
}

impl serde::Serialize for WaveFormSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let file_source_str = match self {
            WaveFormSource::Seed => "FileSource.Seed",
            WaveFormSource::OldBenignExamples => "FileSource.OldBenignExamples",
            WaveFormSource::Mutations => "FileSource.Mutations",
            WaveFormSource::PreDeterminedGenerated => "FileSource.PreDeterminedGenerated",
            WaveFormSource::Unknown => "FileSource.Unknown",
            WaveFormSource::OriginalCex => "FileSource.OriginalCex",
            WaveFormSource::MustFulfill => "FileSource.MustFulfill"
        };
        serializer.serialize_str(file_source_str)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FuzzerDataPoint {
    pub file: String,
    pub waveform_path: String,
    pub file_source: WaveFormSource,
    pub program_distance: u32,
}

impl<'de> Deserialize<'de> for FuzzerDataPoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct FuzzerDataPointHelper {
            #[serde(alias = "path")]
            file: String,
            waveform_path: String,
            #[serde(default)]
            file_source: Option<String>,
            #[serde(default)]
            program_distance: u32,
        }

        let helper = FuzzerDataPointHelper::deserialize(deserializer)?;
        let file_source = match helper.file_source.as_deref() {
            Some("FileSource.Seed") => WaveFormSource::Seed,
            Some("FileSource.OldBenignExamples") => WaveFormSource::OldBenignExamples,
            Some("FileSource.Mutations") => WaveFormSource::Mutations,
            Some("FileSource.PreDeterminedGenerated") => WaveFormSource::PreDeterminedGenerated,
            Some("FileSource.OriginalCex") => WaveFormSource::OriginalCex,
            Some("FileSource.MustFulfill") => WaveFormSource::MustFulfill,
            Some("FileSource.Unknown") | None => WaveFormSource::Unknown,
            Some(other) => {
                return Err(de::Error::custom(format!(
                    "Unknown file source: {}",
                    other
                )))
            }
        };

        Ok(FuzzerDataPoint {
            file: helper.file,
            waveform_path: helper.waveform_path,
            file_source,
            program_distance: helper.program_distance,
        })
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct BitSetWrapper {
    bitset: Box<BitSet>,
}

impl IntoIterator for BitSetWrapper {
    type Item = u32;
    type IntoIter = BitIter<BitSet>;

    fn into_iter(self) -> Self::IntoIter {
        self.bitset.iter()
    }
}

impl std::hash::Hash for BitSetWrapper {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for &word in self.bitset.layer0_as_slice() {
            word.hash(state);
        }
    }
}

impl BitSetWrapper {
    // Create a new BitSetWrapper
    pub fn new() -> Self {
        BitSetWrapper {
            bitset: Box::new(BitSet::new()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.bitset.is_empty()
    }

    pub fn from_hashset(hashset: &HashSet<u32>) -> Self {
        BitSetWrapper {
            bitset: Box::new(hashset.iter().cloned().collect()),
        }
    }

    pub fn from_vec(vec: Vec<u32>) -> Self {
        BitSetWrapper {
            bitset: Box::new(vec.into_iter().collect()),
        }
    }

    pub fn len(&self) -> usize {
        let mut total = 0;
        // Access the raw layer-0 representation
        for &word in self.bitset.layer0_as_slice() {
            total += word.count_ones() as usize;
        }
        total
    }

    pub fn count(&self) -> usize {
        self.len()
    }

    pub fn collect(&self) -> Vec<u32> {
        self.bitset.clone().iter().collect()
    }

    // Add an element to the bitset
    pub fn add(&mut self, index: u32) {
        self.bitset.add(index);
    }

    pub fn insert(&mut self, index: u32) {
        self.add(index);
    }

    pub fn insert_all(&mut self, indices: impl IntoIterator<Item = u32>) {
        for index in indices {
            self.add(index);
        }
    }

    pub fn remove_all(&mut self, indices: impl IntoIterator<Item = u32>) {
        for index in indices {
            self.remove(index);
        }
    }

    // Remove an element from the bitset
    // Remove an element from the bitset
    #[allow(dead_code)]
    pub fn remove(&mut self, index: u32) {
        self.bitset.remove(index);
    }

    // Perform a union with another BitSetWrapper and return a new BitSetWrapper
    pub fn union(&self, other: &Self) -> Self {
        BitSetWrapper {
            bitset: Box::new((&*self.bitset | &*other.bitset).iter().collect()),
        }
    }

    // Perform an intersection with another BitSetWrapper and return a new BitSetWrapper
    pub fn intersection(&self, other: &Self) -> Self {
        BitSetWrapper {
            bitset: Box::new((&*self.bitset & &*other.bitset).iter().collect()),
        }
    }

    pub fn contains_any(&self, other_list: &[u32]) -> bool {
        other_list.iter().any(|item| self.bitset.contains(*item))
    }

    // Check if the bitset contains a specific index
    pub fn contains(&self, index: &u32) -> bool {
        self.bitset.contains(*index)
    }

    pub fn is_strict_subset(&self, other: &Self) -> bool {
        other.is_strict_superset(self)
    }

    pub fn is_strict_superset(&self, other: &Self) -> bool {
        //A is strict superset of B if A contains all elements of B and A != B
        self.bitset.contains_set(&other.bitset) &&
        !(other.bitset.contains_set(&self.bitset))
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        self.bitset.contains_set(&other.bitset)
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        other.is_superset(&self)
    }

    pub fn return_first_element(&self) -> Option<u32> {
        self.bitset.clone().iter().next()
    }

    pub fn is_subset_fast(&self, b: &BitSetWrapper) -> bool {
        let a_words = self.bitset.layer0_as_slice();
        let b_words = b.bitset.layer0_as_slice();
        a_words.iter().zip(b_words.iter()).all(|(aw, bw)| aw & !bw == 0)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_len() {
        let mut set = BitSetWrapper::new();
        assert_eq!(set.len(), 0); // Initially, the set is empty

        for i in 1..=100 {
            set.add(i);
        }
        assert_eq!(set.len(), 100); // After adding three elements, the length should be 3

        set.remove(2);
        assert_eq!(set.len(), 99); // After removing one element, the length should be 2
    }


    #[test]
    fn test_serialize_deserialize() {
        let json_data = r#"
        {
            "path": "example.vcd",
            "waveform_path": "example_waveform.vcd",
            "file_source": "FileSource.Seed",
            "program_distance": 5
        }
        "#;

        let data_point: FuzzerDataPoint = serde_json::from_str(json_data).unwrap();
        assert_eq!(data_point.file, "example.vcd");
        assert_eq!(data_point.waveform_path, "example_waveform.vcd");
        assert_eq!(data_point.file_source, WaveFormSource::Seed);
        assert_eq!(data_point.program_distance, 5);

        let serialized = serde_json::to_string(&data_point).unwrap();
        let expected_serialized = r#"{"file":"example.vcd","waveform_path":"example_waveform.vcd","file_source":"FileSource.Seed","program_distance":5}"#;
        assert_eq!(serialized, expected_serialized);
    }

    #[test]
    fn test_iter() {
        let mut set = BitSetWrapper::new();
        set.add(1);
        set.add(2);
        set.add(3);

        let collected: Vec<u32> = set.into_iter().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }
    
    #[test]
    fn test_is_strict_subset() {
        let mut set_a = BitSetWrapper::new();
        let mut set_b = BitSetWrapper::new();

        set_a.add(1);
        set_a.add(2);

        set_b.add(1);
        set_b.add(2);
        set_b.add(3);

        assert!(set_a.is_strict_subset(&set_b));
        assert!(!set_b.is_strict_subset(&set_a));
        assert!(!set_a.is_strict_subset(&set_a)); // A set is not a strict subset of itself
    }

    #[test]
    fn test_is_strict_superset() {
        let mut set_a = BitSetWrapper::new();
        let mut set_b = BitSetWrapper::new();

        set_a.add(1);
        set_a.add(2);
        set_a.add(3);

        set_b.add(1);
        set_b.add(2);

        assert!(set_a.is_strict_superset(&set_b));
        assert!(!set_b.is_strict_superset(&set_a));
        assert!(!set_a.is_strict_superset(&set_a)); // A set is not a strict superset of itself
    }
}
