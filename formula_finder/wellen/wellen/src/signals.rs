// Copyright 2023-2024 The Regents of the University of California
// Copyright 2024-2025 Cornell University
// released under BSD 3-Clause License
// author: Kevin Laeufer <laeufer@cornell.edu>

use crate::fst::{get_bytes_per_entry, get_len_and_meta, push_zeros};
use crate::hierarchy::SignalRef;
use crate::wavemem::{States, check_if_changed_and_truncate};
use crate::{Hierarchy, SignalEncoding};
use num_enum::TryFromPrimitive;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::num::NonZeroU32;

pub type Real = f64;
pub type Time = u64;
pub type TimeTableIdx = u32;
pub type NonZeroTimeTableIdx = NonZeroU32;

#[derive(Debug, Clone, Copy)]
pub enum SignalValue<'a> {
    Binary(&'a [u8], u32),
    FourValue(&'a [u8], u32),
    NineValue(&'a [u8], u32),
    String(&'a str),
    Real(Real),
}

impl Display for SignalValue<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            SignalValue::Binary(data, bits) => {
                write!(f, "{}", two_state_to_bit_string(data, *bits))
            }
            SignalValue::FourValue(data, bits) => {
                write!(f, "{}", four_state_to_bit_string(data, *bits))
            }
            SignalValue::NineValue(data, bits) => {
                write!(f, "{}", nine_state_to_bit_string(data, *bits))
            }
            SignalValue::String(value) => write!(f, "{value}"),
            SignalValue::Real(value) => write!(f, "{value}"),
        }
    }
}

impl PartialEq for SignalValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SignalValue::String(a), SignalValue::String(b)) => a == b,
            (SignalValue::Real(a), SignalValue::Real(b)) => a == b,
            _ => self.to_bit_string().unwrap() == other.to_bit_string().unwrap(),
        }
    }
}

impl SignalValue<'_> {
    pub fn to_bit_string(&self) -> Option<String> {
        match &self {
            SignalValue::Binary(data, bits) => Some(two_state_to_bit_string(data, *bits)),
            SignalValue::FourValue(data, bits) => Some(four_state_to_bit_string(data, *bits)),
            SignalValue::NineValue(data, bits) => Some(nine_state_to_bit_string(data, *bits)),
            other => panic!("Cannot convert {other:?} to bit string"),
        }
    }

    pub fn to_i64(&self) -> Result<i64, Box<dyn std::error::Error>> {
        match self {
            SignalValue::Binary(data, bits) => n_state_to_i64(States::Two, data, *bits),
            SignalValue::FourValue(data, bits) => n_state_to_i64(States::Four, data, *bits),
            SignalValue::NineValue(data, bits) => n_state_to_i64(States::Nine, data, *bits),
            _ => Err("Cannot convert non-binary/four/nine state value to i64".into()),
        }
    }

    /// Returns the number of bits in the signal value. Returns None if the value is a real or string.
    pub fn bits(&self) -> Option<u32> {
        match self {
            SignalValue::Binary(_, bits) => Some(*bits),
            SignalValue::FourValue(_, bits) => Some(*bits),
            SignalValue::NineValue(_, bits) => Some(*bits),
            _ => None,
        }
    }

    /// Returns the states per bit. Returns None if the value is a real or string.
    pub fn states(&self) -> Option<States> {
        match self {
            SignalValue::Binary(_, _) => Some(States::Two),
            SignalValue::FourValue(_, _) => Some(States::Four),
            SignalValue::NineValue(_, _) => Some(States::Nine),
            _ => None,
        }
    }

    /// Returns a reference to the raw data and a mask. Returns None if the value is a real or string.
    pub(crate) fn data_and_mask(&self) -> Option<(&[u8], u8)> {
        match self {
            SignalValue::Binary(data, bits) => Some((*data, States::Two.first_byte_mask(*bits))),
            SignalValue::FourValue(data, bits) => {
                Some((*data, States::Four.first_byte_mask(*bits)))
            }
            SignalValue::NineValue(data, bits) => {
                Some((*data, States::Nine.first_byte_mask(*bits)))
            }
            _ => None,
        }
    }
}

const TWO_STATE_LOOKUP: [char; 2] = ['0', '1'];
const FOUR_STATE_LOOKUP: [char; 4] = ['0', '1', 'x', 'z'];
const NINE_STATE_LOOKUP: [char; 9] = ['0', '1', 'x', 'z', 'h', 'u', 'w', 'l', '-'];

fn two_state_to_bit_string(data: &[u8], bits: u32) -> String {
    n_state_to_bit_string(States::Two, data, bits)
}

