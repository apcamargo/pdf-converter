mod utils;

use clap::{
    Parser, ValueEnum,
    builder::styling::{AnsiColor, Style, Styles},
    value_parser,
};
use hayro::{Pdf, RenderSettings, render};
use hayro_interpret::InterpreterSettings;
use hayro_svg::convert;
use file_format::FileFormat;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};

use colourful_logger::Logger;

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Cyan.on_default().bold())
    .usage(AnsiColor::Yellow.on_default().bold())
    .literal(AnsiColor::Yellow.on_default().bold())
    .placeholder(Style::new().dimmed());

#[derive(ValueEnum, Clone, Copy, Debug)]
#[value(rename_all = "lower")]
enum Format {
    Png,
    Svg,
}

#[derive(Parser)]
#[command(name = "pdf-converter", version, about = "Convert PDF files to PNG or SVG", max_term_width = 79, styles = STYLES)]
struct Cli {
    /// Suppress informational logging (only errors printed)
    #[arg(short = 'q', long = "quiet", global = true)]
    quiet: bool,

    /// Choose pages to convert. You can provide multiple page numbers separated by commas
    #[arg(
        short = 'p',
        long = "page",
        value_name = "PAGE",
        value_parser = value_parser!(usize),
        num_args = 1,
        value_delimiter = ',',
        action = clap::ArgAction::Append,
        global = true
    )]
    pages: Vec<usize>,

    /// Scale factor applied to outputs
    #[arg(short = 's', long = "scale", default_value = "1.0", global = true)]
    scale: f32,

    /// Prefix for output files. If omitted, inferred from the input name
    #[arg(long = "prefix", global = true)]
    prefix: Option<String>,

    /// Output format
    #[arg(value_enum, value_name = "FORMAT", ignore_case = true)]
    format: Format,

    /// Input PDF file
    #[arg(value_parser = value_parser!(PathBuf), value_name = "INPUT")]
    input: PathBuf,

    /// Output directory
    #[arg(value_parser = value_parser!(PathBuf), value_name = "OUTPUT", default_value = ".")]
    output: PathBuf,
}

// Provide a global colourful logger instance (user requested global usage).
static LOGGER: OnceLock<Logger> = OnceLock::new();
static QUIET: AtomicBool = AtomicBool::new(false);
fn get_logger() -> &'static Logger {
    LOGGER.get_or_init(Logger::default)
}

enum LogLevel {
    Info,
    Error,
}

fn log_event(level: LogLevel, message: &str, tag: &'static str) {
    match level {
        LogLevel::Info => {
            if !QUIET.load(Ordering::SeqCst) {
                get_logger().info_single(message, tag)
            }
        }
        LogLevel::Error => get_logger().error_single(message, tag),
    }
}

fn log_render_summary(kind: &str, count: usize, output: &Path, input: &Path) {
    let suffix = if count == 1 { "" } else { "s" };
    let message = format!(
        "Wrote {} {} file{} to {} (input: {})",
        count,
        kind,
        suffix,
        output.display(),
        input.display()
    );
    log_event(LogLevel::Info, &message, "Output");
}

struct AppError {
    tag: &'static str,
    message: String,
}

impl AppError {
    fn new(tag: &'static str, message: String) -> Self {
        Self { tag, message }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl fmt::Debug for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppError")
            .field("tag", &self.tag)
            .field("message", &self.message)
            .finish()
    }
}

impl Error for AppError {}

fn main() {
    // Initialize the global logger
    let _ = get_logger();

    if let Err(err) = run() {
        log_event(LogLevel::Error, &err.to_string(), err.tag);
        std::process::exit(1);
    }
}

fn run() -> Result<(), AppError> {
    let Cli {
        quiet,
        pages,
        scale,
        prefix,
        format,
        input,
        output,
    } = Cli::parse();

    // Apply quiet setting globally
    QUIET.store(quiet, Ordering::SeqCst);

    let interpreter_settings = InterpreterSettings::default();

    let output_existed = output.exists();
    fs::create_dir_all(&output).map_err(|e| {
        AppError::new(
            "FileSystem",
            format!("Failed to create output directory: {e}"),
        )
    })?;
    if !output_existed {
        let msg = format!("Created output directory: {}", output.display());
        log_event(LogLevel::Info, &msg, "Output");
    }

    match format {
        Format::Png => process_png(&input, &output, scale, prefix.as_deref(), &pages, &interpreter_settings)?,
        Format::Svg => process_svg(&input, &output, scale, prefix.as_deref(), &pages, &interpreter_settings)?,
    }

    Ok(())
}

