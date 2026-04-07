use clap;
use clap::Parser;
use std::error::Error;
use invariant_finder_rust::invariant_searcher;
use invariant_finder_rust::predicates;
use invariant_finder_rust;


/// Command-line argument parser
#[derive(clap::Parser, Debug)]
#[command(name = "My CLAP Program")]
#[command(about = "Parses command-line arguments using clap", long_about = None)]
struct Cli {
    #[clap(long, value_parser)]
    output_sets: String,

    #[clap(long, value_parser)]
    regex_config: String,

    #[clap(long, value_parser, default_value = None)]
    invariant_path: Option<String>,

    #[clap(long, value_parser, default_value_t = 10)]
    predicate_base_cost: usize,

    #[clap(long, value_parser, default_value_t = 35)]
    bex_multiplier: usize
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    // costs::set_predicate_base_cost(args.predicate_base_cost);
    // costs::set_bex_multiplier(args.bex_multiplier);
    //utils::print_z3_version();

    /*let res = search_invariants(
        &args.output_sets,
        &args.regex_config);
    */
    let invariant_path = args.invariant_path.unwrap_or("invariant.json".to_string());
    let file = std::fs::File::open(&invariant_path)?;
    let reader = std::io::BufReader::new(file);
    // let mut search_result: invariant_searcher::SearchResult = serde_json::from_reader(reader)?;
    let mut separator_formula: predicates::SeparatorFormula = {
        if let Ok(json_value) = serde_json::from_reader::<_, serde_json::Value>(reader) {
            if let Some(sep_value) = json_value.get("separator_formula") {
                if let Ok(inner_separator) = serde_json::from_value(sep_value.clone()) {
                    inner_separator
                } else {
                    panic!("Failed to extract separator_formula from json file {}", invariant_path);
                }
            } else if let Some(inv_value) = json_value.get("invariant") {
                if let Ok(inner_invariant) = serde_json::from_value(inv_value.clone()) {
                    predicates::SeparatorFormula::Invariant(inner_invariant)
                } else {
                    panic!("Failed to extract invariant from json file {}", invariant_path);
                }
            } else {
                panic!("No separator_formula or invariant field in json file {}", invariant_path);
            }
        } else {
            panic!("Failed to parse json file {}", invariant_path);
        }
    };
    let (invariant_seacher_obj, regex_stages, bug_type) = invariant_finder_rust::create_searcher_from_config(
        &args.output_sets,
        &args.regex_config,
        args.bex_multiplier,
        args.predicate_base_cost
    )?;
    invariant_seacher_obj.debug_formula(
        &mut separator_formula,
        regex_stages,
        bug_type,
    );
    //println!("Search result {:?}", search_result);

    /*
    match res {
        Ok(inner) => {
            let json = serde_json::to_string_pretty(&inner)?;
            println!("Invariant {}", json);
            if let Some(invariant_out_path) = &args.invariant_out_path {
                let mut file = File::create(invariant_out_path).expect(format!("Creating file {invariant_out_path} failed!").as_str());
                file.write_all(json.as_bytes())?;
            }
        }, 
        Err(e) => {
            eprintln!("Error with search_invariants function: {}", e);
        }
    }
     */
    Result::Ok(())
}