fn four_state_to_bit_string(data: &[u8], bits: u32) -> String {
    n_state_to_bit_string(States::Four, data, bits)
}

fn nine_state_to_bit_string(data: &[u8], bits: u32) -> String {
    n_state_to_bit_string(States::Nine, data, bits)
}

// Lookups to convert extracted raw state bits to i64.
// These mappings are based on common interpretations of VCD/FST states.
// For 2-state (1 bit per state): 0 -> 0, 1 -> 1
const TWO_STATE_TO_I64_LOOKUP: [Option<i64>; 2] = [Some(0), Some(1)]; // 0=0, 1=1

// For 4-state (2 bits per state): 00=0, 01=1, 10=X, 11=Z
// X and Z states should cause a panic since only binary values (0,1) are allowed
const FOUR_STATE_TO_I64_LOOKUP: [Option<i64>; 4] = [Some(0), Some(1), None, None];

// For 9-state (4 bits per state):
// Only binary states (0,1) are allowed for i64 conversion, all others cause panic
// Example mapping (common in VCD):
// 0000 -> 0, 0001 -> 1, 0010 -> X, 0011 -> Z, 0100 -> W, 0101 -> L, 0110 -> H, 0111 -> U, 1000 -> -
// All non-0/1 states are mapped to None (will cause panic)
const NINE_STATE_TO_I64_LOOKUP: [Option<i64>; 16] = [
    Some(0),  // 0000 -> 0
    Some(1),  // 0001 -> 1
    None,     // 0010 -> X (panic)
    None,     // 0011 -> Z (panic)
    None,     // 0100 -> W (panic)
    None,     // 0101 -> L (panic)
    None,     // 0110 -> H (panic)
    None,     // 0111 -> U (panic)
    None,     // 1000 -> - (panic)
    None,     // 1001 -> unused (panic)
    None,     // 1010 -> unused (panic)
    None,     // 1011 -> unused (panic)
    None,     // 1100 -> unused (panic)
    None,     // 1101 -> unused (panic)
    None,     // 1110 -> unused (panic)
    None,     // 1111 -> unused (panic)
];

/// Converts a sequence of packed digital states (represented as bytes) into a single i64 value.
///
/// This function processes a byte slice where each byte contains multiple packed digital states
/// (e.g., 1-bit, 2-bit, or 4-bit states). It reconstructs the full signal value as an i64.
/// If any 'X', 'Z', or other unknown/high-impedance states are encountered, the entire
/// resulting i64 value will be -1, consistent with your existing waveform parsing logic.
///
/// # Arguments
/// * `states` - The `States` enum indicating the encoding (Two, Four, or Nine-state).
/// * `data` - A byte slice containing the packed digital states. The most significant bit
///            of the entire signal is assumed to be the most significant bit of `data`.
/// * `bits` - The total number of bits in the signal.
///
/// # Returns
/// A `Result<i64, Box<dyn Error>>` which is:
/// * `Ok(value)` - The converted i64 value.
/// * `Err(error)` - If the input data is empty for a non-zero bit signal.
///
/// # Panics
/// This function will panic if it encounters a raw bit pattern for a single state
/// that is not defined in the lookup table for the specified `States` type.
#[inline]
fn n_state_to_i64(states: States, data: &[u8], bits: u32) -> Result<i64, Box<dyn Error>> {
    if bits == 0 {
        panic!("Cannot convert a signal with 0 bits to i64");
    }
    if data.is_empty() {
        // If bits > 0 but data is empty, it's an error or implicitly unknown
        return Err("Input data cannot be empty for a non-zero bit signal".into());
    }

    let states_bits = states.bits() as u32; // Bits per state (1, 2, or 4)
    let mask = states.mask();
    let bits_per_byte = states.bits_in_a_byte() as u32;

    let lookup: &[Option<i64>] = match states {
        States::Two => &TWO_STATE_TO_I64_LOOKUP,
        States::Four => &FOUR_STATE_TO_I64_LOOKUP,
        States::Nine => &NINE_STATE_TO_I64_LOOKUP,
    };

    let mut result: i64 = 0;
    let mut bits_processed = 0u32;

    // Handle first byte specially if it doesn't contain a full set of states
    // This matches the logic in n_state_to_bit_string
    let byte0_bits = bits - ((bits / bits_per_byte) * bits_per_byte);
    let byte0_is_special = byte0_bits > 0;
    
    if byte0_is_special {
        let byte0 = data[0];
        for ii in (0..byte0_bits).rev() {
            let shift = ii * states_bits;
            let value_raw = (byte0 >> shift) & mask;

            let converted_state = lookup.get(value_raw as usize)
                .unwrap_or_else(|| {
                    panic!("Invalid raw state value {} for {:?} states. This pattern is not defined in the lookup table.", value_raw, states);
                })
                .unwrap_or_else(|| {
                    panic!("Non-binary state encountered in signal data. Only states 0 and 1 can be converted to i64. Found state value: {} for {:?} encoding.", value_raw, states);
                });

            // Each state represents exactly 1 bit in the final result
            result = (result << 1) | converted_state;
            bits_processed += 1;
        }
    }

    // Process remaining full bytes
    for &byte in data.iter().skip(if byte0_is_special { 1 } else { 0 }) {
        for ii in (0..bits_per_byte).rev() {
            if bits_processed >= bits {
                break;
            }

            let shift = ii * states_bits;
            let value_raw = (byte >> shift) & mask;

            let converted_state = lookup.get(value_raw as usize)
                .unwrap_or_else(|| {
                    panic!("Invalid raw state value {} for {:?} states. This pattern is not defined in the lookup table.", value_raw, states);
                })
                .unwrap_or_else(|| {
                    panic!("Non-binary state encountered in signal data. Only states 0 and 1 can be converted to i64. Found state value: {} for {:?} encoding.", value_raw, states);
                });

            // Each state represents exactly 1 bit in the final result
            result = (result << 1) | converted_state;
            bits_processed += 1;
        }
    }

    Ok(result)
}

