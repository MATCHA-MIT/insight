use clap::Parser;
use invariant_finder_rust::waveform;
use invariant_finder_rust::cycle_types::CycleCountConversion;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(index = 1)]
    waveform_path: String,
    #[arg(long, default_value = "TOP.correctness.clk")]
    clock_signal: String,
}

fn main() {
    println!("Printing waveform values");
    let args = Args::parse();
    let clock_signal = &args.clock_signal; //"TOP.correctness.clk";
    println!("Using clock signal: {}", clock_signal);
    let waveform_path = args.waveform_path;
    let waveform: waveform::WaveForm =
        waveform::WaveForm::load_waveform_and_cycle_map(waveform_path.as_str(), clock_signal, None)
            .unwrap();
    for (signal, code) in waveform.name_to_code.iter() {
        let mut values = Vec::new();
        for cycle in 0..waveform.num_cycles {
            values.push(waveform.get_signal_value_at_cycle(signal, cycle));
        }
        //Format each value in values as hexstring or None
        let values_formatted = values.iter().map(|v| match v {
            Some(val) => format!("{:X}", val),
            None => "None".to_string(),
        }).collect::<Vec<String>>().join(", "); //format each value as hex string or None
        //Print signal name, code, and values
        println!("Signal {:?}, Idx {:?}, values {:?}", signal, code, values_formatted);
    }
    // let mismatch_cycle_ref_core = waveform.get_mismatch_ref_core();
    // match mismatch_cycle_ref_core {
    //     Some(mismatch_cycle_ref_core) => {
    //         println!(
    //             "Signal values at mismatch ref core: {:?}",
    //             mismatch_cycle_ref_core
    //         );
    //         for (signal, _) in waveform.name_to_code.iter() {
    //             let value = waveform.get_signal_value_at_cycle(signal, mismatch_cycle_ref_core.to_cycle_count());
    //             println!("Signal {:?} value {:?}", signal, value);
    //         }
    //     }
    //     None => {
    //         println!("No mismatch cycle found in ref core");
    //     }
    // }
    // let mismatch_cycle_dut_core: Option<u64> = waveform.get_first_mismatch_dut_core();
    // println!("DUT core mismatch cycle: {:?}", mismatch_cycle_dut_core);
    // match mismatch_cycle_dut_core {
    //     Some(mismatch_cycle_dut_core) => {
    //         println!(
    //             "Signal values at mismatch dut core: {:?}",
    //             mismatch_cycle_dut_core
    //         );
    //         for (signal, _) in waveform.name_to_code.iter() {
    //             let value = waveform.get_signal_value_at_cycle(signal, mismatch_cycle_dut_core.to_cycle_count());
    //             println!("Signal {:?} value {:?}", signal, value);
    //         }
    //     }
    //     None => {
    //         println!("No mismatch cycle found in dut core");
    //     }
    // }
    // let mismatch_cycle_ref_core = waveform.get_mismatch_ref_core();
    // println!("ref core mismatch cycle: {:?}", mismatch_cycle_ref_core);
    // if let Some(cycle) = mismatch_cycle_ref_core {
    //     if cycle == 0 {
    //         println!("Mismatch at cycle 0, no previous cycle to show");
    //         return;
    //     }
    // }
    // match mismatch_cycle_ref_core {
    //     Some(mismatch_cycle_ref_core) => {
    //         for (signal, _) in waveform.name_to_code.iter() {
    //             let value = waveform.get_signal_value_at_cycle(signal, mismatch_cycle_ref_core.to_cycle_count()-1);
    //             println!("Signal {:?} value {:?}", signal, value);
    //         }
    //     } 
    //     None => {
    //         println!("No mismatch cycle found in ref core");
    //     }
    // }



    // println!("Clock signal aliases {:?}", waveform.get_signal_aliases(&clock_signal));
}
