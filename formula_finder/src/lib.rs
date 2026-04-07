use std::ffi::c_char;
use std::ffi::c_ulonglong;
pub mod cycle_types;
pub mod waveform;
pub mod smt_solver;
pub mod predicates;
pub mod invariant_searcher;
pub mod allowed_signal_config;
pub mod teacher;
pub mod utils;
pub mod constants;
pub mod data_types;
pub mod decision_tree;
pub mod set_cover_heuristic;
pub mod set_cover_preprocessing;
#[cfg(feature = "ilp")]
pub mod ilp_solver;
pub mod solver;
pub mod jg_formatter;
pub mod set_cover_solver;
pub mod architecture_checker;
pub mod costs;
pub mod set_cover_instance;
use data_types::general_data_types::FuzzerDataPoint;
use serde_json::Value;
use invariant_searcher::SearchResult;
use invariant_searcher::InputCexDescription;
use std::error::Error;
use std::ffi::CString;
use std::fs::File;
use std::io::BufReader;
//use jemallocator;
//#[global_allocator]
//static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;


fn _extract_waveforms(output_sets: &Value, key: &str, waveforms: &mut Vec<String>) {
    if let Some(array) = output_sets.get(key).and_then(|v| v.as_array()) {
        for item in array {
            if let Some(waveform) = item.get("waveform").and_then(|v| v.as_str()) {
                waveforms.push(waveform.to_string());
            }
        }
    }
}

fn extract_fuzzer_data_points(output_sets: &Value, key: &str, fuzzer_data_points: &mut Vec<data_types::general_data_types::FuzzerDataPoint>, max_datapoints: Option<usize>) {
    let mut mutation_datapoint = Vec::new();
    let mut non_mutation_bex = Vec::new();
    if let Some(array) = output_sets.get(key).and_then(|v| v.as_array()) {
        for item in array {
            let data_point: data_types::general_data_types::FuzzerDataPoint = serde_json::from_value(item.clone()).unwrap();
            if data_point.file_source != data_types::general_data_types::WaveFormSource::Mutations {
                non_mutation_bex.push(data_point.clone());
            } else {
                mutation_datapoint.push(data_point.clone());
            }
            //if let Some(waveform) = item.get("waveform").and_then(|v| v.as_str()) {
            //    waveforms.push(waveform.to_string());
            //}
        }
    }
    if let Some(max_datapoints) = max_datapoints {
        non_mutation_bex.sort_by(|a, b| {
            a.program_distance.partial_cmp(&b.program_distance).unwrap()
        });
        non_mutation_bex = non_mutation_bex.into_iter().take(max_datapoints).collect();
        /*
        if non_mutation_bex.len() > max_datapoints {
            let mut rng = rand::thread_rng();
            non_mutation_bex.shuffle(&mut rng);
            non_mutation_bex = non_mutation_bex.into_iter().take(max_datapoints).collect();
        }
         */
    }
    fuzzer_data_points.extend(mutation_datapoint);
    fuzzer_data_points.extend(non_mutation_bex);

}