#[inline]
fn n_state_to_bit_string(states: States, data: &[u8], bits: u32) -> String {
    let lookup = match states {
        States::Two => TWO_STATE_LOOKUP.as_slice(),
        States::Four => FOUR_STATE_LOOKUP.as_slice(),
        States::Nine => NINE_STATE_LOOKUP.as_slice(),
    };
    let bits_per_byte = states.bits_in_a_byte() as u32;
    let states_bits = states.bits() as u32;
    let mask = states.mask();

    let mut out = String::with_capacity(bits as usize);
    if bits == 0 {
        return out;
    }

    // the first byte might not contain a full N bits
    let byte0_bits = bits - ((bits / bits_per_byte) * bits_per_byte);
    let byte0_is_special = byte0_bits > 0;
    if byte0_is_special {
        let byte0 = data[0];
        for ii in (0..byte0_bits).rev() {
            let value = (byte0 >> (ii * states_bits)) & mask;
            let char = lookup[value as usize];
            out.push(char);
        }
    }

    for byte in data.iter().skip(if byte0_is_special { 1 } else { 0 }) {
        for ii in (0..bits_per_byte).rev() {
            let value = (byte >> (ii * states_bits)) & mask;
            let char = lookup[value as usize];
            out.push(char);
        }
    }
    out
}

/// Specifies the encoding of a signal.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub enum FixedWidthEncoding {
    /// Bitvector of length N (u32) with 2, 4 or 9 states.
    /// If `meta_byte` is `true`, each sequence of data bytes is preceded by a meta-byte indicating whether the states
    /// are reduced by 1 (Four -> Two, Nine -> Four) or by 2 (Nine -> Two).
    BitVector {
        max_states: States,
        bits: u32,
        meta_byte: bool,
    },
    /// Each value is encoded as an 8-byte f64 in little endian.
    Real,
}

impl FixedWidthEncoding {
    pub fn signal_encoding(&self) -> SignalEncoding {
        match self {
            FixedWidthEncoding::BitVector { bits, .. } => {
                SignalEncoding::BitVector(NonZeroU32::new(*bits).unwrap())
            }
            FixedWidthEncoding::Real => SignalEncoding::Real,
        }
    }
}

#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct Signal {
    idx: SignalRef,
    time_indices: Vec<TimeTableIdx>,
    data: SignalChangeData,
}

impl Debug for Signal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Signal({:?}, {} changes, {:?})",
            self.idx,
            self.time_indices.len(),
            self.data
        )
    }
}

impl PartialEq for Signal {
    fn eq(&self, other: &Self) -> bool {
        if self.idx == other.idx {
            let mut our_iter = self.iter_changes();
            let mut other_iter = other.iter_changes();
            loop {
                if let Some((our_time, our_value)) = our_iter.next() {
                    if let Some((other_time, other_value)) = other_iter.next() {
                        if our_time != other_time || our_value != other_value {
                            return false;
                        }
                    } else {
                        return false;
                    }
                } else {
                    return other_iter.next().is_none();
                }
            }
        } else {
            false
        }
    }
}

impl Eq for Signal {}

