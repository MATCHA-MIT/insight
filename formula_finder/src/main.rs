use clap;
use clap::Parser;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use invariant_finder_rust::search_invariants;
pub mod architecture_checker;
pub mod invariant_searcher;
pub mod predicates;
pub mod smt_solver;
pub mod teacher;
pub mod waveform;
pub mod constants;
pub mod data_types;
pub mod utils;
pub mod decision_tree;
pub mod cycle_types;
pub mod allowed_signal_config;  
pub mod set_cover_preprocessing;
#[cfg(feature = "ilp")]
pub mod ilp_solver;
pub mod set_cover_solver;
pub mod solver;
pub mod jg_formatter;
pub mod costs;
pub mod set_cover_instance;
pub mod set_cover_heuristic;
use env_logger;
use log;
//use jemallocator;
//#[global_allocator]
//static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;



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
    invariant_out_path: Option<String>,

    #[clap(long, value_parser, default_value_t = 10)]
    predicate_base_cost: usize,

    #[clap(long, value_parser, default_value_t = 35)]
    bex_multiplier: usize
}

fn main() -> Result<(), Box<dyn Error>> {
//    utils::print_z3_version();
    let args = Cli::parse();
    // costs::set_predicate_base_cost(args.predicate_base_cost);
    // costs::set_bex_multiplier(args.bex_multiplier);
    // println!(
    //     "Using predicate base cost {} and bex weight {}",
    //     costs::get_predicate_base_cost(),
    //     costs::get_bex_multiplier()
    // );
    env_logger::init(); // Initializes the logger based on RUST_LOG environment variable
    log::debug!("Debug logging is enabled.");
    let res = search_invariants(
        &args.output_sets,
        &args.regex_config,
        args.bex_multiplier,
        args.predicate_base_cost
    );
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
    Result::Ok(())
}
