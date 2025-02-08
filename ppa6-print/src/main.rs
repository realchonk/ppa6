use std::path::{Path, PathBuf};
use anyhow::Result;
use clap::Parser;
use clap_num::maybe_hex;
use image::{imageops::{dither, ColorMap, FilterType}, DynamicImage, GrayImage, ImageFormat, ImageReader, Luma};
use ppa6::{usb_context, Document, Printer};
use itertools::Itertools;

#[derive(Parser)]
struct Cli {
	/// Path to the file to be printed.
	file: PathBuf,

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

	/// Threshold for dithering
	#[arg(short, long, default_value_t = 0x80, value_parser = maybe_hex::<u8>)]
	threshold: u8,
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

fn main() -> Result<()> {
	let cli = Cli::parse();

	let img = ImageReader::open(&cli.file)?
		.with_guessed_format()?
		.decode()?
		.into_luma8();
	let mut img = resize(rotate(img, cli.rotate));
	assert_eq!(img.width(), 384);
	dither(&mut img, &BlackWhiteMap(cli.threshold));

	if cli.show {
		let temppath = Path::new("/tmp/ppa6-preview.png");
		img.save_with_format(temppath, ImageFormat::Png)?;
		open::that(temppath)?;
		return Ok(());
	}

	let pixels = img
		.pixels()
		.map(|c| (c.0[0] < cli.threshold) ^ cli.invert)
		.chunks(8)
		.into_iter()
		.map(|chunk| {
			chunk
				.enumerate()
				.fold(0u8, |mut acc, (i, c)|  {
					assert!(i < 8);
					if c {
						acc |= 128 >> i;
					}
					acc
				})
		})
		.collect::<Vec<u8>>();

	let doc = Document::new(pixels)?;

	let ctx = usb_context()?;
	let mut printer = Printer::find(&ctx)?;

	for i in 0..cli.num {
		printer.print(&doc, cli.feed && i == (cli.num - 1))?;
	}

	Ok(())
}