impl Signal {
    pub fn new_fixed_len(
        idx: SignalRef,
        time_indices: Vec<TimeTableIdx>,
        encoding: FixedWidthEncoding,
        width: u32,
        bytes: Vec<u8>,
    ) -> Self {
        debug_assert_eq!(time_indices.len(), bytes.len() / width as usize);
        let data = SignalChangeData::FixedLength {
            encoding,
            width,
            bytes,
        };
        Signal {
            idx,
            time_indices,
            data,
        }
    }

    pub fn new_var_len(
        idx: SignalRef,
        time_indices: Vec<TimeTableIdx>,
        strings: Vec<String>,
    ) -> Self {
        assert_eq!(time_indices.len(), strings.len());
        let data = SignalChangeData::VariableLength(strings);
        Signal {
            idx,
            time_indices,
            data,
        }
    }

    pub fn size_in_memory(&self) -> usize {
        let base = std::mem::size_of::<Self>();
        let time = self.time_indices.len() * std::mem::size_of::<TimeTableIdx>();
        let data = match &self.data {
            SignalChangeData::FixedLength { bytes, .. } => bytes.len(),
            SignalChangeData::VariableLength(strings) => strings
                .iter()
                .map(|s| s.len() + std::mem::size_of::<String>())
                .sum::<usize>(),
        };
        base + time + data
    }

    /// Returns the data offset for the nearest change at or before the provided idx.
    /// Returns `None` of not data is available for this signal at or before the idx.
    pub fn get_offset(&self, time_table_idx: TimeTableIdx) -> Option<DataOffset> {
        match self.time_indices.first() {
            None => None,
            Some(first) if *first > time_table_idx => None,
            _ => Some(find_offset_from_time_table_idx(
                &self.time_indices,
                time_table_idx,
            )),
        }
    }

    pub fn get_time_idx_at(&self, offset: &DataOffset) -> TimeTableIdx {
        self.time_indices[offset.start]
    }

    pub fn get_value_at(&self, offset: &DataOffset, element: u16) -> SignalValue<'_> {
        assert!(element < offset.elements);
        self.data.get_value_at(offset.start + element as usize)
    }

    pub fn get_first_time_idx(&self) -> Option<TimeTableIdx> {
        self.time_indices.first().cloned()
    }

    pub fn time_indices(&self) -> &[TimeTableIdx] {
        &self.time_indices
    }

    pub fn iter_changes(&self) -> SignalChangeIterator<'_> {
        SignalChangeIterator::new(self)
    }

    pub fn signal_ref(&self) -> SignalRef {
        self.idx
    }

    pub(crate) fn signal_encoding(&self) -> SignalEncoding {
        self.data.signal_encoding()
    }

    pub fn get_width_or_panic(&self) -> u32 {
        match &self.data.signal_encoding() {
            SignalEncoding::BitVector(width) => width.get(),
            _ => panic!(
                "Cannot get width of signal with encoding: {:?}",
                self.data.signal_encoding()
            ),
        }
    }
}

pub struct SignalChangeIterator<'a> {
    signal: &'a Signal,
    offset: usize,
}

impl<'a> SignalChangeIterator<'a> {
    fn new(signal: &'a Signal) -> Self {
        Self { signal, offset: 0 }
    }
}

impl<'a> Iterator for SignalChangeIterator<'a> {
    type Item = (TimeTableIdx, SignalValue<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(time_idx) = self.signal.time_indices.get(self.offset) {
            let data = self.signal.data.get_value_at(self.offset);
            self.offset += 1;
            Some((*time_idx, data))
        } else {
            None
        }
    }
}

pub struct BitVectorBuilder {
    max_states: States,
    bits: u32,
    len: usize,
    has_meta: bool,
    bytes_per_entry: usize,
    data: Vec<u8>,
    time_indices: Vec<TimeTableIdx>,
}

impl BitVectorBuilder {
    fn new(max_states: States, bits: u32) -> Self {
        assert!(bits > 0);
        let (len, has_meta) = get_len_and_meta(max_states, bits);
        let bytes_per_entry = get_bytes_per_entry(len, has_meta);
        let data = vec![];
        let time_indices = vec![];
        Self {
            max_states,
            bits,
            len,
            has_meta,
            bytes_per_entry,
            data,
            time_indices,
        }
    }

