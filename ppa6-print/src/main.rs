use anyhow::Result;
use clap::Parser;
use clap_num::maybe_hex;
use clap_verbosity::Verbosity;
use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
use image::{
    imageops::{dither, ColorMap, FilterType},
    DynamicImage, GrayImage, ImageFormat, ImageReader, Luma, RgbImage,
};
use ppa6::{FileBackend, Printer};
use rayon::prelude::*;
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

#[derive(Parser)]
struct Cli {
    /// Path to the file to be printed.
    file: PathBuf,

    /// Path to the device file.
    #[arg(short, long)]
    device: Option<PathBuf>,

    /// Number of copies.
    #[arg(short, long, default_value_t = 1)]
    num: usize,

    /// Show the image instead of printing.
    #[arg(short, long)]
    show: bool,

    /// Feed the printer.
    #[arg(short, long)]
    feed: bool,

    /// Invert the printed image.
    #[arg(short, long)]
    invert: bool,

    /// Rotate the image by 0, 90, 180, or 270 degrees.
    #[arg(short, long, default_value_t = 0)]
    rotate: usize,

    /// Threshold for dithering.
    #[arg(short = 'T', long, default_value_t = 0x80, value_parser = maybe_hex::<u8>)]
    threshold: u8,

    /// Treat `file` as a text file.
    #[arg(short, long)]
    text: bool,

    /// Font size for `--text`. Anything below 12 starts to be difficult to read.
    #[arg(short = 'S', long, default_value_t = 18.0)]
    size: f32,

    /// Font weight for `--text`. Good numbers are 600 and 800.
    #[arg(short, long, default_value_t = 800)]
    weight: u16,

    /// Line Height Factor. This gets multiplied with the font size to get the line height.
    #[arg(short, long, default_value_t = 1.0)]
    line_height: f32,

    /// Adjust brightness, positive values increase brightness, negative values decrease brightness
    #[arg(short, long, default_value_t = 0)]
    brighten: i32,

    /// Adjust constrast, positive values increase contrast, negative values decrease contrast
    #[arg(short, long, default_value_t = 0.0)]
    contrast: f32,

    /// Adjust the printer's concentration. Only values between `0..=2` are allowed.
    #[arg(short = 'C', long)]
    concentration: Option<u8>,

    #[command(flatten)]
    verbose: Verbosity,
}

struct BlackWhiteMap(u8);

impl ColorMap for BlackWhiteMap {
    type Color = Luma<u8>;

    fn index_of(&self, color: &Self::Color) -> usize {
        if color.0[0] >= self.0 {
            1
        } else {
            0
        }
    }
    fn map_color(&self, color: &mut Self::Color) {
        let idx = self.index_of(color);
        *color = self.lookup(idx).unwrap();
    }

    fn lookup(&self, index: usize) -> Option<Self::Color> {
        match index {
            0 => Some(Luma([0x00])),
            1 => Some(Luma([0xff])),
            _ => None,
        }
    }

    fn has_lookup(&self) -> bool {
        true
    }
}

fn resize(img: GrayImage) -> GrayImage {
    let (w, h) = img.dimensions();

    if w == 384 {
        return img;
    }

    let w = w as f32;
    let h = h as f32;
    let s = 384.0 / w;

    DynamicImage::ImageLuma8(img)
        .resize(384, (h * s) as u32 + 1, FilterType::Gaussian)
        .into_luma8()
}

fn rotate(img: GrayImage, deg: usize) -> GrayImage {
    match deg {
        0 => img,
        90 => DynamicImage::ImageLuma8(img).rotate90().into_luma8(),
        180 => DynamicImage::ImageLuma8(img).rotate180().into_luma8(),
        270 => DynamicImage::ImageLuma8(img).rotate270().into_luma8(),
        _ => panic!("invalid rotation: {deg}"),
    }
}

