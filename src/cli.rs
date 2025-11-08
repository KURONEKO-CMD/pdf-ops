use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about = "pdf-ops: merge/split PDFs via CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Merge PDFs in a directory (default subcommand)
    Merge(MergeArgs),
    /// Split a single PDF into multiple files
    Split(SplitArgs),
    /// Launch terminal UI (requires `tui` feature)
    #[cfg(feature = "tui")]
    Tui(TuiArgs),
}

impl Cli {
    pub fn default_to_merge(self) -> Commands {
        match self.command {
            Some(cmd) => cmd,
            None => Commands::Merge(MergeArgs::default()),
        }
    }
}

#[derive(Args, Debug)]
pub struct MergeArgs {
    /// Input directory to scan recursively
    #[arg(short, long, value_name = "DIR", default_value = ".")]
    pub input_dir: String,
    /// Output file (relative resolves under input_dir)
    #[arg(short, long, value_name = "FILE", default_value = "merged.pdf")]
    pub output: String,
    /// Page spec applied to each input, e.g. "1-3,5,10-"
    #[arg(long, value_name = "SPEC")]
    pub pages: Option<String>,
    /// Include only files matching these globs (relative to input_dir). Repeatable.
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,
    /// Exclude files matching these globs (relative to input_dir). Repeatable.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,
    /// Overwrite output if it already exists
    #[arg(long)]
    pub force: bool,
}

impl Default for MergeArgs {
    fn default() -> Self {
        MergeArgs { input_dir: ".".into(), output: "merged.pdf".into(), pages: None, include: vec![], exclude: vec![], force: false }
    }
}

#[derive(Args, Debug)]
pub struct SplitArgs {
    /// Input PDF file
    #[arg(short = 'i', long, value_name = "FILE")]
    pub input: PathBuf,
    /// Output directory
    #[arg(short = 'd', long, value_name = "DIR", default_value = ".")]
    pub out_dir: PathBuf,
    /// One file per page (default if --ranges not provided)
    #[arg(long, conflicts_with = "ranges")]
    pub each: bool,
    /// Ranges to split, e.g. "1-3,4-6,7-" (one output per range)
    #[arg(long, value_name = "SPEC")]
    pub ranges: Option<String>,
    /// Output filename pattern, supports {base},{start},{end},{index}
    #[arg(long, value_name = "PATTERN", default_value = "{base}-{start}-{end}.pdf")]
    pub pattern: String,
    /// Overwrite output files if they already exist
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
#[cfg(feature = "tui")]
pub struct TuiArgs {
    /// Theme name, e.g. gitui-dark
    #[arg(long)]
    pub theme: Option<String>,
    /// Theme file (TOML)
    #[arg(long, value_name = "FILE")]
    pub theme_file: Option<PathBuf>,
    /// Initial input directory to scan
    #[arg(short = 'i', long, value_name = "DIR", default_value = ".")]
    pub input_dir: PathBuf,
}
