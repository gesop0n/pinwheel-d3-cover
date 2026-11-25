use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use pinwheel_d3_cover::model::bound::parse_rat;
use pinwheel_d3_cover::model::word::{dominance_box, parse_certificate};
use pinwheel_d3_cover::upper::enumerate::{enumerate_boxes, enumerate_boxes_pruned};
use pinwheel_d3_cover::upper::mincover::{find_min_cover, format_certificate};
use pinwheel_d3_cover::upper::verify::verify_certificate;

#[derive(Parser)]
#[command(
    name = "pinwheel-d3-cover",
    about = "Minimum word cover for 3-task pinwheel scheduling (density <= 5/6)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 1語の支配箱 B(w) を計算して元論文と同じ書式で出力する
    Box { word: String },
    /// 上界（証明書の探索と検証）に関するコマンド
    Upper {
        #[command(subcommand)]
        command: UpperCommand,
    },
}

#[derive(Subcommand)]
enum UpperCommand {
    /// 長さ <= max_len の語から異なる支配箱の代表語を列挙する
    Enum {
        #[arg(long, default_value_t = 12)]
        max_len: usize,
        /// 指定すると上端の和 u1+u2+u3 >= min_sum で枝刈り列挙を行う（例: 19/20）
        #[arg(long)]
        min_sum: Option<String>,
        /// 出力ファイルパス（省略時は標準出力）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// 候補語プール（または --max-len の列挙結果）から D_3 の最小被覆を HiGHS で求める
    Solve {
        /// 候補語ファイル（1行1語、または WORD,b1,b2,b3 形式）。省略時は --max-len で列挙
        pool: Option<PathBuf>,
        /// pool 省略時に列挙する最大語長
        #[arg(long, default_value_t = 12)]
        max_len: usize,
        /// pool 省略時に上端の和 u1+u2+u3 >= min_sum で枝刈り列挙を行う（例: 19/20）
        #[arg(long)]
        min_sum: Option<String>,
        /// 証明書ファイルの出力先（省略時は標準出力）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// 証明書ファイルが D_3 を完全に覆い、冗長な語がないことをソルバ不要の FM 消去で独立検証する
    Verify {
        /// 検証する証明書ファイル（例: certs/D3_11words.txt）
        cert: PathBuf,
    },
}

fn read_word_pool(path: &PathBuf) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.split(',').next().unwrap().trim().to_string())
        .collect())
}

fn run_enum(max_len: usize, min_sum: Option<&str>) -> Result<(Vec<String>, String), String> {
    match min_sum {
        None => Ok((
            enumerate_boxes(max_len),
            format!("all words of length <= {max_len}"),
        )),
        Some(s) => {
            let r = parse_rat(s)?;
            let words = enumerate_boxes_pruned(max_len, *r.numer(), *r.denom());
            Ok((
                words,
                format!("words of length <= {max_len} with u1+u2+u3 >= {r}"),
            ))
        }
    }
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Box { word } => {
            let b = dominance_box(&word)?;
            println!("{word},{},{},{}", b[0], b[1], b[2]);
        }
        Command::Upper { command } => match command {
            UpperCommand::Enum {
                max_len,
                min_sum,
                output,
            } => {
                let (words, desc) = run_enum(max_len, min_sum.as_deref())?;
                eprintln!("{desc}: {} distinct boxes", words.len());
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

                let header = format!(
                    "{} words covering D_3 (3 tasks, density <= 5/6); minimum over {}",
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
                let entries = parse_certificate(&content)?;
                let report = verify_certificate(&entries)?;
                println!(
                    "OK: {} ({} words) covers D_3 ({} FM cells checked, 0 uncovered)",
                    cert.display(),
                    report.word_count,
                    report.total_cells_checked
                );
                for entry in &report.leave_one_out {
                    match entry.witness {
                        Some(p) => println!(
                            "  necessary {:24} witness=({}, {}, {})",
                            entry.word, p[0], p[1], p[2]
                        ),
                        None => println!("  REDUNDANT {:24}", entry.word),
                    }
                }
                let redundant = report.redundant_words();
                if redundant.is_empty() {
                    println!("leave-one-out: all {} words are necessary", report.word_count);
                } else {
                    return Err(format!("redundant words found: {redundant:?}"));
                }
            }
        },
    }
    Ok(())
}
