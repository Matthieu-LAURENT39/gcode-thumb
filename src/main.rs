use base64::Engine;
use clap::{Parser, ValueEnum, ValueHint};
use image::ImageReader;
use log::error;
use std::fs;
use std::process::exit;

mod visitors;

#[derive(Parser)]
#[command(about = "Generate thumbnails from G-code files", long_about = None)]
struct Args {
    /// Path to the G-code file
    #[arg(value_hint = ValueHint::FilePath)]
    file: String,
    /// Path to save the extracted thumbnail (in PNG format)
    #[arg(long, short)]
    output: String,
    /// Thumbnail source: embedded, generate, auto
    /// - embedded: only use embedded thumbnails, fail if none found
    /// - generate: only generate thumbnails from G-code, ignore embedded ones
    /// - auto: use embedded thumbnails if found, otherwise generate from G-code
    #[arg(long, short, value_enum, default_value = "auto")]
    source: Source,
}

/// The source to control which thumbnails to use.
#[derive(Copy, Clone, Debug, ValueEnum)]
enum Source {
    Embedded,
    Generate,
    Auto,
}

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Args::parse();
    let file_path = args.file;
    // TODO: use anyhow to handle errors instead of panicking
    let content = fs::read_to_string(&file_path).expect("Failed to read file");

    // Try to get an embedded thumbnail, if the source allows them
    let embedded_thumbnail = if matches!(args.source, Source::Embedded | Source::Auto) {
        let mut embedded_visitor = visitors::embedded_thumbnail::EmbeddedThumbnailVisitor::new();
        gcode::core::parse(&content, &mut embedded_visitor);
        let embedded_thumbnail = embedded_visitor.get_best_thumbnail();
        embedded_thumbnail.map(|thumbnail| {
            println!(
                "Found embedded thumbnail: {}x{}",
                thumbnail.width, thumbnail.height
            );
            let reader = ImageReader::new(std::io::Cursor::new(
                // Decode the base64 data
                base64::engine::general_purpose::STANDARD
                    .decode(&thumbnail.data)
                    // TODO: use anyhow to handle errors instead of panicking
                    .expect("Failed to decode the thumbnail"),
            ))
            .with_guessed_format()
            .expect("Failed to guess image format");
            reader.decode().expect("Failed to decode image")
        })
    } else {
        None
    };

    let thumbnail = match args.source {
        Source::Embedded => {
            if let Some(thumbnail) = embedded_thumbnail {
                thumbnail
            } else {
                error!("No embedded thumbnail found in the G-code file");
                exit(1);
            }
        }
        Source::Generate => {
            let mut render_visitor = visitors::render_thumbnail::RenderThumbnailVisitor::new();
            gcode::core::parse(&content, &mut render_visitor);
            render_visitor.render()
        }
        Source::Auto => {
            if let Some(thumbnail) = embedded_thumbnail {
                thumbnail
            } else {
                let mut render_visitor = visitors::render_thumbnail::RenderThumbnailVisitor::new();
                gcode::core::parse(&content, &mut render_visitor);
                render_visitor.render()
            }
        }
    };

    // Save the thumbnail to the output path
    thumbnail
        .save(&args.output)
        // TODO: use anyhow to handle errors instead of panicking
        .expect("Failed to save thumbnail");
}
