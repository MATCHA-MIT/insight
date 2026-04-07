use clap::Parser;
use invariant_finder_rust::waveform;
use invariant_finder_rust::cycle_types::CycleCount;
use invariant_finder_rust::data_types::signal_filters::{SignalFilter, SignalFilters};
use regex::Regex;

const CLOCK_SIGNAL: &str = "TOP.correctness.clk";

// Reference core signals
const REF_COMMIT_VALID_SIGNAL: &str = "TOP.correctness.ref_commit.commit_valid";
const REF_COMMIT_RD_SIGNAL: &str = "TOP.correctness.ref_commit.commit_data";
const REF_COMMIT_RS1_SIGNAL: &str = "TOP.correctness.ref_commit.commit_rs1";
const REF_COMMIT_RS2_SIGNAL: &str = "TOP.correctness.ref_commit.commit_rs2";
const REF_COMMIT_PC_SIGNAL: &str = "TOP.correctness.ref_commit.commit_pc";
const REF_COMMIT_NEXT_PC_SIGNAL: &str = "TOP.correctness.ref_commit.commit_next_pc";
const REF_COMMIT_INST_SIGNAL: &str = "TOP.correctness.ref_commit.commit_instr";
const REF_COMMIT_EXCEPTION_SIGNAL: &str = "TOP.correctness.ref_commit.commit_exception_code";

// DUT core signals
const DUT_COMMIT_VALID_SIGNAL: &str = "TOP.correctness.dut_commit.commit_valid";
const DUT_COMMIT_RD_SIGNAL: &str = "TOP.correctness.dut_commit.commit_data";
const DUT_COMMIT_RS1_SIGNAL: &str = "TOP.correctness.dut_commit.commit_rs1";
const DUT_COMMIT_RS2_SIGNAL: &str = "TOP.correctness.dut_commit.commit_rs2";
const DUT_COMMIT_PC_SIGNAL: &str = "TOP.correctness.dut_commit.commit_pc";
const DUT_COMMIT_NEXT_PC_SIGNAL: &str = "TOP.correctness.dut_commit.commit_next_pc";
const DUT_COMMIT_INST_SIGNAL: &str = "TOP.correctness.dut_commit.commit_instr";
const DUT_COMMIT_EXCEPTION_SIGNAL: &str = "TOP.correctness.dut_commit.commit_exception_code";

#[derive(Debug, Clone)]
struct CommitEntry {
	cycle: CycleCount,
	rd: i64,
	rs1: i64,
	rs2: i64,
	pc: i64,
	next_pc: i64,
	inst: i64,
    exception: i64,
}

#[derive(Debug)]
struct CommitDiff {
	index: usize,
	ref_entry: Option<CommitEntry>,
	dut_entry: Option<CommitEntry>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
	#[arg(index = 1)]
	waveform_path: String,
	#[arg(long, default_value = CLOCK_SIGNAL)]
	clock_signal: String,
	#[arg(long, default_value_t = 0)]
	commit_index: usize,
}

fn signal_value_or_panic(waveform: &waveform::WaveForm, signal: &str, cycle: CycleCount) -> i64 {
	waveform
		.get_signal_value_at_cycle(signal, cycle)
		.unwrap_or_else(|| panic!("Signal {} not found at cycle {}", signal, cycle))
}

fn extract_commit_log(
	waveform: &waveform::WaveForm,
	commit_valid_signal: &str,
	rd_signal: &str,
	rs1_signal: &str,
	rs2_signal: &str,
	pc_signal: &str,
	next_pc_signal: &str,
	inst_signal: &str,
    exception_signal: &str,
) -> Vec<CommitEntry> {
	let mut entries = Vec::new();
	for cycle in 0..waveform.num_cycles {
		let commit_valid = signal_value_or_panic(waveform, commit_valid_signal, cycle);
		if commit_valid != 0 {
			let entry = CommitEntry {
				cycle,
				rd: signal_value_or_panic(waveform, rd_signal, cycle),
				rs1: signal_value_or_panic(waveform, rs1_signal, cycle),
				rs2: signal_value_or_panic(waveform, rs2_signal, cycle),
				pc: signal_value_or_panic(waveform, pc_signal, cycle),
				next_pc: signal_value_or_panic(waveform, next_pc_signal, cycle),
				inst: signal_value_or_panic(waveform, inst_signal, cycle),
                exception: signal_value_or_panic(waveform, exception_signal, cycle),
			};
			entries.push(entry);
		}
	}
	entries
}