    fn add_change(&mut self, time_idx: TimeTableIdx, value: SignalValue) {
        debug_assert_eq!(value.bits().unwrap(), self.bits);
        let local_encoding = value.states().unwrap();
        debug_assert!(local_encoding.bits() >= self.max_states.bits());
        if self.bits == 1 {
            let (value, mask) = value.data_and_mask().unwrap();
            let value = value[0] & mask;
            let meta_data = (local_encoding as u8) << 6;
            self.data.push(value | meta_data);
        } else {
            let (data, mask) = value.data_and_mask().unwrap();
            let (local_len, local_has_meta) = get_len_and_meta(local_encoding, self.bits);
            assert_eq!(data.len(), local_len);

            // append data
            let meta_data = (local_encoding as u8) << 6;
            if local_len == self.len && local_has_meta == self.has_meta {
                // same meta-data location and length as the maximum
                if self.has_meta {
                    self.data.push(meta_data);
                    self.data.push(data[0] & mask);
                } else {
                    self.data.push(meta_data | (data[0] & mask));
                }
                self.data.extend_from_slice(&data[1..]);
            } else {
                // smaller encoding than the maximum
                self.data.push(meta_data);
                if self.has_meta {
                    push_zeros(&mut self.data, self.len - local_len);
                } else {
                    push_zeros(&mut self.data, self.len - local_len - 1);
                }
                self.data.push(data[0] & mask);
                self.data.extend_from_slice(&data[1..]);
            }
        }
        // see if there actually was a change and revert if there was not
        if check_if_changed_and_truncate(self.bytes_per_entry, &mut self.data) {
            self.time_indices.push(time_idx);
        }
    }

    fn finish(self, id: SignalRef) -> Signal {
        debug_assert_eq!(
            self.data.len(),
            self.time_indices.len() * self.bytes_per_entry
        );
        let encoding = FixedWidthEncoding::BitVector {
            max_states: self.max_states,
            bits: self.bits,
            meta_byte: self.has_meta,
        };
        Signal::new_fixed_len(
            id,
            self.time_indices,
            encoding,
            self.bytes_per_entry as u32,
            self.data,
        )
    }
}

pub fn slice_signal(id: SignalRef, signal: &Signal, msb: u32, lsb: u32) -> Signal {
    debug_assert!(msb >= lsb);
    if let SignalChangeData::FixedLength {
        encoding: FixedWidthEncoding::BitVector { max_states, .. },
        ..
    } = &signal.data
    {
        slice_bit_vector(id, signal, msb, lsb, *max_states)
    } else {
        panic!("Cannot slice signal with data: {:?}", signal.data);
    }
}

fn slice_bit_vector(
    id: SignalRef,
    signal: &Signal,
    msb: u32,
    lsb: u32,
    max_states: States,
) -> Signal {
    debug_assert!(msb >= lsb);
    let result_bits = msb - lsb + 1;
    let mut builder = BitVectorBuilder::new(max_states, result_bits);
    let mut buf = Vec::with_capacity(result_bits.div_ceil(2) as usize);
    for (time_idx, value) in signal.iter_changes() {
        let out_value = match value {
            SignalValue::Binary(data, in_bits) => {
                slice_n_states(
                    States::Two,
                    data,
                    &mut buf,
                    msb as usize,
                    lsb as usize,
                    in_bits as usize,
                );
                SignalValue::Binary(&buf, result_bits)
            }
            SignalValue::FourValue(data, in_bits) => {
                slice_n_states(
                    States::Four,
                    data,
                    &mut buf,
                    msb as usize,
                    lsb as usize,
                    in_bits as usize,
                );
                SignalValue::FourValue(&buf, result_bits)
            }
            SignalValue::NineValue(data, in_bits) => {
                slice_n_states(
                    States::Nine,
                    data,
                    &mut buf,
                    msb as usize,
                    lsb as usize,
                    in_bits as usize,
                );
                SignalValue::NineValue(&buf, result_bits)
            }
            _ => unreachable!("expected a bit vector"),
        };
        builder.add_change(time_idx, out_value);
        buf.clear();
    }
    builder.finish(id)
}

#[inline]
fn slice_n_states(
    states: States,
    data: &[u8],
    out: &mut Vec<u8>,
    msb: usize,
    lsb: usize,
    in_bits: usize,
) {
    let out_bits = msb - lsb + 1;
    debug_assert!(in_bits > out_bits);
    let mut working_byte = 0u8;
    for (out_bit, in_bit) in (lsb..(msb + 1)).enumerate().rev() {
        let rev_in_bit = in_bits - in_bit - 1;
        let in_byte = data[rev_in_bit / states.bits_in_a_byte()];
        let in_value =
            (in_byte >> ((in_bit % states.bits_in_a_byte()) * states.bits())) & states.mask();

        working_byte = (working_byte << states.bits()) + in_value;
        if out_bit % states.bits_in_a_byte() == 0 {
            out.push(working_byte);
            working_byte = 0;
        }
    }
}

