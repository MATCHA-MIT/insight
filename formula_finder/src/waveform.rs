use std::collections::HashSet;
use std::sync::Arc;
use std::{collections::HashMap, u64};
use std::fs::File;
use std::io::BufReader;
use vcd::{Command, Header, Parser, Scope, ScopeItem, Var};
use std::error::Error;
use rustc_hash::FxHasher;
use crate::{constants, data_types};
use crate::data_types::general_data_types::DefaultScalarHasher;
use crate::cycle_types::{CycleCount, CycleCountConversion};
use ustr;
use wellen;
use fst_reader;
use data_types::signal_filters::{SignalFilter, SignalFilters};


pub fn is_metadata_signal(signal_name: &str, clock_signal: &str) -> bool {
    static METADATA_SIGNAL_NAMES: &[&str] = &[
        constants::COUNTER_SIGNAL,
        constants::INCORRECTNESS_SIGNAL,
        constants::MISMATCH_CYCLE_REF_CORE_SIGNAL,
        constants::MISMATCH_CYCLE_DUT_CORE_SIGNAL,
    ];
    if signal_name == clock_signal {
        return true;
    }
    for meta_signal in METADATA_SIGNAL_NAMES.iter() {
        if signal_name == *meta_signal {
            return true;
        }
    }
    return false;

}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SignalValue {
    Scalar(vcd::Value),
    Vector(vcd::Vector),
    // Real(f64),
    // Str(String),
}

pub type SignalValueType = i64;
pub type CycleMapType = HashMap<u64, Vec<SignalValueType>, DefaultScalarHasher>;


/// Converts a bitstring (represented as a String) to an i64.
///
/// This function processes a string where each character represents a bit.
/// '0' and '1' are converted to their binary equivalents.
/// If any character is 'X', 'Z', or any other non-binary state (like 'H', 'U', 'W', 'L', '-'),
/// the entire value is considered unknown, and the function returns -1.
///
/// # Arguments
/// * `bitstring` - A string slice representing the bitstring.
///
/// # Returns
/// A `Result<i64, Box<dyn Error>>` which is:
/// * `Ok(value)` - The converted i64 value.
/// * `Err(error)` - If the input string contains invalid characters or is empty.
pub fn bitstring_to_i64(bitstring: &str) -> Result<i64, Box<dyn Error>> {
    if bitstring.is_empty() {
        return Err("Input bitstring cannot be empty".into());
    }

    let mut result: i64 = 0;
    let mut contains_unknown = false;

    for char_bit in bitstring.chars() {
        match char_bit {
            '0' => {
                result <<= 1;
                result |= 0;
            }
            '1' => {
                result <<= 1;
                result |= 1;
            }
            'X' | 'Z' | 'H' | 'U' | 'W' | 'L' | '-' => { // [2]
                contains_unknown = true;
                break; // Propagate unknown state for the entire bitstring
            }
            _ => {
                return Err(format!("Invalid character in bitstring: '{}'", char_bit).into());
            }
        }
    }

    if contains_unknown {
        panic!("Bitstring {} contains unknown state, returning -1", bitstring);
    } else {
        Ok(result)
    }
}

/// Return a possibly-trimmed view of `name` with a trailing [..] removed
/// iff it matches the declared `width`. Falls back to the original `name`.
fn trim_trailing_range(name: &str, width: usize) -> &str {
    let bytes = name.as_bytes();
    let mut n = bytes.len();
    if n == 0 {
        return name;
    }
    // trim trailing whitespace
    while n > 0 && bytes[n - 1].is_ascii_whitespace() {
        n -= 1;
    }
    if n == 0 || bytes[n - 1] != b']' {
        return name; // no trailing bracket group
    }
    if n < 2 {
        return name; // too short to contain "[...]"
    }

    // j points to last char *inside* the brackets (skip the ']')
    let mut j = n - 2;

    // scan backward to the matching '['; only allow digits/colon/space inside
    while j > 0 && bytes[j] != b'[' {
        let b = bytes[j];
        if !(b.is_ascii_digit() || b == b':' || b.is_ascii_whitespace()) {
            return name; // not a numeric slice -> keep original
        }
        j -= 1;
    }
    if bytes[j] != b'[' {
        return name; // no matching '['
    }

    let content_start = j + 1;
    let content_end = n - 1; // index of the ']'

    // helper: skip spaces
    let mut p = content_start;
    let skip_ws = |p: &mut usize| {
        while *p < content_end && bytes[*p].is_ascii_whitespace() {
            *p += 1;
        }
    };

    // helper: read unsigned number
    let read_num = |p: &mut usize| -> Option<u64> {
        let mut v: u64 = 0;
        let mut any = false;
        while *p < content_end && bytes[*p].is_ascii_digit() {
            any = true;
            v = v * 10 + (bytes[*p] - b'0') as u64;
            *p += 1;
        }
        if any { Some(v) } else { None }
    };

    skip_ws(&mut p);
    let first = match read_num(&mut p) {
        Some(v) => v,
        None => return name,
    };
    skip_ws(&mut p);

    let matches_width = if p < content_end && bytes[p] == b':' {
        // form: hi:lo
        p += 1;
        skip_ws(&mut p);
        let second = match read_num(&mut p) {
            Some(v) => v,
            None => return name,
        };
        skip_ws(&mut p);
        if p != content_end {
            return name; // trailing junk inside []
        }
        let hi = first;
        let lo = second;
        if hi >= lo {
            let span = (hi - lo + 1) as usize;
            span == width
        } else {
            false
        }
    } else {
        // form: idx (only trim when declared width == 1)
        skip_ws(&mut p);
        if p != content_end {
            return name;
        }
        width == 1
    };

    if !matches_width {
        return name;
    }

    // also trim an optional space before '['
    let mut cut = j;
    while cut > 0 && bytes[cut - 1].is_ascii_whitespace() {
        cut -= 1;
    }
    &name[..cut]
}