fn which_fields_used_by_instr(inst: i64) -> (bool, bool, bool) {
	// Decode the instruction, return which fields are used (rd, rs1, rs2)
	let opcode = (inst & 0x7f) as i64;
    let rd = ((inst >> 7) & 0x1f) as i64;
    let rd_non_zero = rd != 0;
	match opcode {
		0x33 => (rd_non_zero, true, true),  // R-type
		0x13 => (rd_non_zero, true, false), // I-type arithmetic
		0x03 => (rd_non_zero, true, false), // Loads
		0x23 => (false, true, true), // Stores
		0x63 => (false, true, true), // Branches
		0x6f => (rd_non_zero, false, false), // JAL
		0x67 => (rd_non_zero, true, false),  // JALR
		0x37 => (rd_non_zero, false, false), // LUI
		0x17 => (rd_non_zero, false, false), // AUIPC
		0x0f => (false, false, false), // FENCE
		0x2f => (rd_non_zero, true, true),   // AMO
		0x73 => {
			let funct3 = ((inst >> 12) & 0x7) as i64;
			if funct3 == 0 {
				(false, false, false) // ECALL/EBREAK
			} else {
				(rd_non_zero, true, false) // CSR: rd and rs1 (or zimm)
			}
		}
		_ => (false, false, false),
	}
}

fn print_whole_commit_log(ref_log: &[CommitEntry], dut_log: &[CommitEntry]) {
    let max_len = ref_log.len().max(dut_log.len());
    for index in 0..max_len {
        if let Some(ref_entry) = ref_log.get(index) {
            println!("ref commit[{}]: {}", index, format_entry(ref_entry));
        } else {
            println!("ref commit[{}]: <missing>", index);
        }
        if let Some(dut_entry) = dut_log.get(index) {
            println!("dut commit[{}]: {}", index, format_entry(dut_entry));
        } else {
            println!("dut commit[{}]: <missing>", index);
        }
    }
}

fn compare_commit_logs(ref_log: &[CommitEntry], dut_log: &[CommitEntry]) -> Vec<CommitDiff> {
	let max_len = ref_log.len().max(dut_log.len());
	let mut diffs = Vec::new();
	for index in 0..max_len {
		let ref_entry = ref_log.get(index).cloned();
		let dut_entry = dut_log.get(index).cloned();
		let mismatch = match (&ref_entry, &dut_entry) {
			(Some(r), Some(d)) => {
				let (rd_used, rs1_used, rs2_used) = which_fields_used_by_instr(r.inst);
				(rd_used && r.rd != d.rd)
					|| (rs1_used && r.rs1 != d.rs1)
					|| (rs2_used && r.rs2 != d.rs2)
					|| (r.pc != d.pc)
					|| (r.next_pc != d.next_pc)
                    || (r.inst != d.inst)
                    || (r.exception != d.exception)
			}
			_ => true,
		};
		if mismatch {
			diffs.push(CommitDiff {
				index,
				ref_entry,
				dut_entry,
			});
		}
	}
	diffs
}

fn format_entry(entry: &CommitEntry) -> String {
	format!(
		"cycle={} rd=0x{:x} rs1=0x{:x} rs2=0x{:x} pc=0x{:x} next_pc=0x{:x} inst=0x{:08x}({}) (exception: 0x{:x})",
			entry.cycle,
			entry.rd,
			entry.rs1,
			entry.rs2,
			entry.pc,
			entry.next_pc,
			entry.inst,
			disassemble_instruction(entry.inst),
			entry.exception
	)
}

