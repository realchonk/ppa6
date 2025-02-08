use std::{borrow::Cow, fmt::{self, Formatter, Debug}};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DocumentError {
	#[error("document has an invalid width")]
	Width,

	#[error("expected a length of {0}, got {1}")]
	Len(usize, usize),
}

/// A document, to be printed.
pub struct Document<'a> {
	pixels: Cow<'a, [u8]>,
}

impl<'a> Document<'a> {
	/// The maximum width a document can have. (384px = 48mm)
	pub const WIDTH: usize = 384;

	/// Create a new document.
	pub fn new(pixels: impl Into<Cow<'a, [u8]>>) -> Result<Self, DocumentError> {
		Self::do_new(pixels.into())
	}

	fn do_new(pixels: Cow<'a, [u8]>) -> Result<Self, DocumentError> {
		let height = pixels.len() / Self::WIDTH;
		let expected = Self::WIDTH * height;
		if expected != pixels.len() {
			return Err(DocumentError::Len(expected, pixels.len()));
		}

		Ok(Self {
			pixels,
		})
	}

	pub fn width(&self) -> usize {
		Self::WIDTH
	}

	pub fn height(&self) -> usize {
		self.pixels.len() / (Self::WIDTH / 8)
	}

	pub fn pixels(&self) -> &[u8] {
		&self.pixels
	}

	pub fn get(&self, x: usize, y: usize) -> Option<bool> {
		if x >= self.width() || y >= self.height() {
			return None;
		}

		let b = self.pixels[(y * self.width() + x) / 8];
		Some(b & (128 >> (x % 8)) != 0)
	}
}


impl Debug for Document<'_> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f
			.debug_struct("Document")
			.field("width", &self.width())
			.field("height", &self.height())
			.finish()
	}
}