struct WaveFormIntermediate {
    timestamp_to_value: HashMap<u64, Vec<SignalValueType>, DefaultScalarHasher>,
    signal_to_value_changes: HashMap<u64, Vec<(u64, SignalValueType)>, DefaultScalarHasher>,
    name_to_code: HashMap<ustr::Ustr, u64, DefaultScalarHasher>,
    signal_length_map: HashMap<u64, usize, DefaultScalarHasher>,
    pub path: ustr::Ustr,
    pub file_source: data_types::general_data_types::WaveFormSource,
}

//    pub timestamp_to_value: HashMap<(u64, u64), SignalValueType>,
//    pub signal_to_value_changes: HashMap<u64, Vec<(u64, SignalValueType)>>,
#[derive(Clone, Debug)]
pub struct WaveForm {
    pub name_to_code: HashMap<ustr::Ustr, u64, DefaultScalarHasher>,
    pub signal_length_map: HashMap<u64, usize, DefaultScalarHasher>,
    cycle_map: CycleMapType,
    pub constant_signals: HashMap<u64, SignalValueType, DefaultScalarHasher>,
    pub num_cycles: CycleCount,
    pub path: ustr::Ustr,
    pub file_source: data_types::general_data_types::WaveFormSource,
}

fn get_timestamp_to_value<H>(
    signal_to_value_changes: &HashMap<u64, Vec<(u64, SignalValueType)>, H>,
    tracked_idx: &HashSet<u64, H>,
    end_time: u64
) -> HashMap<u64, Vec<SignalValueType>, H> 
where H: std::hash::BuildHasher + Default
{
    let mut timestamp_to_value: HashMap<u64, Vec<i64>, H> = HashMap::with_capacity_and_hasher(tracked_idx.len(), H::default());
    for (id_code, changes) in signal_to_value_changes {
        if changes.len() == 1 {
            continue; // Constant signal, no need to fill in the values
        }
        timestamp_to_value.insert(*id_code, vec![changes[0].1; (end_time + 1) as usize]);
        let mut last_value: Option<i64> = None;
        let mut last_timestamp: u64 = 0;
        for (timestamp, value) in changes {
            if let Some(ref last) = last_value {
                for t in (last_timestamp + 1)..*timestamp {
                    timestamp_to_value.get_mut(id_code).unwrap()[t as usize] = *last;
                }
            }
            timestamp_to_value.get_mut(id_code).unwrap()[*timestamp as usize] = *value;
            last_value = Some(*value);
            last_timestamp = *timestamp;
        }
        for t in (last_timestamp + 1)..=end_time {
            if let Some(ref last) = last_value {
                timestamp_to_value.get_mut(id_code).unwrap()[t as usize] = *last;
            } else {
                panic!("No value found for signal {} at all?", id_code);
            }
        }
    }
    timestamp_to_value
}

pub fn get_end_time_from_timetable(time_table: &[wellen::Time]) -> u64 {
    if time_table.is_empty() {
        panic!("Time table is empty, cannot get end time");
    }
    *time_table.last().unwrap()
}

impl WaveFormIntermediate {
    fn new_from_fuzzer_data_point(data_point: &data_types::general_data_types::FuzzerDataPoint, filter_signal_list: Option<&SignalFilters>,  clock_signal: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let waveform_path = std::path::Path::new(&data_point.waveform_path);
        if !(waveform_path.exists()) {
            panic!("Waveform {:?} does not exist!", waveform_path);
        }
        let waveform_file_type = wellen::viewers::open_and_detect_file_format(waveform_path);
        let (timestamp_to_value, signal_to_value_changes, name_to_code, signal_length_map) = if waveform_file_type == wellen::FileFormat::Fst {
            Self::parse_fst_to_hashmap_efficient(&data_point.waveform_path, filter_signal_list, clock_signal).unwrap()
        } else {
            Self::parse_vcd_to_hashmap(&data_point.waveform_path, filter_signal_list, clock_signal).unwrap()        
        };
        
        let waveform = WaveFormIntermediate {
            timestamp_to_value,
            signal_to_value_changes,
            name_to_code,
            signal_length_map,
            path: ustr::ustr(&data_point.waveform_path),
            file_source: data_point.file_source.clone()
        };
        Ok(waveform)
    }

