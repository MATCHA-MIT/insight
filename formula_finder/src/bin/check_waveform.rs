use clap::{CommandFactory, Parser};
use clap_complete::{generate, shells::Bash};
use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use invariant_finder_rust::utils;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the waveform file
    /// Path to the waveform files
    #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
    waveform_paths: Vec<PathBuf>,
    /// Path to the invariant json file
    #[arg(short, long)]
    invariant_json: PathBuf,
}

pub fn _autocomplete() {
    if let Ok(shell) = env::var("SHELL") {
        println!("Generating autocomplete script");
        if shell.contains("bash") {
            let mut app = Args::command();
            let out_dir = env::var_os("OUT_DIR").unwrap();
            let dest_path = PathBuf::from(out_dir).join("autocomplete.sh");
            let file = File::create(dest_path).unwrap();
            let mut buf = BufWriter::new(file);
            generate(Bash, &mut app, "check_waveform", &mut buf);
        }
    }
}

fn main() {
    let args = Args::parse();
    let clock_signal = "TOP.correctness.clk";
    let waveform_paths = args.waveform_paths;
    if waveform_paths.is_empty() {
        eprintln!("No files found matching pattern");
        std::process::exit(1);
    }
            
    // let file = std::fs::File::open(&args.invariant_json).expect("Failed to open invariant json file");
    // let reader = BufReader::new(file);
    // let search_result: serde_json::Value = serde_json::from_reader(reader).expect("Failed to parse invariant json file");
    // let invariant: predicates::Invariant = search_result.get("invariant")
    //     .and_then(|v| serde_json::from_value(v.clone()).ok())
    //     .expect("Failed to extract invariant from json");
    let separator_formula = utils::get_separator_formula_from_json_file(&args.invariant_json.as_path().to_str().unwrap())
        .expect("Failed to extract separator formula from json");

    for waveform_path in waveform_paths {
        println!("Checking waveform: {}", waveform_path.display());
        let res = utils::check_on_waveform(waveform_path.to_string_lossy().as_ref(), &separator_formula, clock_signal, false, None).unwrap();
        println!("Checked waveform: {}, res {}", waveform_path.display(), res);
    }
}