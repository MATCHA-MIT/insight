use clap;
use clap::Parser;
use env_logger;
use invariant_finder_rust::constants;
use invariant_finder_rust::search_invariants;
use log;
use std::error::Error;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::Ordering;
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
    bex_multiplier: usize,

    #[clap(long, value_parser, default_value = "benchmark_results.csv")]
    output_csv: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    env_logger::init();

    // Define benchmark ranges
    // These steps can be adjusted as preferred
    let control_pred_steps = vec![10, 30, 50, 70, 100, 150, 200, 500];
    let cex_limit_steps = vec![5, 10, 20, 50, 100, 200];
    let default_bex_limit = 15000;

    println!("preds,cex_limit,bex_limit,actual_cex,actual_bex,ilp_states,total_predicates,time_ms");

    let mut csv_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&args.output_csv)?;
    writeln!(
        csv_file,
        "preds,cex_limit,bex_limit,actual_cex,actual_bex,ilp_states,total_predicates,time_ms"
    )?;
    csv_file.flush()?;

    for &preds in &control_pred_steps {
        // Update predicate collection limits atomically through one control block.
        constants::PREDICATE_TAKE_LIMITS.store_control_predicates(preds, Ordering::Relaxed);
        constants::PREDICATE_TAKE_LIMITS.store_regular_predicates(preds, Ordering::Relaxed);

        for &cex_lim in &cex_limit_steps {
            // Update the limits
            constants::ATOMIC_MAX_NUM_CEX.store(cex_lim, Ordering::Relaxed);
            constants::ATOMIC_MAX_NUM_BEX.store(default_bex_limit, Ordering::Relaxed);

            // Reset counters for safety (though they are overwritten)
            constants::BENCHMARK_NUM_CEX.store(0, Ordering::Relaxed);
            constants::BENCHMARK_NUM_BEX.store(0, Ordering::Relaxed);
            constants::BENCHMARK_ILP_STATES.store(0, Ordering::Relaxed);
            constants::BENCHMARK_NUM_COLLECTED_PREDICATES.store(0, Ordering::Relaxed);

            let start = std::time::Instant::now();
            let res = search_invariants(
                &args.output_sets,
                &args.regex_config,
                args.bex_multiplier,
                args.predicate_base_cost,
            );
            let duration = start.elapsed().as_millis();

            // Collect metrics after run
            let actual_cex = constants::BENCHMARK_NUM_CEX.load(Ordering::Relaxed);
            let actual_bex = constants::BENCHMARK_NUM_BEX.load(Ordering::Relaxed);
            let ilp_states = constants::BENCHMARK_ILP_STATES.load(Ordering::Relaxed);
            let total_predicates =
                constants::BENCHMARK_NUM_COLLECTED_PREDICATES.load(Ordering::Relaxed);

            println!(
                "{},{},{},{},{},{},{},{}",
                preds,
                cex_lim,
                default_bex_limit,
                actual_cex,
                actual_bex,
                ilp_states,
                total_predicates,
                duration
            );
            writeln!(
                csv_file,
                "{},{},{},{},{},{},{},{}",
                preds,
                cex_lim,
                default_bex_limit,
                actual_cex,
                actual_bex,
                ilp_states,
                total_predicates,
                duration
            )?;
            csv_file.flush()?;

            // If run failed, log error
            if let Err(e) = res {
                eprintln!(
                    "Error during benchmark iteration preds={} cex={}: {}",
                    preds, cex_lim, e
                );
            }
        }
    }

    Result::Ok(())
}
