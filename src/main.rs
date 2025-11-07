mod cli;
mod merge;
mod spec;
mod split;

use cli::{Cli, Commands};
use clap::Parser;
use std::path::PathBuf;

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
            if let Err(e) = merge::run(&input_dir, &output_path, args.pages.as_deref()) {
                eprintln!("❌ 合并失败: {}", e);
                std::process::exit(1);
            }
            println!("✅ 合并完成 -> {}", output_path.display());
        }
        Commands::Split(args) => {
            if let Err(e) = split::run(&args.input, &args.out_dir, args.each, args.ranges.as_deref(), &args.pattern) {
                eprintln!("❌ 分割失败: {}", e);
                std::process::exit(1);
            }
            println!("✅ 分割完成 -> {}", args.out_dir.display());
        }
    }
}
