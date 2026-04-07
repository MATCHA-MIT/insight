//extern crate invariant_finder_rust;
use invariant_finder_rust::waveform;
use invariant_finder_rust::cycle_types::CycleCountConversion;
use std::time::Instant;

#[test]
fn waveform_loaded() {
    //The waveform implementation relies on a HashMap, which is random in the order the items are received (the hasher is initialized with a random seed).
    //This test is to make sure that the waveform is loaded correctly, independent of that random seed
    //the hashmap is seeded different for every call to ::new(), see https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=e47cdc9a66fb854f1287528e43bc87aa, 
    //https://www.reddit.com/r/rust/comments/1fbvrws/is_the_default_hash_function_for_the_hashmap/
    // for _ in 0..10 {
    let start = Instant::now();
    let _waveform = waveform::WaveForm::load_waveform_and_cycle_map("tests/testfiles/instructions_265.s.bin.vcd", "TOP.correctness.clk", None).unwrap();
    let duration = start.elapsed();
    println!("Time elapsed in waveform loading() is: {:?}", duration);
        //     let value = waveform.get_signal_value_at_cycle("TOP.correctness.sodor_core.core.d.wb_addr", 0).unwrap();
    //     assert_eq!(value, 31);
    //     let value = waveform.get_signal_value_at_cycle("TOP.correctness.sodor_core.core.d.wb_addr", 1).unwrap();
    //     assert_eq!(value, 5);
    // }
    // for _ in 0..10 {
    let start = Instant::now();
        let waveform = waveform::WaveForm::load_waveform_and_cycle_map("tests/testfiles/instructions_265.s.bin.fst", "TOP.correctness.clk", None).unwrap();
        let duration = start.elapsed();
        println!("Time elapsed in fst waveform loading() is: {:?}", duration);
        let value = waveform.get_signal_value_at_cycle("TOP.correctness.sodor_core.core.d.wb_addr", 0).unwrap();
        assert_eq!(value, 31);
        let value = waveform.get_signal_value_at_cycle("TOP.correctness.sodor_core.core.d.wb_addr", 1).unwrap();
        assert_eq!(value, 5);
    // }
    for cycle in 0..waveform.num_cycles {
        let value = waveform.get_signal_value_at_cycle("TOP.correctness.counter", cycle);
        assert_eq!(value.unwrap() as u64, u64::from_cycle_count(cycle));
    }
    // println!("Waveform num constant signals {:?}", waveform.constant_signals.len());


    // let waveform = waveform::WaveForm::load_waveform_and_cycle_map("tests/testfiles/sharp_cutoff.vcd", "TOP.correctness.clk", None).unwrap();
    // for cycle in 0..waveform.num_cycles {
    //     let value = waveform.get_signal_value_at_cycle("TOP.correctness.counter", cycle);
    //     assert_eq!(value.unwrap() as u64, u64::from_cycle_count(cycle));
    // }
    
    //assert_eq!(waveform.num_cycles, 19);
    //let _ = waveform.print_signal_to_cycle();
    /*println!("clk_period {:?}", waveform.clk_period);
    println!("TOP.correctness.sodor_core.core.d.wb_addr {:#?}", waveform.get_timestamp_to_value("TOP.correctness.sodor_core.core.d.wb_addr"));
    println!("TOP.correctness.sodor_core.core.d.csr.io_rw_addr {:#?}", waveform.get_timestamp_to_value("TOP.correctness.sodor_core.core.d.csr.io_rw_addr"));
    println!("TOP.counter {:#?}", waveform.get_timestamp_to_value("TOP.correctness.counter"));
    println!("get_signal_value_at_cycle 0x{:?}", );
    println!("get_signal_value_at_cycle 0x{:x}", waveform.get_signal_value_at_cycle("TOP.correctness.sodor_core.core.d.csr.io_rw_addr", 1).unwrap());
    println!("get_signal_value_at_cycle 0x{:x}", waveform.get_signal_value_at_cycle("TOP.correctness.sodor_core.core.d.opcode", 1).unwrap());
    */

}