    fn new_from_path(path: &str, filter_signal_list: Option<&SignalFilters>, clock_signal: &str) -> Result<Self,  Box<dyn Error + Send + Sync>> {
        let waveform_file_type = wellen::viewers::open_and_detect_file_format(std::path::Path::new(path));
        let (timestamp_to_value, signal_to_value_changes, name_to_code, signal_length_map) = if waveform_file_type == wellen::FileFormat::Fst {
            Self::parse_fst_to_hashmap_efficient(path, filter_signal_list, clock_signal).unwrap()
        } else {
            Self::parse_vcd_to_hashmap(path, filter_signal_list, clock_signal).unwrap()        
        };
        Ok(WaveFormIntermediate {
            timestamp_to_value,
            signal_to_value_changes,
            name_to_code,
            signal_length_map,
            path: ustr::ustr(&path),
            file_source: data_types::general_data_types::WaveFormSource::Unknown
        })
    }

    fn traverse_scope(scope: &Scope, current_path: &mut Vec<String>, mapping: &mut HashMap<ustr::Ustr, u64, DefaultScalarHasher>, signal_length_map: &mut HashMap<u64, usize, DefaultScalarHasher>, filter_signal_list: Option<&SignalFilters>, clock_signal: &str) {
        current_path.push(scope.identifier.clone());

        for item in &scope.items {
            let v: &Var = match item {
                ScopeItem::Scope(scope) => {
                    Self::traverse_scope(scope, current_path, mapping, signal_length_map, filter_signal_list, clock_signal);
                    continue;
                }
                ScopeItem::Var(var) => var,
                ScopeItem::Comment(_) => unimplemented!(),
                _ => unimplemented!(),
            };
            let full_name = current_path.join(".");
            let var_name = format!("{}.{}", full_name, v.reference);
            // println!("Traversing va/riable: {:?} with code {:?}", var_name, v.code);
            if let Some(filter_signal_list) = filter_signal_list {
                if !filter_signal_list.check_signal_all_filters(&var_name, &v.code.into()) {
                    if !is_metadata_signal(&var_name, clock_signal) {
                        continue;
                    }
                    
                }
            }
            mapping.insert(ustr::ustr(&var_name), v.code.into());
            signal_length_map.insert(v.code.into(), v.size as usize);
        }
        current_path.pop();
    }

