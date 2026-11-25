use clap::{Parser, Subcommand};
use pinwheel_d3_cover::model::word::dominance_box;

#[derive(Parser)]
#[command(
    name = "pinwheel-d3-cover",
    about = "Minimum word cover for
    3-task pinwheel scheduling (density <= 5/6)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 1語の Dominance Box B(w) を計算して参考論文と同じ書式で出力する
    Box { word: String },
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Box { word } => {
            let b = dominance_box(&word)?;
            println!("{word},{},{},{}", b[0], b[1], b[2]);
        }
    }
    Ok(())
}
