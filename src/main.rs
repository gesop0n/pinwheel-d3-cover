use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use pinwheel_d3_cover::model::bound::parse_fraction;
use pinwheel_d3_cover::model::word::{PeriodicSchedule, parse_certificate};
use pinwheel_d3_cover::upper::enumerate::{enumerate_boxes, enumerate_boxes_pruned};
use pinwheel_d3_cover::upper::mincover::{find_min_cover, format_certificate};
use pinwheel_d3_cover::upper::verify::verify_certificate;

#[derive(Parser)]
#[command(
    name = "pinwheel-d3-cover",
    about = "Minimum dominating word cover for 3-task real-period pinwheel scheduling (D_3, density <= 5/6)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compute the dominance box B(w) of a 3-task word, canonicalized like the paper's
    /// `packing_schedule_quality` (tasks relabeled by descending bound, smallest rotation)
    Box {
        /// Periodic word using all of {A, B, C} (e.g. AABACB)
        word: String,
    },
    /// Upper-bound search and independent domination verification over D_3
    Upper {
        #[command(subcommand)]
        command: UpperCommand,
    },
}

#[derive(Subcommand)]
enum UpperCommand {
    /// Enumerate one shortest word per distinct density-ordered dominance box up to max_len
    Enum {
        #[arg(long, default_value_t = 12)]
        max_len: usize,
        /// Optional lower bound on u_1 + u_2 + u_3 for pruned DFS (e.g. 19/20)
        #[arg(long)]
        min_sum: Option<String>,
        /// Write output to a file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Find a minimum dominating family of words over D_3 using slice cells and HiGHS
    Solve {
        /// Candidate word pool file (one word per line or WORD,b1,b2,b3). If omitted, enumerates up to --max-len
        pool: Option<PathBuf>,
        /// Maximum word length when --pool is omitted
        #[arg(long, default_value_t = 12)]
        max_len: usize,
        /// Optional lower bound on u_1 + u_2 + u_3 for pruned DFS when pool is omitted (e.g. 19/20)
        #[arg(long)]
        min_sum: Option<String>,
        /// Write certificate to a file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Independently verify domination of D_3 and leave-one-out necessity (solver-free FM check)
    Verify {
        /// Path to a certificate file (e.g. proofs/D3_11words.txt)
        cert: PathBuf,
    },
}

fn read_word_pool(path: &PathBuf) -> Result<Vec<String>, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split(',').next().unwrap().trim().to_string())
        .collect())
}

fn run_enum(max_len: usize, min_sum: Option<&str>) -> Result<(Vec<String>, String), String> {
    if !(3..=64).contains(&max_len) {
        return Err(format!("--max-len must be between 3 and 64, got {max_len}"));
    }
    match min_sum {
        None => Ok((
            enumerate_boxes(max_len),
            format!("all words of length <= {max_len}"),
        )),
        Some(s) => {
            let r = parse_fraction(s)?;
            let words = enumerate_boxes_pruned(max_len, *r.numer(), *r.denom());
            Ok((
                words,
                format!("words of length <= {max_len} with u_1 + u_2 + u_3 >= {r}"),
            ))
        }
    }
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Box { word } => {
            let sched = PeriodicSchedule::from_word(&word)?.canonicalize()?;
            println!("{sched}");
        }
        Command::Upper { command } => match command {
            UpperCommand::Enum {
                max_len,
                min_sum,
                output,
            } => {
                let (words, desc) = run_enum(max_len, min_sum.as_deref())?;
                eprintln!("{desc}: {} distinct dominance boxes", words.len());
                let body = words.join("\n") + "\n";
                match output {
                    Some(path) => {
                        fs::write(&path, body)
                            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
                    }
                    None => print!("{body}"),
                }
            }
            UpperCommand::Solve {
                pool,
                max_len,
                min_sum,
                output,
            } => {
                let (words, source_desc) = match pool {
                    Some(path) => (read_word_pool(&path)?, path.display().to_string()),
                    None => run_enum(max_len, min_sum.as_deref())?,
                };
                let res = find_min_cover(&words)?;
                eprintln!(
                    "pool: {} words -> {} distinct in D_3 -> {} maximal boxes",
                    res.pool_total_words,
                    res.pool.distinct_in_domain,
                    res.pool.items.len()
                );
                eprintln!(
                    "slice cells: {:?} elems/axis, {} tested, {} feasible, {} cover-sets -> {} minimal rows",
                    res.cells.axis_sizes,
                    res.cells.tested_cells,
                    res.cells.feasible_cells,
                    res.cells.distinct_cover_sets,
                    res.cells.minimal_rows.len()
                );
                eprintln!("MINIMUM: {} words", res.chosen.len());

                // HiGHS works in floating point: never emit a certificate that has not passed
                // the exact, solver-free domination and leave-one-out check.
                let report = verify_certificate(&res.chosen)?;
                let redundant = report.redundant_words();
                if !redundant.is_empty() {
                    return Err(format!(
                        "solver returned a cover with redundant words: {redundant:?}"
                    ));
                }
                eprintln!("exact FM verification of the chosen words: passed");

                let header = format!(
                    "{} words dominating D_3 (dimension 3; density at most 5/6); minimum over {}",
                    res.chosen.len(),
                    source_desc
                );
                let cert = format_certificate(&res.chosen, &header);
                match output {
                    Some(path) => {
                        fs::write(&path, &cert)
                            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
                    }
                    None => print!("{cert}"),
                }
            }
            UpperCommand::Verify { cert } => {
                let content = fs::read_to_string(&cert)
                    .map_err(|e| format!("failed to read {}: {e}", cert.display()))?;
                let schedules = parse_certificate(&content)?;
                let report = verify_certificate(&schedules)?;
                println!(
                    "The supplied {} words dominate D_3 ({} threshold cells enumerated, 0 undominated): {}",
                    report.word_count,
                    report.total_cells_checked,
                    cert.display()
                );
                for entry in &report.leave_one_out {
                    match entry.undominated_witness {
                        Some(d) => println!(
                            "  necessary {:24} undominated density vector = ({}, {}, {})",
                            entry.word, d[0], d[1], d[2]
                        ),
                        None => println!("  REDUNDANT {:24}", entry.word),
                    }
                }
                let redundant = report.redundant_words();
                if redundant.is_empty() {
                    println!(
                        "leave-one-out: all {} words are necessary",
                        report.word_count
                    );
                } else {
                    return Err(format!("redundant words found: {redundant:?}"));
                }
            }
        },
    }
    Ok(())
}