pub fn create_searcher_from_config(
    output_sets_path: &str, 
    regex_config_path: &str,
    bex_multiplier: usize,
    predicate_base_cost: usize
) -> Result<(invariant_searcher::PriorityInvariantSearcher, Value, Option<String>), Box<dyn Error>> {
    let output_sets_file = match File::open(output_sets_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Error for opening {}: {}", output_sets_path, e);
            return Err(e.into());
        }
    };
    let output_sets_reader = BufReader::new(output_sets_file);
    let output_sets: Value = serde_json::from_reader(output_sets_reader)?;
    let bug_type = output_sets.get("bug_type").and_then(|v| Some(v.as_str().unwrap().to_owned()));
    println!("Loading waveforms");

    let regex_stages_file = File::open(regex_config_path);
    let regex_stages: Value = match regex_stages_file {
        Ok(file) => {
            let reader = BufReader::new(file);
            serde_json::from_reader(reader)?
        }
        Err(_) => return Err(format!("regex_stages {} file not found", regex_config_path).into()),
    };
    //Set regex_stages to regex_stages["regex_stages"]
    let regex_stages = regex_stages.get("regex_stages").unwrap();

    let mut cex_data_points = Vec::new();
    let mut bex_data_points = Vec::new();

    extract_fuzzer_data_points(&output_sets, "cex", &mut cex_data_points, None);
    extract_fuzzer_data_points(&output_sets, "bex", &mut bex_data_points, Some(constants::MAX_NUM_BEX));
    if bex_data_points.len() < cex_data_points.len() * 120 / 100 {
        println!(
            "Warning: The number of bex_data_points ({}) is not at least 20% greater than the number of cex_data_points ({}). \n
            This may lead to ineffective separator search.
            ",
            bex_data_points.len(),
            cex_data_points.len()
        );
    }
    let allowed_signal_list = output_sets.get("allowed_signals");
    let cond_map = output_sets.get("conditional_signals_to_condition_mapping");

    let invariant_searcher = if allowed_signal_list.is_some() || cond_map.is_some() {
        let remap_info = data_types::signal_remap_info::SignalRemappingInfo::load_from_json_strings(allowed_signal_list, cond_map);
        invariant_searcher::PriorityInvariantSearcher::new_from_fuzzer_data_point_and_signal_remap_info(
            cex_data_points,
            bex_data_points,
            "TOP.correctness.clk",
            remap_info,
            data_types::general_data_types::FormulaScoreWeights { bex_multiplier, predicate_base_cost }
        )
    } else  {
        invariant_searcher::PriorityInvariantSearcher::new_from_fuzzer_data_point(
            cex_data_points,
            bex_data_points,
            "TOP.correctness.clk",
            data_types::general_data_types::FormulaScoreWeights { bex_multiplier, predicate_base_cost }
        )
    };

    //println!("Searching invariants");
    Ok((invariant_searcher, regex_stages.clone(), bug_type))
}



pub fn search_invariants(
    output_sets_path: &str,
    regex_config_path: &str,
    bex_multiplier: usize,
    predicate_base_cost: usize
) -> Result<SearchResult, Box<dyn Error>> {
    //rayon::ThreadPoolBuilder::new().num_threads(constants::NUM_THREADS).build_global(); //.unwrap();
    // costs::set_bex_multiplier(bex_multiplier);
    // costs::set_predicate_base_cost(predicate_base_cost);
    let output_sets_file = match File::open(output_sets_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Error for opening {}: {}", output_sets_path, e);
            return Err(e.into());
        }
    };
    let output_sets_reader: BufReader<File> = BufReader::new(output_sets_file);
    let output_sets: Value = serde_json::from_reader(output_sets_reader)?;
    // if let Some(original_jaspergold_waveform_path) = output_sets.get("original_jaspergold_waveform_path").and_then(|v| v.as_str()) {
    //     let original_jaspergold_waveform_path = original_jaspergold_waveform_path.to_string();
    //     let waveform = waveform::WaveForm::load_waveform_and_cycle_map(&original_jaspergold_waveform_path, "clk").unwrap();
    // }
    let (invariant_searcher, regex_stages, bug_type) = create_searcher_from_config(
        output_sets_path,
        regex_config_path,
        bex_multiplier,
        predicate_base_cost
    )?;
    let res: Option<SearchResult> = invariant_searcher
        .search_separator_in_stages(regex_stages.clone(), bug_type);
    let mut res: SearchResult = if res.is_some() {
        res.unwrap()
    } else {
        return Err("Invariant not found".into());
    };
    
    /*if let Some(original_jaspergold_waveform_path) = output_sets.get("original_jaspergold_waveform_path").and_then(|v| v.as_str()) {
        println!("Using original jaspergold waveform path to remap signal names: {}", original_jaspergold_waveform_path);
        let original_jaspergold_waveform_path = original_jaspergold_waveform_path.to_string();
        res.assume_invariant = jg_formatter::get_jg_assume_command(&res.invariant, Some(original_jaspergold_waveform_path.as_str()), Some("correctness.clk"));
        res.assert_invariant = jg_formatter::get_jg_assert_command(&res.invariant, Some(original_jaspergold_waveform_path.as_str()), Some("correctness.clk"));
    }*/
    // if let Some(allowed_signals_list) = output_sets.get("allowed_signals").and_then(|v| v.as_array()) {
    //     // let allowed_signals: HashSet<String> = allowed_signals_list.iter()
    //     //     .filter_map(|v| v.as_str().map(|s| s.to_string()))
    //     //     .collect();
    //     let allowed_signals: HashSet<Arc<str>> = allowed_signals_list.iter()
    //         .filter_map(|v| v.as_str().map(|s| Arc::from(format!("correctness.{}", s.clone()))))
    //         .collect();
    //     let signal_mapping = jg_formatter::get_signal_mapping_between_jg_and_verilator(
    //         &allowed_signals,
    //         &res.invariant.get_signal_names().iter().map(|s| Arc::from(s.clone())).collect::<Vec<Arc<str>>>()
    //     );
    //     res.assume_invariant = jg_formatter::get_jg_assume_command(&res.invariant, Some(&signal_mapping));
    //     res.assert_invariant = jg_formatter::get_jg_assert_command(&res.invariant, Some(&signal_mapping));
    // }
    let minimized_instructions: Vec<String> =  output_sets.get("minimized_cex").and_then(|v| v.get("instructions")).and_then(|v: &Value| v.as_array()).unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let instructions: Vec<String> =  output_sets.get("input_cex").and_then(|v| v.get("instructions")).and_then(|v: &Value| v.as_array()).unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let input_cex_description = InputCexDescription{
        bug_type: Some(output_sets.get("bug_type").and_then(|v| Some(v.as_str().unwrap().to_owned())).unwrap()),
        minimized_instructions: Some(minimized_instructions),
        instructions: Some(instructions)
    };
    // Option<InputCexDescription>  = output_sets.get("input_cex").and_then(|v| serde_json::from_value(v.clone()).ok());
    res.input_cex = Some(input_cex_description);
    Ok(res)
}