    fn parse_fst_to_hashmap_efficient(
        path: &str,
        filter_signal_list: Option<&SignalFilters>,
        clock_signal: &str
    ) -> Result<(HashMap<u64,Vec<SignalValueType>, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>, HashMap<u64, Vec<(u64, SignalValueType)>, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>, HashMap<ustr::Ustr, u64, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>, HashMap<u64, usize, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>), Box<dyn Error + Send + Sync>> {
                // Open the .fst file
        let file = File::open(path)
            .unwrap_or_else(|e| panic!("Error opening FST file {}: {:?}", path, e));
        let reader = BufReader::new(file);
        // Initialize the FST reader (reads file metadata and hierarchy)
        let mut fst_reader = fst_reader::FstReader::open(reader)
            .unwrap_or_else(|err| panic!("Failed to open FST waveform {}: {:?}", path, err));
        let header = fst_reader.get_header();
        let end_time = header.end_time;
        let expected_signal_len = match filter_signal_list {
            Some(filters) => filters.estimate_max_length_matching_signals(),
            None => header.var_count as usize,
        };
        let mut name_to_code: HashMap<ustr::Ustr, u64, DefaultScalarHasher> = 
            HashMap::with_capacity_and_hasher(expected_signal_len, DefaultScalarHasher::default());
        let mut signal_length_map: HashMap<u64, usize, DefaultScalarHasher> = 
            HashMap::with_capacity_and_hasher(expected_signal_len, DefaultScalarHasher::default());
        // Traverse the FST hierarchy (scopes and vars) to populate name_to_code and signal_length_map
        let match_idx = match filter_signal_list {
            Some(filters) => filters.filters.iter().find_map(|f| {
                if let SignalFilter::StrictSignalIdxFilter(idx) = f {
                    Some(idx)
                } else {
                    None
                }
            }),
            None => None,
        };
        let mut current_path: Vec<String> = Vec::new();
        fst_reader.read_hierarchy(|entry| {
            match entry {
               fst_reader::FstHierarchyEntry::Scope { name, .. } => {
                    current_path.push(name);
                }
                fst_reader::FstHierarchyEntry::UpScope => {
                    current_path.pop();
                }
                fst_reader::FstHierarchyEntry::Var { name, length, handle, .. } => {
                    // Construct full signal name with hierarchical path
                    if match_idx.is_some() && !match_idx.as_ref().unwrap().contains(&(handle.get_index() as u64)) {
                        return; // skip variables not in the strict index filter
                    }
                    let full_name = if current_path.is_empty() {
                        name.clone()
                    } else {
                        format!("{}.{}", current_path.join("."), name)
                    };
                    let normalized_name = trim_trailing_range(&full_name, length as usize);
                    // Apply any signal filters (unless it's a clock or metadata signal)
                    if let Some(filters) = filter_signal_list {
                        if !filters.check_signal_all_filters(&normalized_name, &(handle.get_index() as u64)) {
                            if !is_metadata_signal(&normalized_name, clock_signal) {
                                return; // skip this signal
                            }
                        }
                    }
                    name_to_code.insert(ustr::ustr(&normalized_name), handle.get_index() as u64);
                    signal_length_map.insert(handle.get_index() as u64, length as usize);
                }
                _ => { /* Ignore other entries (e.g., attributes, comments) */ }
            }
        })?;  // propagate error if hierarchy parsing fails:contentReference[oaicite:2)
        // for (name, code) in &name_to_code {
        //      println!("Signal {} has code {}", name, code);
        // }
    // Prepare to read signal value changes. Determine which signals to include.
        let tracked_idx: HashSet<u64, DefaultScalarHasher> = name_to_code.values().cloned().collect();
        let filter: fst_reader::FstFilter = if tracked_idx.len() < header.max_handle as usize {
            // Limit to tracked signals for efficiency
            let handles: Vec<fst_reader::FstSignalHandle> = tracked_idx
                .iter()
                .map(|&code| fst_reader::FstSignalHandle::from_index(code as usize))
                .collect();
            fst_reader::FstFilter::new(0, header.end_time, handles)  // include only selected signals:contentReference[oaicite:3]{index=3}
        } else {
            fst_reader::FstFilter::all()  // no filtering, read all signals:contentReference[oaicite:4]{index=4}
        };
        let mut signal_to_value_changes: HashMap<u64, Vec<(u64, SignalValueType)>, DefaultScalarHasher> =
            HashMap::with_capacity_and_hasher(tracked_idx.len(), DefaultScalarHasher::default());
        // Iterate over all value changes in the FST, filtered by the signals of interest
        fst_reader.read_signals(&filter, |timestamp, handle, value| {
            let code = handle.get_index() as u64;
            if !tracked_idx.contains(&code) {
                return; // skip changes for signals not in our tracking set
            }
            // Convert the FST signal value (bits or real) into i64 (same encoding as VCD parser)
            let val_i64: i64 = match value {
                fst_reader::FstSignalValue::String(bit_bytes) => {
                    if bit_bytes.len() == 1 {
                        // Single-bit (scalar) value
                        match bit_bytes[0] {
                            b'0' => 0,
                            b'1' => 1,
                            b'X' | b'x' | b'Z' | b'z' => panic!("Unexpected high-impedance value: {} for signal {} and waveform {}", bit_bytes[0] as char, code, path),
                            _ => panic!("Unexpected scalar value: {} for signal {}", bit_bytes[0] as char, code),
                        }
                    } else {
                        // Vector (multi-bit) value
                        let mut acc: i64 = 0;
                        for &bit in bit_bytes {
                            acc <<= 1;
                            acc |= match bit {
                                b'0' => 0,
                                b'1' => 1,
                                b'X' | b'x' | b'Z' | b'z' => panic!("Unexpected high-impedance value: {} for signal {} and waveform {}", bit as char, code, path),
                                _ => panic!("Unexpected vector bit: {} for signal {}", bit as char, code),
                            };
                        }
                        acc
                    }
                }
                fst_reader::FstSignalValue::Real(val) => {
                    // We expect only bit-string signals in this context
                    unimplemented!("Expected only bit-string signals, got Real({})", val);
                }
            };
            signal_to_value_changes
                .entry(code)
                .or_insert_with(|| Vec::with_capacity(16))
                .push((timestamp, val_i64));
        })?;  // propagate errors during value reading if any
        let timestamp_to_value = get_timestamp_to_value(&signal_to_value_changes, &tracked_idx, end_time);
        Ok((timestamp_to_value, signal_to_value_changes, name_to_code, signal_length_map))
    }