fn disassemble_instruction(inst: i64) -> String {
    //REturn a string formatting the instruction in a human-readable way
    //Format should be like "ADD x1, x2, x3" or "LW x1, 0(x2)"
    //If the instruction is unknown, return "UNKNOWN"
		let opcode = (inst & 0x7f) as i64;
		let rd = ((inst >> 7) & 0x1f) as i64;
		let funct3 = ((inst >> 12) & 0x7) as i64;
		let rs1 = ((inst >> 15) & 0x1f) as i64;
		let rs2 = ((inst >> 20) & 0x1f) as i64;
		let funct7 = ((inst >> 25) & 0x7f) as i64;

		let imm_i = ((inst as i64) >> 20) as i64;
		let imm_s = (((inst >> 7) & 0x1f) | (((inst >> 25) & 0x7f) << 5)) as i64;
		let imm_s = ((imm_s << 20) >> 20) as i64;
		let imm_b = (
			(((inst >> 8) & 0x0f) << 1)
				| (((inst >> 25) & 0x3f) << 5)
				| (((inst >> 7) & 0x01) << 11)
				| (((inst >> 31) & 0x01) << 12)
		) as i64;
		let imm_b = ((imm_b << 19) >> 19) as i64;
		let imm_u = ((inst as i64) & 0xfffff000) as i64;
		let imm_j = (
			(((inst >> 21) & 0x03ff) << 1)
				| (((inst >> 20) & 0x01) << 11)
				| (((inst >> 12) & 0x0ff) << 12)
				| (((inst >> 31) & 0x01) << 20)
		) as i64;
		let imm_j = ((imm_j << 11) >> 11) as i64;

		match opcode {
			0x33 => match (funct3, funct7) {
				(0x0, 0x00) => format!("ADD x{}, x{}, x{}", rd, rs1, rs2),
				(0x0, 0x20) => format!("SUB x{}, x{}, x{}", rd, rs1, rs2),
				(0x7, 0x00) => format!("AND x{}, x{}, x{}", rd, rs1, rs2),
				(0x6, 0x00) => format!("OR x{}, x{}, x{}", rd, rs1, rs2),
				(0x4, 0x00) => format!("XOR x{}, x{}, x{}", rd, rs1, rs2),
				(0x1, 0x00) => format!("SLL x{}, x{}, x{}", rd, rs1, rs2),
				(0x5, 0x00) => format!("SRL x{}, x{}, x{}", rd, rs1, rs2),
				(0x5, 0x20) => format!("SRA x{}, x{}, x{}", rd, rs1, rs2),
				(0x2, 0x00) => format!("SLT x{}, x{}, x{}", rd, rs1, rs2),
				(0x3, 0x00) => format!("SLTU x{}, x{}, x{}", rd, rs1, rs2),
				_ => "UNKNOWN".to_string(),
			},
			0x13 => match funct3 {
				0x0 => format!("ADDI x{}, x{}, {}", rd, rs1, imm_i),
				0x7 => format!("ANDI x{}, x{}, {}", rd, rs1, imm_i),
				0x6 => format!("ORI x{}, x{}, {}", rd, rs1, imm_i),
				0x4 => format!("XORI x{}, x{}, {}", rd, rs1, imm_i),
				0x2 => format!("SLTI x{}, x{}, {}", rd, rs1, imm_i),
				0x3 => format!("SLTIU x{}, x{}, {}", rd, rs1, imm_i),
				0x1 => {
					let shamt = (imm_i & 0x1f) as i64;
					format!("SLLI x{}, x{}, {}", rd, rs1, shamt)
				}
				0x5 => {
					let shamt = (imm_i & 0x1f) as i64;
					if funct7 == 0x20 {
						format!("SRAI x{}, x{}, {}", rd, rs1, shamt)
					} else {
						format!("SRLI x{}, x{}, {}", rd, rs1, shamt)
					}
				}
				_ => "UNKNOWN".to_string(),
			},
			0x03 => match funct3 {
				0x0 => format!("LB x{}, {}(x{})", rd, imm_i, rs1),
				0x1 => format!("LH x{}, {}(x{})", rd, imm_i, rs1),
				0x2 => format!("LW x{}, {}(x{})", rd, imm_i, rs1),
				0x4 => format!("LBU x{}, {}(x{})", rd, imm_i, rs1),
				0x5 => format!("LHU x{}, {}(x{})", rd, imm_i, rs1),
				_ => "UNKNOWN".to_string(),
			},
			0x23 => match funct3 {
				0x0 => format!("SB x{}, {}(x{})", rs2, imm_s, rs1),
				0x1 => format!("SH x{}, {}(x{})", rs2, imm_s, rs1),
				0x2 => format!("SW x{}, {}(x{})", rs2, imm_s, rs1),
				_ => "UNKNOWN".to_string(),
			},
			0x63 => match funct3 {
				0x0 => format!("BEQ x{}, x{}, {}", rs1, rs2, imm_b),
				0x1 => format!("BNE x{}, x{}, {}", rs1, rs2, imm_b),
				0x4 => format!("BLT x{}, x{}, {}", rs1, rs2, imm_b),
				0x5 => format!("BGE x{}, x{}, {}", rs1, rs2, imm_b),
				0x6 => format!("BLTU x{}, x{}, {}", rs1, rs2, imm_b),
				0x7 => format!("BGEU x{}, x{}, {}", rs1, rs2, imm_b),
				_ => "UNKNOWN".to_string(),
			},
			0x6f => format!("JAL x{}, {}", rd, imm_j),
			0x67 => format!("JALR x{}, {}(x{})", rd, imm_i, rs1),
			0x37 => format!("LUI x{}, {}", rd, imm_u),
			0x17 => format!("AUIPC x{}, {}", rd, imm_u),
			0x0f => "FENCE".to_string(),
			0x73 => match funct3 {
				0x0 => "SYSTEM".to_string(),
				0x1 => format!("CSRRW x{}, x{}, 0x{:x}", rd, rs1, imm_i & 0xfff),
				0x2 => format!("CSRRS x{}, x{}, 0x{:x}", rd, rs1, imm_i & 0xfff),
				0x3 => format!("CSRRC x{}, x{}, 0x{:x}", rd, rs1, imm_i & 0xfff),
				0x5 => format!("CSRRWI x{}, {}, 0x{:x}", rd, rs1, imm_i & 0xfff),
				0x6 => format!("CSRRSI x{}, {}, 0x{:x}", rd, rs1, imm_i & 0xfff),
				0x7 => format!("CSRRCI x{}, {}, 0x{:x}", rd, rs1, imm_i & 0xfff),
				_ => "UNKNOWN".to_string(),
			},
			_ => "UNKNOWN".to_string(),
		}
}