//https://stackoverflow.com/questions/30510764/returning-a-string-from-rust-function-to-python
#[no_mangle]
pub extern "C" fn ffi_find_invariant(output_sets_path: *const c_char, regex_config_path: *const c_char,
    bex_multiplier: c_ulonglong, predicate_base_cost: c_ulonglong
    ) -> *const c_char {
    let output_sets_path: &str = unsafe { std::ffi::CStr::from_ptr(output_sets_path).to_str().unwrap() };
    let regex_config_path = unsafe { std::ffi::CStr::from_ptr(regex_config_path).to_str().unwrap() };
    println!("Finding invariant for output sets: {}, regex config: {}, bex multiplier {}", output_sets_path, regex_config_path, bex_multiplier);
    let res = search_invariants(
        &output_sets_path,
        &regex_config_path,
        bex_multiplier as usize,
        predicate_base_cost as usize
    );
    match res {
        Ok(inner) => {
            let json_string_res = serde_json::to_string(&inner);
            let json_string = match json_string_res {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error while serializing invariant: {}", e);
                    return std::ptr::null();
                }
            };
            let s = CString::new(json_string).unwrap();
            s.into_raw()
        },
        Err(e) => {
            eprintln!("Error while finding invariant: {}", e);
            return std::ptr::null();
        }
    }
}

// #[no_mangle]
// pub extern "C" fn ffi_set_predicate_base_cost(new_cost: c_ulonglong) {
//     println!("Setting predicate base cost to {}", new_cost);
//     costs::set_predicate_base_cost(new_cost as usize);
// }

// #[no_mangle]
// pub extern "C" fn ffi_set_bex_multiplier(new_multiplier: c_ulonglong) {
//     println!("Setting bex multiplier to {}", new_multiplier);
//     costs::set_bex_multiplier(new_multiplier as usize);
// }

//https://stackoverflow.com/questions/30510764/returning-a-string-from-rust-function-to-python
#[no_mangle]
pub extern "C" fn ffi_free_library_string(c: *mut c_char) {
    // convert the pointer back to `CString`
    // it will be automatically dropped immediately
    unsafe { drop(CString::from_raw(c)); }
}

/// Formats the sum of two numbers as string.
#[no_mangle]
pub extern "C" fn ffi_check_on_waveform(path: *const c_char, invariant_json_path: *const c_char, clock_signal: *const c_char) -> bool {
    let path = unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap() };
    let invariant_json_path = unsafe { std::ffi::CStr::from_ptr(invariant_json_path).to_str().unwrap() };
    let clock_signal = unsafe { std::ffi::CStr::from_ptr(clock_signal).to_str().unwrap() };
    return utils::load_json_and_check_on_waveform(invariant_json_path, path, clock_signal).unwrap();
}

#[no_mangle]
pub extern "C" fn get_invariant_objects(
    invariants_directory: *const c_char,
) -> *const c_char {
    let invariants_directory = unsafe { std::ffi::CStr::from_ptr(invariants_directory).to_str().unwrap() };
    let mut invariants: Vec<predicates::SeparatorFormulaWithPath> = Vec::new();
    for entry in std::fs::read_dir(invariants_directory).unwrap() {
        let entry = entry.unwrap();
        println!("Reading invariant file: {:?}", entry.path());
        if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
            let separator_formula = utils::get_separator_formula_from_json_file(entry.path().to_str().unwrap()).unwrap();
            let invariant = predicates::SeparatorFormulaWithPath {
                separator_formula: separator_formula,
                path: entry.path().as_path().to_str().unwrap().to_string(),
            };
            invariants.push(invariant);
        }
    }
    let json_string = serde_json::to_string(&invariants).unwrap();
    let s = CString::new(json_string).unwrap();
    s.into_raw()
}