    fn parse_vcd_to_hashmap(
        path: &str,
        filter_signal_list: Option<&SignalFilters>,
        clock_signal: &str
    ) -> Result<(HashMap<u64,Vec<SignalValueType>, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>, HashMap<u64, Vec<(u64, SignalValueType)>, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>, HashMap<ustr::Ustr, u64, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>, HashMap<u64, usize, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>), Box<dyn Error + Send + Sync>> {
        let file = File::open(path).unwrap_or_else(|e| panic!("Error for file: {} {:?}", path, e));
        let reader = BufReader::new(file);
        let mut parser = Parser::new(reader);
        let header: Header = parser.parse_header().unwrap_or_else(|err| {
            panic!("Failed to parse VCD header in file {}: {}", path, err);
        });
        //println!("Signal filter list {:?}", filter_signal_list);
        let expected_signal_len = match filter_signal_list {
            Some(filter_signal_list) => filter_signal_list.estimate_max_length_matching_signals(),
            None => header.items.len(),
        };
        let mut name_to_code: HashMap<ustr::Ustr, u64, std::hash::BuildHasherDefault<rustc_hash::FxHasher>> = HashMap::with_capacity_and_hasher(expected_signal_len, std::hash::BuildHasherDefault::<FxHasher>::default());
        let mut signal_length_map: HashMap<u64, usize, std::hash::BuildHasherDefault<rustc_hash::FxHasher>> = HashMap::with_capacity_and_hasher(expected_signal_len, std::hash::BuildHasherDefault::<FxHasher>::default());
        for scope_item in &header.items {
            match scope_item {
                ScopeItem::Scope(scope) => Self::traverse_scope(&scope, &mut Vec::new(), &mut name_to_code,&mut signal_length_map, filter_signal_list, clock_signal),
                ScopeItem::Var(var) => {
                    if !filter_signal_list.is_none() && !filter_signal_list.unwrap().check_signal_all_filters(&var.reference, &var.code.into()) {
                        if is_metadata_signal(&var.reference, clock_signal) {
                            {}
                        } else {
                            continue;
                        }
                    }
                    let var_name = ustr::ustr(&var.reference); //Arc::from(var.reference.clone());
                    name_to_code.insert(var_name, var.code.into());
                    signal_length_map.insert(var.code.into(), var.size as usize);
                }
                _ => (),
            }
        }
        let tracked_idx = name_to_code.values().cloned().collect::<HashSet<u64, DefaultScalarHasher>>();
        //println!("name to code {:?}", name_to_code);

        let mut signal_to_value_changes: HashMap<u64, Vec<(u64, i64)>, DefaultScalarHasher> = HashMap::with_capacity_and_hasher(tracked_idx.len(), DefaultScalarHasher::default());
        let mut current_time: u64 = 0;
        let mut end_time: u64 = 0;

        for command_result in parser {
            let command = command_result?;
            match command {
                Command::Timestamp(t) => {
                    current_time = t;
                    end_time = std::cmp::max(end_time, current_time);
                }
                Command::ChangeScalar(code, value) => {
                    //timestamp_to_value.insert((code.clone(), current_time), SignalValue::Scalar(value));
                    if !tracked_idx.contains(&code.into()) {
                        continue;
                    }
                    signal_to_value_changes.entry(code.into()).or_insert_with(|| Vec::with_capacity(end_time as usize)).push((current_time, value.into()));
                }
                Command::ChangeVector(code, values) => {
                    if !tracked_idx.contains(&code.into()) {
                        continue;
                    }
                    //timestamp_to_value.insert((code.clone(), current_time), SignalValue::Vector(values.clone()));
                    signal_to_value_changes.entry(code.into()).or_insert_with(|| Vec::with_capacity(end_time as usize)).push((current_time, values.into()));
                }
                Command::ChangeReal(_code, _real_val) => {
                    unimplemented!("Expected only bit-string signals");
                    // if !tracked_idx.contains(&code.into()){
                    //     continue;
                    // }
                    // //timestamp_to_value.insert((code.clone(), current_time), SignalValue::Real(real_val));
                    // signal_to_value_changes.entry(code.into()).or_insert_with(Vec::new).push((current_time, SignalValue::Real(real_val)));
                }
                Command::ChangeString(_code, _s) => {
                    unimplemented!("Expected only bit-string signals");
                    // if !tracked_idx.contains(&code.into()) {
                    //     continue;
                    // }
                    // //timestamp_to_value.insert((code.clone(), current_time), SignalValue::Str(s.clone()));
                    // signal_to_value_changes.entry(code.into()).or_insert_with(Vec::new).push((current_time, SignalValue::Str(s.clone())));
                }
                _ => {
                    // Ignore other command types.
                }
            }
        }
        let timestamp_to_value = get_timestamp_to_value(&signal_to_value_changes, &tracked_idx, end_time);
        Ok((timestamp_to_value, signal_to_value_changes, name_to_code, signal_length_map))
    }

