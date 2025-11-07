mod cli;
mod merge;
mod spec;
mod split;
mod progress;
mod scan;
#[cfg(feature = "tui")]
mod tui;

use cli::{Cli, Commands};
use clap::Parser;
use std::path::PathBuf;
use progress::{IndicatifProgress, ProgressSink};

fn main() {
    let cmd = Cli::parse().default_to_merge();
    match cmd {
        Commands::Merge(args) => {
            let mut output_path = PathBuf::from(&args.output);
            if output_path.is_relative() {
                let mut new_path = PathBuf::from(&args.input_dir);
                new_path.push(&output_path);
                output_path = new_path;
            }
            let input_dir = PathBuf::from(&args.input_dir);
            let pb = IndicatifProgress::new();
            if let Err(e) = merge::run(&input_dir, &output_path, args.pages.as_deref(), &args.include, &args.exclude, args.force, &pb) {
                eprintln!("❌ 合并失败: {}", e);
                std::process::exit(1);
            }
            println!("✅ 合并完成 -> {}", output_path.display());
        }
        Commands::Split(args) => {
            let each = if args.ranges.is_none() { true } else { args.each };
            let pb = IndicatifProgress::new();
            if let Err(e) = split::run(&args.input, &args.out_dir, each, args.ranges.as_deref(), &args.pattern, args.force, &pb) {
                eprintln!("❌ 分割失败: {}", e);
                std::process::exit(1);
            }
            println!("✅ 分割完成 -> {}", args.out_dir.display());
        }
        #[cfg(feature = "tui")]
        Commands::Tui(args) => {
            if let Err(e) = tui::run(args.theme, args.theme_file, args.input_dir) {
                eprintln!("❌ TUI 启动失败: {}", e);
                std::process::exit(1);
            }
        }
    }
}