#[no_mangle]
pub extern "C" fn check_any_invariant_fulfilled_on_waveform_from_json_string(
    waveform_path: *const c_char,
    invariant_dict_json_str: *const c_char,
    clock_signal: *const c_char,
    short_circuit: bool
) -> i64 { //Return the index of the invariant that is fulfilled, or -1 if none is fulfilled
    // let start_time = std::time::Instant::now();
    let waveform_path = unsafe { std::ffi::CStr::from_ptr(waveform_path).to_str().unwrap() };
    let invariant_dict_json_str = unsafe { std::ffi::CStr::from_ptr(invariant_dict_json_str).to_str().unwrap() };
    let clock_signal = unsafe { std::ffi::CStr::from_ptr(clock_signal).to_str().unwrap() };
    // println!("Time to convert strings to &str: {:?}", start_time.elapsed());
    let invariants_with_path: Vec<predicates::SeparatorFormulaWithPath> = serde_json::from_str(invariant_dict_json_str).unwrap();
    let invariants = invariants_with_path.iter().map(|inv| &inv.separator_formula).collect::<Vec<&predicates::SeparatorFormula>>();
    // println!("Time to load invariants from directory: {:?}", start_time.elapsed());
    let signal_filter_regex = invariants_with_path.iter().flat_map(|inv| inv.separator_formula.get_relevant_signals())
        .map(|s| regex::Regex::new(&format!("^{}$", regex::escape(&s))).unwrap())
        .collect::<Vec<regex::Regex>>();
    // println!("Signal filters regex {:?}", signal_filter_regex);
    // println!("Time to create signal filter regex: {:?}", start_time.elapsed());
    let signal_filter = data_types::signal_filters::SignalFilter::RegexFilter(signal_filter_regex);
    let all_filters = data_types::signal_filters::SignalFilters {
        filters: vec![signal_filter.clone()],
    };
    // println!("Time to create all signal filters: {:?}", start_time.elapsed());
    let res = utils::check_any_separator_formula_fulfilled_on_waveform(waveform_path,&invariants, clock_signal, short_circuit,Some(&all_filters));
    match res {
        Some(idx) => return idx as i64,
        None => return -1
    }
    // println!("Total time to check invariants on waveform: {:?}", start_time.elapsed());
}


#[no_mangle]
pub extern "C" fn check_any_invariant_fulfilled_on_waveform(
    waveform_path: *const c_char,
    invariants_directory: *const c_char,
    clock_signal: *const c_char,
    short_circuit: bool
) -> i64 {
    // let _start_time = std::time::Instant::now();
    let waveform_path = unsafe { std::ffi::CStr::from_ptr(waveform_path).to_str().unwrap() };
    let invariants_directory = unsafe { std::ffi::CStr::from_ptr(invariants_directory).to_str().unwrap() };
    let clock_signal = unsafe { std::ffi::CStr::from_ptr(clock_signal).to_str().unwrap() };
    // println!("Time to convert strings to &str: {:?}", start_time.elapsed());
    let invariants: Vec<predicates::SeparatorFormula> = utils::load_separator_formulas_from_directory(invariants_directory);
    if invariants.len() == 0 {
        println!("No invariants found in directory: {}", invariants_directory);
        return -1;
    }
    let invariants_ref = invariants.iter().collect::<Vec<&predicates::SeparatorFormula>>();
    // println!("Time to load invariants from directory: {:?}", start_time.elapsed());
     let signal_filter_regex = invariants.iter().flat_map(|inv| inv.get_relevant_signals())
        .map(|s| regex::Regex::new(&format!("^{}$", regex::escape(&s))).unwrap())
        .collect::<Vec<regex::Regex>>();
    // println!("Time to create signal filter regex: {:?}", start_time.elapsed());
    let signal_filter = data_types::signal_filters::SignalFilter::RegexFilter(signal_filter_regex);
    let all_filters = data_types::signal_filters::SignalFilters {
        filters: vec![signal_filter.clone()],
    };
    // println!("Time to create all signal filters: {:?}", start_time.elapsed());
    let res = utils::check_any_separator_formula_fulfilled_on_waveform(waveform_path,&invariants_ref, clock_signal, short_circuit,Some(&all_filters));
    // println!("Total time to check invariants on waveform: {:?}", start_time.elapsed());
    match res {
        Some(idx) => return idx as i64,
        None => return -1
    }
}