    fn get_signal_changes<'a>(signal_to_value_changes: &'a HashMap<u64, Vec<(u64, SignalValueType)>, DefaultScalarHasher>, name_to_code: &HashMap<ustr::Ustr, u64, DefaultScalarHasher>, signal_name: &str) -> Option<&'a Vec<(u64, SignalValueType)>> {
        if let Some(id_code) = name_to_code.get(&ustr::ustr(signal_name.as_ref())) {
            signal_to_value_changes.get(&((*id_code).into()))
        } else {
            None
        }
    }

    fn get_clock_period(&self, clock_signal: &str) -> (u64, u32) {
        let clk_period;
        let num_cycles;
        if let Some(changes) = Self::get_signal_changes(&self.signal_to_value_changes, &self.name_to_code, clock_signal) {
            if changes.len() < 2 {
                panic!("Clock signal must have at least two changes.");
            }
            let start = changes.first().unwrap().0;
            let first_wrapup = changes[2].0;
            clk_period = first_wrapup - start;
            let last_timestamp: u64 = changes.last().unwrap().0;
            num_cycles = (last_timestamp / clk_period).to_cycle_count();
        } else {
            println!("Clock signal {} (ustrid {:?}) not found in waveform {:?}", clock_signal, ustr::ustr(clock_signal), self.path);
            println!("Available signals are:");
            for signal_name in self.name_to_code.keys() {
                println!("{} ustrid {:?}", signal_name, signal_name);
            }
            panic!("Clock signal {} not found in waveform {:?}", clock_signal, self.path);

        };
        //println!("Clock signal {} has period {} and {} cycles", clock_signal, clk_period, num_cycles);
        // self.clk_period = clk_period;    
        // self.num_cycles = num_cycles+1;
        return (clk_period, num_cycles+1)
    }

}

impl WaveForm {
    pub fn filter_unique_signals(&self, signals: &Vec<Arc<str>>) -> Vec<Arc<str>> {
        let mut unique_codes = std::collections::HashSet::new();
        let mut result = Vec::new();

        for signal in signals.iter() {
            if let Some(code) = self.name_to_code.get(&ustr::existing_ustr(signal.as_ref()).unwrap()) {
                if unique_codes.insert(code) {
                    result.push(signal.clone());
                }
            }
        }
        result
    }

    pub fn filter_control_signals(&self, signals: Vec<Arc<str>>) -> Vec<Arc<str>> {
        //|| signal_name.matches('.').count() <= 2
        let filtered_signals: Vec<Arc<str>> = signals.iter().filter_map(|signal_name| {
            //let signal_name = signal_name_symbol.as_str();
            //(signal_name.contains("data"))
            if self.get_signal_length(signal_name) < 32 || signal_name.contains("inst") || signal_name.contains("addr") || signal_name.contains("address") {
                Some(signal_name.clone())
            } else {
                None
            }
        }).collect();
        //println!("Found in filtered_signals {:?}", filtered_signals.contains(&Arc::from("TOP.correctness.sodor_core.core.c.data_misaligned")));
        let filtered_signals: Vec<Arc<str>> = filtered_signals
            .into_iter()
            //.filter(|signal_name| !signal_name.contains("TOP.correctness.sodor_commit_valid"))
            .collect();
        //Score invariant PriorityScores { cex_only_score: Sat(375), cex_with_bex_block_score: Sat(375), cex_and_bex_score: Sat(3241) }
        //println!("Found in filtered_signals2 {:?}", filtered_signals.contains(&Arc::from("TOP.correctness.sodor_core.core.c.data_misaligned")));
        //let unfiltered_signals: Vec<String> = signals.iter().filter(|signal_name| self.get_signal_length(signal_name) >= 32).cloned().collect();
        
        /*for signal in &filtered_signals {
            println!("Filtered signal: {}", signal);
        }*/
        
        /*for signal in &unfiltered_signals {
            println!("Skipping signal: {}", signal);
        }
        */
        filtered_signals
    }

    pub fn load_waveform_and_cycle_map_from_fuzzer_datapoint(data_point: &data_types::general_data_types::FuzzerDataPoint, clock_signal: &str, filter_signal_list: Option<&SignalFilters>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let waveform_intermediate = WaveFormIntermediate::new_from_fuzzer_data_point(data_point, filter_signal_list, clock_signal).unwrap();
        let (cycle_map, cycle_count, constant_signals) = Self::create_cycle_map(&waveform_intermediate, clock_signal);
        let waveform = WaveForm {
            name_to_code: waveform_intermediate.name_to_code,
            signal_length_map: waveform_intermediate.signal_length_map,
            cycle_map,
            constant_signals: constant_signals,
            num_cycles: cycle_count,
            path: waveform_intermediate.path,
            file_source: waveform_intermediate.file_source,
        };
        Ok(waveform)
    }