fn build_commit_signal_filters(clock_signal: &str) -> SignalFilters {
	let signals = [
		clock_signal,
		REF_COMMIT_VALID_SIGNAL,
		REF_COMMIT_RD_SIGNAL,
		REF_COMMIT_RS1_SIGNAL,
		REF_COMMIT_RS2_SIGNAL,
		REF_COMMIT_PC_SIGNAL,
		REF_COMMIT_NEXT_PC_SIGNAL,
		REF_COMMIT_INST_SIGNAL,
        REF_COMMIT_EXCEPTION_SIGNAL,
		DUT_COMMIT_VALID_SIGNAL,
		DUT_COMMIT_RD_SIGNAL,
		DUT_COMMIT_RS1_SIGNAL,
		DUT_COMMIT_RS2_SIGNAL,
		DUT_COMMIT_PC_SIGNAL,
		DUT_COMMIT_NEXT_PC_SIGNAL,
		DUT_COMMIT_INST_SIGNAL,
        DUT_COMMIT_EXCEPTION_SIGNAL,
	];
	let mut regexes = Vec::with_capacity(signals.len());
	for signal in signals.iter() {
		let escaped = regex::escape(signal);
		regexes.push(Regex::new(&format!("^{}$", escaped)).unwrap());
	}
	let mut filters = SignalFilters::new();
	filters.add_filter(SignalFilter::RegexFilter(regexes));
	filters
}