/// Finds the index that is the same or less than the needle and returns the position of it.
/// Note that `indices` needs to be sorted from smallest to largest.
/// Essentially implements a binary search!
fn find_offset_from_time_table_idx(indices: &[TimeTableIdx], needle: TimeTableIdx) -> DataOffset {
    debug_assert!(!indices.is_empty(), "empty time table");

    // find the index of a matching time
    let res = binary_search(indices, needle);
    let res_index = indices[res];

    // find start
    let mut start = res;
    while start > 0 && indices[start - 1] == res_index {
        start -= 1;
    }
    // find number of elements
    let mut elements = 1;
    while start + elements < indices.len() && indices[start + elements] == res_index {
        elements += 1;
    }

    // find next index
    let next_index = if start + elements < indices.len() {
        NonZeroTimeTableIdx::new(indices[start + elements])
    } else {
        None
    };

    DataOffset {
        start,
        elements: elements as u16,
        time_match: res_index == needle,
        next_index,
    }
}

#[inline]
fn binary_search(indices: &[TimeTableIdx], needle: TimeTableIdx) -> usize {
    debug_assert!(!indices.is_empty(), "empty time table!");
    let mut lower_idx = 0usize;
    let mut upper_idx = indices.len() - 1;
    while lower_idx <= upper_idx {
        let mid_idx = lower_idx + ((upper_idx - lower_idx) / 2);

        match indices[mid_idx].cmp(&needle) {
            std::cmp::Ordering::Less => {
                lower_idx = mid_idx + 1;
            }
            std::cmp::Ordering::Equal => {
                return mid_idx;
            }
            std::cmp::Ordering::Greater => {
                upper_idx = mid_idx - 1;
            }
        }
    }
    lower_idx - 1
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct DataOffset {
    /// Offset of the first data value at the time requested (or earlier).
    pub start: usize,
    /// Number of elements that have the same time index. This is usually 1. Greater when there are delta cycles.
    pub elements: u16,
    /// Indicates that the offset exactly matches the time requested. If false, then we are matching an earlier time step.
    pub time_match: bool,
    /// Indicates the time table index of the next change.
    pub next_index: Option<NonZeroTimeTableIdx>,
}

#[derive(Eq, PartialEq)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
enum SignalChangeData {
    FixedLength {
        encoding: FixedWidthEncoding,
        width: u32, // bytes per entry
        bytes: Vec<u8>,
    },
    VariableLength(Vec<String>),
}

impl SignalChangeData {
    fn signal_encoding(&self) -> SignalEncoding {
        match self {
            SignalChangeData::FixedLength { encoding, .. } => encoding.signal_encoding(),
            SignalChangeData::VariableLength(_) => SignalEncoding::String,
        }
    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        match self {
            SignalChangeData::FixedLength { bytes, .. } => bytes.is_empty(),
            SignalChangeData::VariableLength(data) => data.is_empty(),
        }
    }
}

impl Debug for SignalChangeData {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalChangeData::FixedLength {
                encoding, bytes, ..
            } => {
                write!(
                    f,
                    "SignalChangeData({encoding:?}, {} data bytes)",
                    bytes.len()
                )
            }
            SignalChangeData::VariableLength(values) => {
                write!(f, "SignalChangeData({} strings)", values.len())
            }
        }
    }
}

impl SignalChangeData {
    fn get_value_at(&self, offset: usize) -> SignalValue<'_> {
        match &self {
            SignalChangeData::FixedLength {
                encoding,
                width,
                bytes,
            } => {
                let start = offset * (*width as usize);
                let raw_data = &bytes[start..(start + (*width as usize))];
                match encoding {
                    FixedWidthEncoding::BitVector {
                        max_states,
                        bits,
                        meta_byte,
                    } => {
                        let data = if *meta_byte { &raw_data[1..] } else { raw_data };
                        match max_states {
                            States::Two => {
                                debug_assert!(!meta_byte);
                                // if the max state is 2, then all entries must be binary
                                SignalValue::Binary(data, *bits)
                            }
                            States::Four | States::Nine => {
                                // otherwise the actual number of states is encoded in the meta data
                                let meta_value = (raw_data[0] >> 6) & 0x3;
                                if States::try_from_primitive(meta_value).is_err() {
                                    println!(
                                        "ERROR: offset={offset}, encoding={encoding:?}, width={width}, raw_data[0]={}",
                                        raw_data[0]
                                    );
                                }
                                let states = States::try_from_primitive(meta_value).unwrap();
                                let num_out_bytes = states.bytes_required(*bits as usize);
                                debug_assert!(num_out_bytes <= data.len());
                                let signal_bytes = if num_out_bytes == data.len() {
                                    data
                                } else {
                                    &data[(data.len() - num_out_bytes)..]
                                };
                                match states {
                                    States::Two => SignalValue::Binary(signal_bytes, *bits),
                                    States::Four => SignalValue::FourValue(signal_bytes, *bits),
                                    States::Nine => SignalValue::NineValue(signal_bytes, *bits),
                                }
                            }
                        }
                    }
                    FixedWidthEncoding::Real => SignalValue::Real(Real::from_le_bytes(
                        <[u8; 8]>::try_from(raw_data).unwrap(),
                    )),
                }
            }
            SignalChangeData::VariableLength(strings) => SignalValue::String(&strings[offset]),
        }
    }
}