    pub fn load_waveform_and_cycle_map(path: &str, clock_signal: &str, filter_signal_list: Option<&SignalFilters>) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let waveform_intermediate = WaveFormIntermediate::new_from_path(path, filter_signal_list, clock_signal).unwrap();
        let (cycle_map, cycle_count, constant_signals) = Self::create_cycle_map(&waveform_intermediate, clock_signal);
        let waveform = WaveForm {
            name_to_code: waveform_intermediate.name_to_code,
            signal_length_map: waveform_intermediate.signal_length_map,
            cycle_map,
            constant_signals: constant_signals,
            num_cycles: cycle_count,
            path: waveform_intermediate.path,
            file_source: waveform_intermediate.file_source,
        };
        Ok(waveform)
    }

    pub fn signal_is_constant(&self, signal_id: &u64) -> bool {
        self.constant_signals.contains_key(signal_id)
    }

    

    fn create_cycle_map(waveform_intermediate: &WaveFormIntermediate, clock_signal: &str) -> (CycleMapType, CycleCount, HashMap<u64, SignalValueType, DefaultScalarHasher>) {
        let (clk_period, num_cycles) = waveform_intermediate.get_clock_period(clock_signal);
        let mut cycle_map: CycleMapType = CycleMapType::default();
        let all_codes = waveform_intermediate.name_to_code.values().collect::<HashSet<&u64>>();
        let mut constant_signals: HashMap<u64, SignalValueType, DefaultScalarHasher> = HashMap::default();
        for id_code in all_codes {
            if waveform_intermediate.signal_to_value_changes.get(id_code).unwrap().len()  == 1 {
                // println!("Signal {:?} is constant in waveform {}", waveform_intermediate.name_to_code.iter().find(|(_, &v)| v == *id_code).unwrap(), waveform_intermediate.path);
                constant_signals.insert(*id_code, waveform_intermediate.signal_to_value_changes.get(id_code).unwrap()[0].1);
                continue; // Constant signal, no need to fill in the values
            }
            let this_signal_timestamp_vector = waveform_intermediate.timestamp_to_value.get(id_code).unwrap();
            let this_cycle_map = cycle_map.entry(*id_code).or_insert_with(|| Vec::with_capacity(num_cycles as usize));
            for cycle in 0..num_cycles {
                let ask_for_timestamp = (cycle as f64) * clk_period as f64; //0.5 would be enough, but let us account for "machine-precision" problems
                let ask_for_timestamp = ask_for_timestamp as u64;
                //if name == "TOP.correctness.correct" {
                //    println!("Cycle {:?} ask for timestamp {:?}", cycle, ask_for_timestamp);
                //}
                //println!("Cycle {:?} ask for timestamp {:?}", cycle, ask_for_timestamp);
                let value = this_signal_timestamp_vector[ask_for_timestamp as usize];
                let value_i64: i64 = value.into(); // match value {
                //     SignalValue::Scalar(v) => {
                //         match v {
                //             Value::V0 => 0,
                //             Value::V1 => 1,
                //             Value::X => -1, //Set X to -1 -> The value is not relevant for computation..
                //             _ => panic!("Unexpected scalar value: {:?} signal {:?}", v, name),
                //         }
                //     },
                //     SignalValue::Vector(v) => {
                //         v.iter().fold(0, |acc, value| {
                //             (acc << 1) | match value {
                //                 Value::V0 => 0,
                //                 Value::V1 => 1,
                //                 Value::X => -1, //Set X to -1 -> The value is not relevant for computation..
                //                 _ => panic!("Unexpected vector value: {:?} signal {:?}", v, name),
                //             }
                //         })
                //     },
                //     _ => panic!("Expected only bit-string signals"),
                // };
                this_cycle_map.push(value_i64);
                //cycle_map.insert((*id_code, cycle), value_i64);
            }
        }
        // println!("Found {} constant signals out of {} signals in waveform {}", constant_signals.len(), waveform_intermediate.signal_length_map.len(), waveform_intermediate.path);
        (cycle_map, CycleCount::from(num_cycles), constant_signals)
    }

    pub fn get_signal_value_at_cycle_from_id(&self, signal_id: &u64, cycle: &CycleCount) -> Option<i64> {
        if let Some(constant_val) = self.constant_signals.get(signal_id) {
            return Some(*constant_val);
        }
        self.cycle_map.get(signal_id).and_then(|values|  Some(values[*cycle as usize]))
    }

    pub fn get_signal_value_at_cycle(&self, signal_name: &str, cycle: CycleCount) -> Option<i64> {
        //let ask_for_timestamp = (cycle as f64 + 0.5) * self.clk_period as f64; //0.5 would be enough, but let us account for "machine-precision" problems
        //let ask_for_timestamp = ask_for_timestamp as u64;
        //println!("ask_for_timestamp {}, cycle {}", ask_for_timestamp as u64, cycle);
        if let Some(id_code) = self.name_to_code.get(&ustr::ustr(signal_name.as_ref())) {
            //println!("timestamp to value {:?}", self.timestamp_to_value.get(&(*id_code, ask_for_timestamp)));
            return self.get_signal_value_at_cycle_from_id(id_code, &cycle);
        } else {
            println!("Id code of signal {} not found", signal_name);
            for (signal_name, key) in self.name_to_code.iter() {
                println!("Signal: {}, Code: {:?}", signal_name, key);
            }
            None
        }
    }

    pub fn print_signal_to_cycle(&self){
        for (signal_name, _key) in self.name_to_code.iter(){
            for cycle in 0..self.num_cycles {
                let value = self.get_signal_value_at_cycle(signal_name, cycle).unwrap();
                println!("Signal: {}, Cycle: {}, Value: {:x}", signal_name, cycle, value);
            }
        }
    }

    pub fn get_signal_length(&self, signal_name: &str) -> usize{
        if let Some(id_code) = self.name_to_code.get(&ustr::existing_ustr(signal_name.as_ref()).unwrap()) {
            return *(self.signal_length_map.get(&id_code).unwrap());
            // let v = self.timestamp_to_value.get(&(id_code.clone(), 0)).unwrap();
            // let length_value = match v {
            //     SignalValue::Scalar(_) => 1,
            //     SignalValue::Vector(v) => v.len(),
            //     SignalValue::Real(_) => panic!("f64 not handled"),
            //     SignalValue::Str(_) => panic!("String not handled"),
            // };
            // return length_value;
        } else {
            panic!("Id code of signal {} not found", signal_name);
        }
    }

    ///
    /// Get the first cycle where the signal has a given value
    pub fn get_first_cycle_for_value(&self, signal_name: &str, value: i64) -> Option<CycleCount> {
        for cycle in 0..self.num_cycles {
            let signal_value = self.get_signal_value_at_cycle(signal_name, cycle);
            if signal_value.is_none() {
                println!("Signal {} not found at cycle {}", signal_name, cycle);
                return None;
            }
            //println!("Signal: {}, Cycle: {}, Value: {:x}", signal_name, cycle, signal_value);
            if signal_value.unwrap() == value {
                return Some(cycle);
            }
        }
        return None;
    }

    pub fn get_signal_aliases(&self, signal_name: &str) -> Vec<Arc<str>> {
        if let Some(&code) = self.name_to_code.get(&ustr::existing_ustr(signal_name.as_ref()).unwrap()) {
            self.name_to_code.iter()
            .filter_map(|(name, &id_code)| if id_code == code { Some(Arc::from(name.as_str())) } else { None })
            .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_signal_aliases_ustr_from_idx(&self, signal_id: &u64) -> Vec<ustr::Ustr> {
        self.name_to_code.iter()
        .filter_map(|(name, &id_code)| if id_code == *signal_id { Some(*name) } else { None })
        .collect()
    }

    pub fn get_first_incorrect_cycle(&self) -> Option<CycleCount> {
        return self.get_first_cycle_for_value(constants::INCORRECTNESS_SIGNAL, 0);
    }

    pub fn get_mismatch_ref_core(&self) -> Option<u64> {
        let mismatch_instruction_cycle = self.get_first_incorrect_cycle();
        if mismatch_instruction_cycle.is_none() {
            return None;
        }
        let mismatch_instruction_cycle = mismatch_instruction_cycle.unwrap();
        let ret = self.get_signal_value_at_cycle(constants::MISMATCH_CYCLE_REF_CORE_SIGNAL, mismatch_instruction_cycle);
        let ret = match ret {
            Some(value) => Some(value as u64),
            None => {
                //println!("Mismatch instruction cycle not found in waveform {}", self.path);
                None
            }
        };
        ret
    }

    pub fn get_first_mismatch_dut_core(&self) -> Option<u64> {
        let mismatch_instruction_cycle = self.get_first_incorrect_cycle();
        if mismatch_instruction_cycle.is_none() {
            return None;
        }
        let mismatch_instruction_cycle = mismatch_instruction_cycle.unwrap();
        let ret = self.get_signal_value_at_cycle(constants::MISMATCH_CYCLE_DUT_CORE_SIGNAL, mismatch_instruction_cycle);
        let ret = match ret {
            Some(value) => Some(value as u64),
            None => {
                //println!("Mismatch instruction cycle not found in waveform {}", self.path);
                None
            }
        };
        ret
    }

    pub fn was_stalled_dut(&self) -> bool {
        let ret = self.get_signal_value_at_cycle(constants::DUT_STALL_SIGNAL, self.num_cycles - 1);
        match ret {
            Some(value) => value != 0,
            None => {
                panic!("DUT stall signal {} not found in waveform {}", constants::DUT_STALL_SIGNAL, self.path);
            }
        }
    }

    pub fn was_stalled_refcore(&self) -> bool {
        let ret = self.get_signal_value_at_cycle(constants::REFCORE_STALL_SIGNAL, self.num_cycles - 1);
        match ret {
            Some(value) => value != 0,
            None => {
                panic!("REFCORE stall signal {} not found in waveform {}", constants::REFCORE_STALL_SIGNAL, self.path);
            }
        }
    }
}
