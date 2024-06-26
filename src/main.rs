mod image_to_poster;
mod poster;

use clap::{arg, command, Parser};
use image::io::Reader as ImageReader;
use image::{imageops::FilterType, DynamicImage, GenericImageView};
use poster::*;
use rand::Rng;
use std::fs;
use std::fs::File;
use std::path::PathBuf;

#[derive(PartialEq)]
enum Format {
    Image,
    Poster,
}

#[derive(clap::ValueEnum, Clone)]
enum ResizeAlgorithm {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

impl From<ResizeAlgorithm> for FilterType {
    fn from(value: ResizeAlgorithm) -> Self {
        match value {
            ResizeAlgorithm::Nearest => FilterType::Nearest,
            ResizeAlgorithm::Triangle => FilterType::Triangle,
            ResizeAlgorithm::CatmullRom => FilterType::CatmullRom,
            ResizeAlgorithm::Gaussian => FilterType::Gaussian,
            ResizeAlgorithm::Lanczos3 => FilterType::Lanczos3,
        }
    }
}

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "INPUT_FILE")]
    input: PathBuf,

    #[arg(short, long, value_name = "OUTPUT_FILE")]
    output: PathBuf,

    #[arg(short = 'p', long, value_name = "PREVIEW_OUTPUT_FILE")]
    preview: Option<PathBuf>,

    #[arg(short = 'x', long, value_name = "SCALE_X")]
    scale_x: Option<u32>,

    #[arg(short = 'y', long, value_name = "SCALE_Y")]
    scale_y: Option<u32>,

    /// Algorithm to use for resizing and scaling. Defaults to catmull-rom
    #[arg(short = 'r', long, value_name = "RESIZE_ALGORITHM")]
    resize_algorithm: Option<ResizeAlgorithm>,

    #[arg(short = 'a', long, value_name = "AUTOSCALE")]
    autoscale: Option<f64>,

    #[arg(short, long, value_name = "LABEL")]
    label: Option<String>,

    #[arg(short = 'L', long = "forcelabel", value_name = "FORCED_LABEL")]
    force_label: Option<String>,

    #[arg(short = 'T', long = "forcetooltip", value_name = "TOOLTIP")]
    force_tooltip: Option<String>,

    #[arg(short = 'Q', long)]
    per_poster_quantization: bool,

    #[arg(short = 'j', long, value_name = "JOBS")]
    jobs: Option<u32>,
}

fn read_image(image_file: &PathBuf) -> (bool, Option<DynamicImage>) {
    let image_reader = ImageReader::open(image_file);
    if image_reader.is_err() {
        return (false, None);
    }

    let mut decoder = image_reader.unwrap();
    decoder.no_limits();

    let decoded_image = decoder.decode();
    if decoded_image.is_err() {
        return (false, None);
    }

    return (true, Some(decoded_image.unwrap()));
}

fn autoscale_image(mut width: u32, mut height: u32, scale: f64) -> (u32, u32) {
    //TODO: make this attempt to preserve aspect ratio later
    width = (width as f64 * scale) as u32;
    height = (height as f64 * scale) as u32;
    let (wr, hr) = (width % 128, height % 128);
    let (scaled_x, scaled_y) = (
        if wr >= 64 {
            width + (128 - wr)
        } else {
            width - wr
        },
        if hr >= 64 {
            height + (128 - hr)
        } else {
            height - hr
        },
    );
    return (
        if scaled_x == 0 { 128 } else { scaled_x },
        if scaled_y == 0 { 128 } else { scaled_y },
    );
}