fn picture(cli: &Cli, data: &[u8]) -> Result<GrayImage> {
    log::trace!("parsing...");
    let img = ImageReader::new(Cursor::new(data))
        .with_guessed_format()?
        .decode()?
        .into_luma8();

    log::trace!("rotating...");
    let img = rotate(img, cli.rotate);

    log::trace!("resizing...");
    let mut img = DynamicImage::ImageLuma8(resize(img));

    if cli.brighten != 0 {
        log::trace!("brightening...");
        img = img.brighten(cli.brighten);
    }

    if cli.contrast != 0.0 {
        log::trace!("adjusting contrast...");
        img = img.adjust_contrast(cli.contrast);
    }

    let mut img = img.into_luma8();
    assert_eq!(img.width(), 384);

    log::trace!("dithering...");
    dither(&mut img, &BlackWhiteMap(cli.threshold));
    Ok(img)
}

// TODO: parse ANSI escape sequences
fn text(cli: &Cli, data: &[u8]) -> Result<GrayImage> {
    let text = String::from_utf8(data.to_vec())?;

    let mut font_system = FontSystem::new();
    let mut cache = SwashCache::new();
    let metrics = Metrics::new(cli.size, cli.size * cli.line_height);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    let mut buffer = buffer.borrow_with(&mut font_system);
    buffer.set_size(Some(340.0), None);
    let mut attrs = Attrs::new();
    attrs.weight.0 = cli.weight;

    buffer.set_text(&text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(true);

    let mut pixels = Vec::new();
    let mut height = 0;

    buffer.draw(&mut cache, Color::rgb(0xff, 0, 0), |x, y, w, h, color| {
        let a = color.a();
        if x < 0 || y < 0 || x > 384 || w != 1 || h != 1 || a == 0 {
            return;
        }

        let x = x as usize;
        let y = y as usize;

        if y >= height {
            height = y + 1;
            pixels.resize(3 * 384 * height, 0xff);
        }

        let scale = |c: u8| {
            let c = c as f32 / 255.0;
            let a = a as f32 / 255.0;
            let c = (c * a) + (1.0 * (1.0 - a));
            (c * 255.0).clamp(0.0, 255.0) as u8
        };

        pixels[(y * 384 + x) * 3 + 0] = scale(color.r());
        pixels[(y * 384 + x) * 3 + 1] = scale(color.g());
        pixels[(y * 384 + x) * 3 + 2] = scale(color.b());
    });

    let img = DynamicImage::ImageRgb8(RgbImage::from_vec(384, height as u32, pixels).unwrap());
    Ok(img.into_luma8())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::builder()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    let data = if cli.file == Path::new("-") {
        let mut data = Vec::new();
        std::io::stdin().read_to_end(&mut data)?;
        data
    } else {
        std::fs::read(&cli.file)?
    };
    let img = if cli.text {
        text(&cli, &data)
    } else {
        picture(&cli, &data)
    }?;

    if cli.show {
        let temppath = Path::new("/tmp/ppa6-preview.png");
        img.save_with_format(temppath, ImageFormat::Png)?;
        open::that(temppath)?;
        return Ok(());
    }

    log::trace!("mapping...");
    let pixels = img
        .par_pixels()
        .map(|c| (c.0[0] < cli.threshold) ^ cli.invert)
        .chunks(8)
        .map(|chunk| {
            chunk.iter().enumerate().fold(0u8, |mut acc, (i, c)| {
                assert!(i < 8);
                if *c {
                    acc |= 128 >> i;
                }
                acc
            })
        })
        .collect::<Vec<u8>>();

    let mut printer = if let Some(dev) = cli.device {
        Printer::new(FileBackend::open(&dev)?)
    } else {
        log::trace!("searching for printer...");
        Printer::find()?
    };

    log::trace!("resetting printer...");
    printer.reset()?;
    log::info!("IP: {}", printer.get_ip()?);
    log::info!("Firmware: {}", printer.get_firmware_ver()?);
    log::info!("Serial: {}", printer.get_serial()?);
    log::info!("Hardware: {}", printer.get_hardware_ver()?);
    log::info!("Name: {}", printer.get_name()?);
    log::info!("MAC: {:x?}", printer.get_mac()?);
    log::info!("Battery: {}%", printer.get_battery()?);

    if let Some(c) = cli.concentration {
        log::trace!("setting printer concentration to {c}...");
        printer.set_concentration(c)?;
    }

    for i in 0..cli.num {
        log::trace!("printing chunk {i}...");
        printer.print_image_chunked(&pixels, 384)?;
    }

    if cli.feed {
        log::trace!("feeding...");
        printer.push(0x60)?;
    }

    Ok(())
}
