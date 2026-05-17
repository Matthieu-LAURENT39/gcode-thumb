use anyhow::{Context, bail};
use base64::Engine;
use clap::{ArgAction, Parser, ValueEnum, ValueHint};
use image::{ImageReader, imageops::FilterType};
use std::fs;

mod visitors;

/// Validates a hexadecimal RGB or RGBA color string.
fn parse_color(value: &str) -> Result<String, String> {
    let len = value.len();
    if (len != 6 && len != 8) || !value.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "Invalid background color: {}. Must be a 6 or 8 digit hexadecimal RGB(A) value.",
            value
        ));
    }
    Ok(value.to_string())
}

#[derive(Parser)]
#[command(about = "Generate thumbnails from G-code files", long_about = None)]
struct Args {
    /// Path to the G-code file
    #[arg(value_hint = ValueHint::FilePath)]
    file: String,
    /// Path to save the extracted thumbnail (in PNG format)
    #[arg(long, short)]
    output: String,

    /// Which source to get the thumbnail from.
    #[arg(long, value_enum, default_value = "auto")]
    source: Source,

    /// The size of the thumbnail to generate, in pixels. The image will be a square of this size.
    /// For embedded thumbnails, they will be downscaled to fit within this size if they are larger, but won't be upscaled if they are smaller.
    #[arg(long, short, default_value_t = 512, verbatim_doc_comment)]
    size: u32,

    /// The background color for generated thumbnails, in hexadecimal RGB (with optional alpha).
    #[arg(long, short, default_value = "00000000", value_parser = parse_color)]
    background: String,

    // TODO: it'd be nice to have the positive version of this flag too, but
    // it's hard to do until https://github.com/clap-rs/clap/issues/815 is fixed
    /// Don't ignore priming-line for thumbnail generation.
    /// If this flag is not set, the priming line will be ignored when generating the thumbnail.
    #[arg(
        long = "no-ignore-priming-line",
        action = ArgAction::SetFalse,
        default_value_t = true
    )]
    ignore_priming_line: bool,
}

/// The source to control which thumbnails to use.
#[derive(Copy, Clone, Debug, ValueEnum)]
enum Source {
    /// Only use embedded thumbnails, fail if none found
    Embedded,
    /// Only generate thumbnails from G-code, ignore embedded ones
    Generate,
    /// Use embedded thumbnails if found, otherwise generate from G-code-code
    Auto,
}

fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = Args::parse();
    let file_path = args.file;
    let size = args.size;
    let content = fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read file: {file_path}"))?;

    // Try to get an embedded thumbnail, if the source allows them
    let embedded_thumbnail = if matches!(args.source, Source::Embedded | Source::Auto) {
        let mut embedded_visitor = visitors::embedded_thumbnail::EmbeddedThumbnailVisitor::new();
        gcode::core::parse(&content, &mut embedded_visitor);
        let embedded_thumbnail = embedded_visitor.get_best_thumbnail();
        embedded_thumbnail
            .map(|thumbnail| -> anyhow::Result<_> {
                println!(
                    "Found embedded thumbnail: {}x{}",
                    thumbnail.width, thumbnail.height
                );
                let reader = ImageReader::new(std::io::Cursor::new(
                    // Decode the base64 data
                    base64::engine::general_purpose::STANDARD
                        .decode(&thumbnail.data)
                        .context("Failed to decode embedded thumbnail base64")?,
                ))
                .with_guessed_format()
                .context("Failed to guess embedded image format")?;
                let image = reader.decode().context("Failed to decode embedded image")?;
                if image.width() > size || image.height() > size {
                    Ok(image.resize(size, size, FilterType::Triangle))
                } else {
                    Ok(image)
                }
            })
            .transpose()?
    } else {
        None
    };

    let thumbnail = match args.source {
        Source::Embedded => {
            if let Some(thumbnail) = embedded_thumbnail {
                thumbnail
            } else {
                bail!("No embedded thumbnail found in the G-code file");
            }
        }
        Source::Generate => {
            let mut render_visitor =
                visitors::render_thumbnail::RenderThumbnailVisitor::new(args.ignore_priming_line);
            gcode::core::parse(&content, &mut render_visitor);
            render_visitor.render(&args.background, args.size)?
        }
        Source::Auto => {
            if let Some(thumbnail) = embedded_thumbnail {
                thumbnail
            } else {
                let mut render_visitor = visitors::render_thumbnail::RenderThumbnailVisitor::new(
                    args.ignore_priming_line,
                );
                gcode::core::parse(&content, &mut render_visitor);
                render_visitor.render(&args.background, args.size)?
            }
        }
    };

    // Save the thumbnail to the output path
    thumbnail
        .save(&args.output)
        .with_context(|| format!("Failed to save thumbnail to {}", args.output))?;

    Ok(())
}