pub trait SignalSourceImplementation: Sync + Send {
    /// Loads new signals.
    /// Many implementations take advantage of loading multiple signals at a time.
    fn load_signals(
        &mut self,
        ids: &[SignalRef],
        types: &[SignalEncoding],
        multi_threaded: bool,
    ) -> Vec<Signal>;
    /// Print memory size / speed statistics.
    fn print_statistics(&self);
}

pub struct SignalSource {
    inner: Box<dyn SignalSourceImplementation>,
}

impl SignalSource {
    pub fn new(inner: Box<dyn SignalSourceImplementation + Send + Sync>) -> Self {
        Self { inner }
    }

    /// Loads new signals.
    /// Many implementations take advantage of loading multiple signals at a time.
    pub fn load_signals(
        &mut self,
        ids: &[SignalRef],
        hierarchy: &Hierarchy,
        multi_threaded: bool,
    ) -> Vec<(SignalRef, Signal)> {
        // sort and dedup ids
        let mut ids = Vec::from_iter(ids.iter().cloned());
        ids.sort();
        ids.dedup();

        // replace any aliases by their source signal
        let orig_ids = ids.clone();
        let mut is_alias = vec![false; ids.len()];
        for (ii, id) in ids.iter_mut().enumerate() {
            if let Some(slice) = hierarchy.get_slice_info(*id) {
                *id = slice.sliced_signal;
                is_alias[ii] = true;
                println!("Is alias: {:?} -> {:?}", orig_ids[ii], id);
            }
        }

        // collect meta data
        let types: Vec<_> = ids
            .iter()
            .map(|i| hierarchy.get_signal_tpe(*i).unwrap())
            .collect();
        let signals = self.inner.load_signals(&ids, &types, multi_threaded);
        // the signal source must always return the correct number of signals!
        assert_eq!(signals.len(), ids.len());
        let mut out = Vec::with_capacity(orig_ids.len());
        for ((id, is_alias), signal) in orig_ids
            .iter()
            .zip(is_alias.iter())
            .zip(signals.into_iter())
        {
            if *is_alias {
                let slice = hierarchy.get_slice_info(*id).unwrap();
                let sliced = slice_signal(*id, &signal, slice.msb, slice.lsb);
                out.push((*id, sliced));
            } else {
                out.push((*id, signal));
            }
        }
        out
    }

    /// Print memory size / speed statistics.
    pub fn print_statistics(&self) {
        self.inner.print_statistics();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sizes() {
        assert_eq!(std::mem::size_of::<SignalRef>(), 4);

        // 4 bytes for length + tag + padding
        assert_eq!(std::mem::size_of::<FixedWidthEncoding>(), 8);

        assert_eq!(std::mem::size_of::<SignalChangeData>(), 40);
        assert_eq!(std::mem::size_of::<Signal>(), 72);

        // since there is some empty space in the Signal struct, we can make it an option for free!
        assert_eq!(std::mem::size_of::<Option<Signal>>(), 72);

        // signal values contain a slice (ptr + len) as well as a tag and potentially a length
        assert_eq!(std::mem::size_of::<&[u8]>(), 16);
        assert_eq!(std::mem::size_of::<SignalValue>(), 16 + 8);
    }

    #[test]
    fn test_to_bit_string_binary() {
        let data0 = [0b11100101u8, 0b00110010];
        let full_str = "1110010100110010";
        let full_str_len = full_str.len();

        for bits in 0..(full_str_len + 1) {
            let expected: String = full_str.chars().skip(full_str_len - bits).collect();
            let number_of_bytes = bits.div_ceil(8);
            let drop_bytes = data0.len() - number_of_bytes;
            let data = &data0[drop_bytes..];
            assert_eq!(
                SignalValue::Binary(data, bits as u32)
                    .to_bit_string()
                    .unwrap(),
                expected,
                "bits={bits}"
            );
        }
    }

    #[test]
    fn test_to_bit_string_four_state() {
        let data0 = [0b11100101u8, 0b00110010];
        let full_str = "zx110z0x";
        let full_str_len = full_str.len();

        for bits in 0..(full_str_len + 1) {
            let expected: String = full_str.chars().skip(full_str_len - bits).collect();
            let number_of_bytes = bits.div_ceil(4);
            let drop_bytes = data0.len() - number_of_bytes;
            let data = &data0[drop_bytes..];
            assert_eq!(
                SignalValue::FourValue(data, bits as u32)
                    .to_bit_string()
                    .unwrap(),
                expected,
                "bits={bits}"
            );
        }
    }

    #[test]
    fn test_long_2_state_to_string() {
        let data = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0b110001, 0b11, 0b10110011,
        ];
        let out = SignalValue::Binary(data.as_slice(), 153)
            .to_bit_string()
            .unwrap();
        let expected = "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001100010000001110110011";
        assert_eq!(out, expected);
    }

