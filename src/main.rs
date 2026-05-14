use base64::Engine;
use clap::{Parser, ValueHint};
use log::error;
use std::fs;
use std::path::Path;

mod parser;

#[derive(Parser)]
#[command(about = "Generate thumbnails from G-code files", long_about = None)]
struct Args {
    /// Path to the G-code file
    #[arg(value_hint = ValueHint::FilePath)]
    file: String,
    /// Path to save the extracted thumbnail (in PNG format)
    #[arg(long, short)]
    output: String,
}

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Args::parse();
    let file_path = args.file;
    // TODO: use anyhow to handle errors instead of panicking
    let content = fs::read_to_string(&file_path).expect("Failed to read file");

    // Visit the G-code file
    let mut visitor = parser::ProgramVisitorImpl::new();
    gcode::core::parse(&content, &mut visitor);

    // Find the largest embedded thumbnail
    if let Some(thumbnail) = visitor.get_best_thumbnail() {
        println!("Found thumbnail: {}x{}", thumbnail.width, thumbnail.height);

        // Decode the base64 data
        match base64::engine::general_purpose::STANDARD.decode(&thumbnail.data) {
            Ok(png_data) => {
                // Save the thumbnail to a file
                // TODO: maybe allow non-png output (by transcoding)?
                let output_path = Path::new(&args.output);
                match fs::write(output_path, png_data) {
                    Ok(_) => println!("Thumbnail saved to: {}", output_path.display()),
                    Err(e) => error!(
                        "Failed to write thumbnail to file {}: {e}",
                        output_path.display(),
                    ),
                }
            }
            Err(e) => error!("Failed to decode the thumbnail's base64 data: {e}"),
        }
    } else {
        println!("No thumbnails found in the G-code file");
    }
}
