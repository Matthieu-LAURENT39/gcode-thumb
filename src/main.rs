use clap::{Parser, ValueHint};
use std::fs;

mod parser;

#[derive(Parser)]
#[command(about = "Generate thumbnails from G-code files", long_about = None)]
struct Args {
    /// Path to the G-code file
    #[arg(value_hint = ValueHint::FilePath)]
    file: String,
}

fn main() {
    let args = Args::parse();
    let file_path = args.file;
    // TODO: use anyhow to handle errors instead of panicking
    let content = fs::read_to_string(&file_path).expect("Failed to read file");

    // Visit the G-code file
    let mut visitor = parser::ProgramVisitorImpl::new();
    gcode::core::parse(&content, &mut visitor);
}