    #[test]
    fn test_slice_signal() {
        let mut out = vec![];
        slice_n_states(States::Two, &[0b001001], &mut out, 3, 3, 7);
        assert_eq!(out[0], 1);
        out.clear();
    }

    #[test]
    fn test_n_state_to_i64_binary() {
        // Test binary (2-state) conversion
        let data = [0b11100101u8, 0b00110010];
        
        // Test various bit lengths
        for bits in 1u32..=16u32 {
            let number_of_bytes = bits.div_ceil(8) as usize;
            let drop_bytes = data.len() - number_of_bytes;
            let test_data = &data[drop_bytes..];
            
            // Convert to bit string for comparison
            let bit_string = SignalValue::Binary(test_data, bits)
                .to_bit_string()
                .unwrap();
            
            // Convert to i64
            let i64_result = n_state_to_i64(States::Two, test_data, bits).unwrap();
            
            // Convert the bit string to i64 manually for verification
            let expected = i64::from_str_radix(&bit_string, 2).unwrap();
            
            assert_eq!(i64_result, expected, 
                "Mismatch for {} bits: bit_string='{}', i64_result={}, expected={}", 
                bits, bit_string, i64_result, expected);
        }
    }

    #[test]
    fn test_n_state_to_i64_four_state() {
        // Test four-state conversion (only 0 and 1 states should convert properly)
        // For 4-state: 00=0, 01=1, 10=X(2), 11=Z(3)
        let data = [0b00010001u8]; // 00,01,00,01 in 2-bit encoding = "0101" (binary values only)
        
        let result = n_state_to_i64(States::Four, &data, 4).unwrap();
        assert_eq!(result, 0b0101); // Should be 5
        
        // Test with X and Z states (should panic)
        let data_with_x = [0b10110011u8]; // 10,11,00,11 in 2-bit encoding = "XZ0Z"
        
        // Test that it panics when encountering X state
        let result = std::panic::catch_unwind(|| n_state_to_i64(States::Four, &data_with_x, 4));
        assert!(result.is_err(), "Expected panic when encountering X/Z states");
    }

    #[test]
    fn test_n_state_to_i64_nine_state() {
        // Test nine-state conversion (only 0 and 1 states should convert properly)
        let data = [0b00000001u8]; // 0000,0001 in 4-bit encoding = "01" 
        
        let result = n_state_to_i64(States::Nine, &data, 2).unwrap();
        assert_eq!(result, 0b01); // Should be 1
        
        // Test with X state (should panic)  
        let data_with_x = [0b00100001u8]; // 0010,0001 in 4-bit encoding = "X1"
        
        let result = std::panic::catch_unwind(|| n_state_to_i64(States::Nine, &data_with_x, 2));
        assert!(result.is_err(), "Expected panic when encountering X state in nine-state");
    }

    #[test]
    fn test_n_state_to_i64_consistency_with_bit_string() {
        // Test that n_state_to_i64 produces results consistent with to_bit_string
        // for simple binary cases
        
        let test_cases: &[(&[u8], u32)] = &[
            (&[0b10101010u8], 8),
            (&[0b11110000u8], 8), 
            (&[0b00000001u8], 8),
            (&[0b11111111u8], 8),
            (&[0b10101010u8, 0b01010101u8], 16),
        ];
        
        for (data, bits) in test_cases {
            let bit_string = SignalValue::Binary(data, *bits)
                .to_bit_string()
                .unwrap();
            
            let i64_result = n_state_to_i64(States::Two, data, *bits).unwrap();
            let expected = i64::from_str_radix(&bit_string, 2).unwrap();
            
            assert_eq!(i64_result, expected,
                "Inconsistency: data={:?}, bits={}, bit_string='{}', i64_result={}, expected={}",
                data, bits, bit_string, i64_result, expected);
        }
    }
}
