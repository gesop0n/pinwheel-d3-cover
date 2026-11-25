use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use pinwheel_d3_cover::model::word::dominance_box;
use pinwheel_d3_cover::upper::enumerate::enumerate_boxes;

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
    /// 長さ <= max_len の全 Lyndon 語から異なる支配箱の代表語を列挙する
    Enum {
        #[arg(long, default_value_t = 12)]
        max_len: usize,
        /// 出力ファイルパス（省略時は標準出力）
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Box { word } => {
            let b = dominance_box(&word)?;
            println!("{word},{},{},{}", b[0], b[1], b[2]);
        }
        Command::Upper { command } => match command {
            UpperCommand::Enum { max_len, output } => {
                let words = enumerate_boxes(max_len);
                eprintln!("L={max_len}: {} distinct boxes", words.len());
                let body = words.join("\n") + "\n";
                match output {
                    Some(path) => {
                        fs::write(&path, body)
                            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
                    }
                    None => print!("{body}"),
                }
            }
        },
    }
    Ok(())
}