fn main() {
    let cli = Cli::parse();

    let per_poster_quantization_enabled = cli.per_poster_quantization;

    if !cli.input.exists() {
        eprintln!("Input file doesn't exist.");
        return;
    }
    if cli.input.is_dir() {
        eprintln!("Input can't be a directory.");
        return;
    }

    if cli.output.is_dir() {
        eprintln!("Output can't be a directory.");
        return;
    }

    match cli.output.parent() {
        Some(parent) => {
            if !parent.exists() {
                eprintln!("Output file parent directory doesn't exist.");
                return;
            } else if !parent.is_dir() {
                eprintln!("Output file parent is not a directory.");
                return;
            }
        }
        None => {
            eprintln!("Output file parent directory doesn't exist.");
            return;
        }
    }

    if let Some(ref preview) = cli.preview {
        match preview.parent() {
            Some(parent) => {
                if !parent.exists() {
                    eprintln!("Preview file parent directory doesn't exist.");
                    return;
                } else if !parent.is_dir() {
                    eprintln!("Preview file parent is not a directory.");
                    return;
                }
            }
            None => {
                eprintln!("Preview file parent directory doesn't exist.");
                return;
            }
        }

        let preview_extension = match preview.extension() {
            Some(t) => t,
            None => {
                eprintln!("Preview file has no extension.");
                return;
            }
        }
        .to_str()
        .unwrap()
        .to_lowercase();
        let preview_extension = preview_extension.as_str();

        match preview_extension {
            "png" => Format::Image,
            "jpg" => Format::Image,
            "jpeg" => Format::Image,
            "bmp" => Format::Image,
            _ => {
                eprintln!("Unsupported preview format: {}", preview_extension);
                return;
            }
        };
    }

    let input_extension = match cli.input.extension() {
        Some(t) => t,
        None => {
            eprintln!("Input file has no extension.");
            return;
        }
    }
    .to_str()
    .unwrap()
    .to_lowercase();
    let output_extension = match cli.output.extension() {
        Some(t) => t,
        None => {
            eprintln!("Output file has no extension.");
            return;
        }
    }
    .to_str()
    .unwrap()
    .to_lowercase();
    let input_extension = input_extension.as_str();
    let output_extension = output_extension.as_str();

    let input_format: Format = match input_extension {
        "png" => Format::Image,
        "jpg" => Format::Image,
        "jpeg" => Format::Image,
        "bmp" => Format::Image,
        // can likely support more image formats, but cant be bothered
        "2dj" => Format::Poster,
        "2dja" => Format::Poster,
        _ => {
            eprintln!("Unsupported input format: {}", input_extension);
            return;
        }
    };
    let output_format: Format = match output_extension {
        "png" => Format::Image,
        "jpg" => Format::Image,
        "jpeg" => Format::Image,
        "bmp" => Format::Image,
        // can likely support more image formats, but cant be bothered
        "2dj" => Format::Poster,
        "2dja" => Format::Poster,
        _ => {
            eprintln!("Unsupported output format: {}", output_extension);
            return;
        }
    };

    // TODO: clean up
    {
        let mut e: bool = false;
        if input_format == Format::Poster {
            if cli.per_poster_quantization {
                eprintln!("per-poster-quantization flag only allowed with input format: Image");
                e = true;
            }
            if cli.label.is_some() {
                eprintln!("label arg only allowed with input format: Image");
                e = true;
            }
            if cli.force_label.is_some() {
                eprintln!("force-label arg only allowed with input format: Image");
                e = true;
            }
            if cli.force_tooltip.is_some() {
                eprintln!("force-tooltip arg only allowed with input format: Image");
                e = true;
            }
            if cli.scale_x.is_some() {
                eprintln!("scale-x arg only allowed with input format: Image");
                e = true;
            }
            if cli.scale_y.is_some() {
                eprintln!("scale-y arg only allowed with input format: Image");
                e = true;
            }
            if cli.autoscale.is_some() {
                eprintln!("autoscale arg only allowed with input format: Image");
                e = true;
            }
        }

        if cli.autoscale.is_some() {
            if cli.scale_x.is_some() {
                eprintln!("scale-x arg not allowed with autoscale");
                e = true;
            }
            if cli.scale_y.is_some() {
                eprintln!("scale-y arg not allowed with autoscale");
                e = true;
            }
        }

        if e {
            return;
        }
    }

    let mut poster_array: poster::PosterArray;
    if input_format == Format::Image {
        let (image_ok, image) = read_image(&cli.input);
        if !image_ok {
            eprintln!("Failed to decode or open image.");
            return;
        }
        let mut unwrapped_image = image.unwrap();

        let (mut x_size, mut y_size) = unwrapped_image.dimensions();

        {
            let mut resize = false;
            let (mut resize_x, mut resize_y) = (x_size, y_size);

            if let Some(res) = cli.scale_x {
                resize = true;
                resize_x = res;
            }

            if let Some(res) = cli.scale_y {
                resize = true;
                resize_y = res;
            }

            if cli.autoscale.is_some() {
                let (x, y) = autoscale_image(x_size, y_size, cli.autoscale.unwrap());
                if x != x_size || y != y_size {
                    resize_x = x;
                    resize_y = y;
                    resize = true;
                }
            }

            if resize && (resize_x < 1 || resize_y < 1) {
                eprintln!("Can't resize to x:{0} y:{1}", resize_x, resize_y);
                return;
            }

            if resize && ((resize_x % 128 != 0) || (resize_y % 128 != 0)) {
                eprintln!("Image resolutions have to be multiples of 128 (Attempted to resize to x:{0} y:{1})",resize_x, resize_y);
                return;
            }

            if resize {
                println!(
                    "Resizing image to x:{0} y:{1} (from x:{2} y:{3})",
                    resize_x, resize_y, x_size, y_size
                );

                x_size = resize_x;
                y_size = resize_y;
                unwrapped_image = unwrapped_image.resize_exact(
                    resize_x,
                    resize_y,
                    cli.resize_algorithm
                        .unwrap_or(ResizeAlgorithm::CatmullRom)
                        .into(),
                );
            }
        }

        if (x_size % 128 != 0) || (y_size % 128 != 0) {
            eprintln!(
                "Image resolutions have to be multiples of 128 (Currently x:{0} y:{1})",
                x_size, y_size
            );
            return;
        }

        let mut forced_label: bool = false;
        let label: String;

        if let Some(txt) = cli.force_label {
            label = txt.to_string();
            forced_label = true;
            if label.len() > 48 {
                eprintln!(
                    "Forced label can't be longer than 48 characters, currently {0}",
                    label.len()
                );
                return;
            }
        } else if let Some(txt) = cli.label {
            label = txt.to_string();
            if label.len() > 23 {
                eprintln!(
                    "Label can't be longer than 23 characters, currently {0}",
                    label.len()
                );
                return;
            }
        } else {
            label = "PatriikPlays/img2poster".to_string();
        }

        let mut use_forced_tooltip = false;
        let mut forced_tooltip: String = "".to_string();
        if let Some(txt) = cli.force_tooltip {
            forced_tooltip = txt.to_string();
            use_forced_tooltip = true;
            if forced_tooltip.len() > 256 {
                eprintln!(
                    "Forced tooltip can't be longer than 256 characters, currently {0}",
                    forced_tooltip.len()
                );
                return;
            }
        }

        let print_id = format!("{:0>6}", rand::thread_rng().gen_range(0..999999));

        let label_generator_label = label.clone();
        let tooltip_generator_label = label.clone();

        poster_array = image_to_poster::image_to_posters(
            unwrapped_image,
            move |x, y, w, h| {
                if forced_label {
                    return label.clone();
                } else {
                    return format!(
                        "{0}: ({1},{2})/({3}x{4})",
                        label_generator_label.clone(),
                        x + 1,
                        y + 1,
                        w,
                        h
                    );
                }
            },
            move |x, y, w, h| {
                let tooltip: PosterTooltip = PosterTooltip {
                    print_id: print_id.clone(),
                    print_name: tooltip_generator_label.clone(),
                    total_width: w,
                    total_height: h,
                    pos_x: x,
                    pos_y: y,
                    info: "https://github.com/PatriikPlays/img2poster".to_string(),
                };

                if use_forced_tooltip {
                    return forced_tooltip.clone();
                } else {
                    return serde_json::to_string(&tooltip)
                        .unwrap()
                        .as_str()
                        .to_string();
                }
            },
            (per_poster_quantization_enabled, Some(cli.jobs.unwrap_or(1))),
        );
    } else if input_format == Format::Poster {
        if input_extension == "2dj" {
            poster_array = PosterArray {
                pages: vec![],
                width: 1,
                height: 1,
                title: "untitled".to_string(),
            };
            let reader = File::open(cli.input).expect("Failed to open input file.");
            poster_array
                .pages
                .push(serde_json::from_reader(reader).expect("Failed to parse json in input file"));
        } else if input_extension == "2dja" {
            let reader = File::open(cli.input).expect("Failed to open input file.");
            poster_array =
                serde_json::from_reader(reader).expect("Failed to parse json in input file");
        } else {
            eprintln!("Shouldn't have gotten here 0");
            return;
        }
    } else {
        eprintln!("Shouldn't have gotten here 1");
        return;
    }

    println!("Done, saving to file");
    if output_format == Format::Poster {
        match output_extension {
            "2dj" => {
                if poster_array.pages.len() > 1 {
                    eprintln!("Format 2dj doesn't support multi poster images.");
                    return;
                }

                let json_str = serde_json::to_string(&poster_array.pages[0])
                    .expect("Failed to serialize this somehow");
                fs::write(&cli.output, json_str).expect("Failed to write to output file.");

                if let Some(ref preview) = cli.preview {
                    println!("Generating preview...");
                    let output_image = posters_to_dynamic_image(&poster_array);
                    output_image
                        .save(preview)
                        .expect("Failed to save preview image.");
                }
            }
            "2dja" => {
                let json_str =
                    serde_json::to_string(&poster_array).expect("Failed to serialize this somehow");
                fs::write(cli.output, json_str).expect("Failed to write to output file.");
                if let Some(ref preview) = cli.preview {
                    println!("Generating preview...");
                    let output_image = posters_to_dynamic_image(&poster_array);
                    output_image
                        .save(preview)
                        .expect("Failed to save preview image.");
                }
            }
            _ => {
                eprintln!("Invalid output extension: {}.", output_extension);
                return;
            }
        }
    } else if output_format == Format::Image {
        let output_image = posters_to_dynamic_image(&poster_array);

        output_image
            .save(cli.output)
            .expect("Failed to save image.");
    }
}