fn main() {
	let args = Args::parse();
	let filter_signal_list = build_commit_signal_filters(args.clock_signal.as_str());
	let waveform: waveform::WaveForm = waveform::WaveForm::load_waveform_and_cycle_map(
		args.waveform_path.as_str(),
		args.clock_signal.as_str(),
		Some(&filter_signal_list),
	)
	.unwrap();

	let ref_log = extract_commit_log(
		&waveform,
		REF_COMMIT_VALID_SIGNAL,
		REF_COMMIT_RD_SIGNAL,
		REF_COMMIT_RS1_SIGNAL,
		REF_COMMIT_RS2_SIGNAL,
		REF_COMMIT_PC_SIGNAL,
		REF_COMMIT_NEXT_PC_SIGNAL,
		REF_COMMIT_INST_SIGNAL,
        REF_COMMIT_EXCEPTION_SIGNAL
	);
	let dut_log = extract_commit_log(
		&waveform,
		DUT_COMMIT_VALID_SIGNAL,
		DUT_COMMIT_RD_SIGNAL,
		DUT_COMMIT_RS1_SIGNAL,
		DUT_COMMIT_RS2_SIGNAL,
		DUT_COMMIT_PC_SIGNAL,
		DUT_COMMIT_NEXT_PC_SIGNAL,
		DUT_COMMIT_INST_SIGNAL,
        DUT_COMMIT_EXCEPTION_SIGNAL
	);

	println!("ref commits: {}", ref_log.len());
	println!("dut commits: {}", dut_log.len());

	if let Some(ref_entry) = ref_log.get(args.commit_index) {
		println!(
			"ref commit[{}] inst=0x{:08x} pc=0x{:x} next_pc=0x{:x} exception=0x{:x}",
			args.commit_index,
			ref_entry.inst,
			ref_entry.pc,
			ref_entry.next_pc,
			ref_entry.exception
		);
	} else {
		println!("ref commit[{}] not found", args.commit_index);
	}
	if let Some(dut_entry) = dut_log.get(args.commit_index) {
		println!(
			"dut commit[{}] inst=0x{:08x} pc=0x{:x} next_pc=0x{:x} exception=0x{:x}",
			args.commit_index,
			dut_entry.inst,
			dut_entry.pc,
			dut_entry.next_pc,
			dut_entry.exception
		);
	} else {
		println!("dut commit[{}] not found", args.commit_index);
	}
    //Print the whole commit log
    print_whole_commit_log(&ref_log, &dut_log);

	let diffs = compare_commit_logs(&ref_log, &dut_log);
    // Sort diffs by commit index
    let mut diffs = diffs;
    diffs.sort_by_key(|d| d.index);

	println!("commit diffs: {}", diffs.len());
	if let Some(first_diff) = diffs.first() {
		if let Some(prev_index) = first_diff.index.checked_sub(1) {
			println!("commit before first diff: {}", prev_index);
			match ref_log.get(prev_index) {
				Some(entry) => println!("  ref: {}", format_entry(entry)),
				None => println!("  ref: <missing>"),
			}
			match dut_log.get(prev_index) {
				Some(entry) => println!("  dut: {}", format_entry(entry)),
				None => println!("  dut: <missing>"),
			}
		}
	}
	for diff in diffs.iter().take(50) {
		println!("diff at commit {}", diff.index);
		match &diff.ref_entry {
			Some(entry) => println!("  ref: {}", format_entry(entry)),
			None => println!("  ref: <missing>"),
		}
		match &diff.dut_entry {
			Some(entry) => println!("  dut: {}", format_entry(entry)),
			None => println!("  dut: <missing>"),
		}
	}
	if diffs.len() > 50 {
		println!("... {} more diffs not shown", diffs.len() - 50);
	}

    //If the commit logs are not of the same length, print the first missing commit
    if ref_log.len() != dut_log.len() {
        if ref_log.len() > dut_log.len() {
            println!("DUT log is shorter than reference log.");
            if let Some(missing_entry) = ref_log.get(dut_log.len()) {
                println!("First missing DUT commit: {}", format_entry(missing_entry));
            }
            //Print the last 5 entries of the DUT log
            println!("Last 5 entries of DUT log:");
            for entry in dut_log.iter().rev().take(5).rev() {
                println!("  {}", format_entry(entry));
            }
        } else {
            println!("Reference log is shorter than DUT log.");
            if let Some(missing_entry) = dut_log.get(ref_log.len()) {
                println!("First missing reference commit: {}", format_entry(missing_entry));
            }
            println!("Last 5 entries of reference log:");
            for entry in ref_log.iter().rev().take(5).rev() {
                println!("  {}", format_entry(entry));
            }
        }
    }

}