/*
 * Check CEX items against invariants
 * cex_items: JSON string of list of FuzzerDataPoint
 * invariants_directory: directory containing invariants in JSON format
 * Returns: JSON string of list of FuzzerDataPoint that satisfy the invariants
 */
#[no_mangle]
pub extern "C" fn check_cex_items_against_invariants(
    cex_items: *const c_char,
    invariants_directory: *const c_char,
) -> *const c_char {
    let cex_items = unsafe { std::ffi::CStr::from_ptr(cex_items).to_str().unwrap() };
    // let signal_name = unsafe { std::ffi::CStr::from_ptr(signal_name).to_str().unwrap() };
    let items: Vec<FuzzerDataPoint> = serde_json::from_str(cex_items).unwrap();
    let invariants_directory = unsafe { std::ffi::CStr::from_ptr(invariants_directory).to_str().unwrap() };
    let mut invariants: Vec<predicates::SeparatorFormula> = Vec::new();
    for entry in std::fs::read_dir(invariants_directory).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
            let invariant = utils::get_separator_formula_from_json_file(entry.path().to_str().unwrap()).unwrap();
            invariants.push(invariant);
        }
    }
    // return

    let mut filtered_items: Vec<&FuzzerDataPoint> = Vec::new();
    for (item, idx) in utils::check_cex_items_against_invariants(&items, &invariants, true) {
        if idx.len() > 0 {
            filtered_items.push(item);
        }
    }
    let json_string = serde_json::to_string(&filtered_items).unwrap();
    let s = CString::new(json_string).unwrap();
    s.into_raw()
}


/*
 * Check CEX items against invariants
 * cex_items: JSON string of list of FuzzerDataPoint
 * invariants_directory: directory containing invariants in JSON format
 * Returns: JSON string of list of FuzzerDataPoint that satisfy the invariants
 */
#[no_mangle]
pub extern "C" fn check_cex_items_all_invariants(
    cex_items: *const c_char,
    invariant_dict_json_str: *const c_char,
) -> *const c_char {
    let cex_items = unsafe { std::ffi::CStr::from_ptr(cex_items).to_str().unwrap() };
    let invariant_dict_json_str = unsafe { std::ffi::CStr::from_ptr(invariant_dict_json_str).to_str().unwrap() };
    
    // println!("Time to convert strings to &str: {:?}", start_time.elapsed());
    let invariants_with_path: Vec<predicates::SeparatorFormulaWithPath> = serde_json::from_str(invariant_dict_json_str).unwrap();
    let invariants = invariants_with_path.iter().map(|inv| &inv.separator_formula).cloned().collect::<Vec<predicates::SeparatorFormula>>();
    println!("Loaded {} invariants", invariants.len());
    for inv in invariants.iter() {
        println!("Invariant: {}", inv);
    }
    // let signal_name = unsafe { std::ffi::CStr::from_ptr(signal_name).to_str().unwrap() };
    let items: Vec<FuzzerDataPoint> = serde_json::from_str(cex_items).unwrap();
    // return
    // println!("Checking {} items against {} invariants", items.len(), invariants.len());
    // for item in items.iter() {
    //     println!("Item: {:?}", item);
    // }
    let mut res: Vec<(FuzzerDataPoint, Vec<String>)> = Vec::new();
    for (item, idx) in utils::check_cex_items_against_invariants(&items, &invariants, false) {
        // for this_idx in idx {

        //     res.push((item.clone(), invariants[this_idx].to_string()));
        // }
        // if item.file == "evaluation_data/dedup/testcases/reduced-testcases-k1-k4-k5/testcase-k1-1259.bin" {
        //     println!("Item {:?} satisfies invariants at indices {:?}", item, idx);
        // }
        let invariant_strings: Vec<String> = idx.iter().map(|i| invariants[*i].to_string()).collect();
        res.push((item.clone(), invariant_strings));
    }
    //let filtered_items: Vec<&FuzzerDataPoint> = utils::check_cex_items_against_invariants(&items, &invariants);
    let json_string = serde_json::to_string(&res).unwrap();
    let s = CString::new(json_string).unwrap();
    s.into_raw()
}