fn load_pdf_and_pages(
    input: &Path,
    pages: &[usize],
) -> Result<(Pdf, Option<HashSet<usize>>), AppError> {
    let bytes = fs::read(input)
        .map_err(|e| AppError::new("FileSystem", format!("Failed to read input file: {e}")))?;

    // Detect file format and ensure it's a PDF
    let fmt = FileFormat::from_bytes(&bytes);
    if fmt != FileFormat::PortableDocumentFormat {
        return Err(AppError::new(
            "FileType",
            "Input file is not a PDF".to_string(),
        ));
    }

    let data = Arc::new(bytes);
    let pdf =
        Pdf::new(data).map_err(|e| AppError::new("PDF", format!("Failed to read PDF: {e:?}")))?;

    let page_set = if pages.is_empty() {
        None
    } else {
        let validated_set = utils::validate_requested_pages(pages, pdf.pages().len())
            .map_err(|msg| AppError::new("PageValidation", msg))?;
        Some(validated_set)
    };

    Ok((pdf, page_set))
}

fn process_png(
    input: &Path,
    output: &Path,
    scale: f32,
    prefix: Option<&str>,
    pages: &[usize],
    interpreter_settings: &InterpreterSettings,
) -> Result<(), AppError> {
    let (pdf, page_set) = load_pdf_and_pages(input, pages)?;

    let render_settings = RenderSettings {
        x_scale: scale,
        y_scale: scale,
        ..Default::default()
    };
    let prefix = utils::resolve_prefix(prefix, input);

    let files_written = pdf
        .pages()
        .iter()
        .enumerate()
        .filter(|(idx, _)| {
            page_set
                .as_ref()
                .map(|set| set.contains(idx))
                .unwrap_or(true)
        })
        .map(|(idx, page)| {
            let pixmap = render(page, interpreter_settings, &render_settings);
            let out_name = format!("{}{}.png", prefix, idx + 1);
            let out_path = output.join(out_name);
            let png_bytes = pixmap.take_png();
            fs::write(out_path, png_bytes)
                .map_err(|e| AppError::new("FileSystem", format!("Failed to write PNG: {e}")))?;
            Ok(())
        })
        .collect::<Result<Vec<_>, AppError>>()?
        .len();

    log_render_summary("PNG", files_written, output, input);

    Ok(())
}

fn process_svg(
    input: &Path,
    output: &Path,
    scale: f32,
    prefix: Option<&str>,
    pages: &[usize],
    interpreter_settings: &InterpreterSettings,
) -> Result<(), AppError> {
    let (pdf, page_set) = load_pdf_and_pages(input, pages)?;

    let prefix = utils::resolve_prefix(prefix, input);

    let mut files_written = 0usize;

    for (idx, page) in pdf.pages().iter().enumerate() {
        if let Some(ref set) = page_set
            && !set.contains(&idx)
        {
            continue;
        }

        let svg = convert(page, interpreter_settings);
        let mut out_svg = svg;
        if (scale - 1.0).abs() > f32::EPSILON {
            if let Some(w_pos) = out_svg.find("width=\"") {
                let start = w_pos + 7;
                if let Some(rel_end) = out_svg[start..].find('"') {
                    let end = start + rel_end;
                    if let Ok(old_w) = out_svg[start..end].parse::<f32>() {
                        let new_w = old_w * scale;
                        out_svg.replace_range(start..end, &format!("{:.6}", new_w));
                    }
                }
            }
            if let Some(h_pos) = out_svg.find("height=\"") {
                let start = h_pos + 8;
                if let Some(rel_end) = out_svg[start..].find('"') {
                    let end = start + rel_end;
                    if let Ok(old_h) = out_svg[start..end].parse::<f32>() {
                        let new_h = old_h * scale;
                        out_svg.replace_range(start..end, &format!("{:.6}", new_h));
                    }
                }
            }
        }

        let out_name = format!("{}{}.svg", prefix, idx + 1);
        let out_path = output.join(out_name);
        fs::write(out_path, out_svg)
            .map_err(|e| AppError::new("FileSystem", format!("Failed to write SVG: {e}")))?;
        files_written += 1;
    }

    log_render_summary("SVG", files_written, output, input);

    Ok(())
}
