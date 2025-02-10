use std::{fmt::{self, Debug, Display, Formatter}, time::Duration};

use anyhow::{bail, Context, Result};

macro_rules! backends {
	[$($(# [$($m:tt)*])? $mod:ident :: $name:ident),* $(,)?] => {
		$(
			$(# [$($m)*])*
			mod $mod;
			$(# [$($m)*])*
			pub use crate::$mod::$name;
		)*
	};
}

backends! [
	#[cfg(feature = "usb")]
	usb::UsbBackend,
];


/// Printing backend.
pub trait Backend {
	/// Send data to the printer.
	/// TODO: return number of bytes sent
	fn send(&mut self, buf: &[u8], timeout: Duration) -> Result<()>;

	/// Receive at most `buf.len()` bytes of data from the printer.
	///
	/// # Return value
	/// This functions the number of bytes received from the printer.
	fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize>;
}

/// MAC Address, see [`Printer::get_mac()`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MacAddr(pub [u8; 6]);

/// PeriPage A6 printer.
pub struct Printer {
	backend: Box<dyn Backend>,
}

impl Printer {
	/// Construct a new printer using `backend` as it's printing [`Backend`].
	pub fn new(backend: impl Backend + 'static) -> Self {
		Self {
			backend: Box::new(backend),
		}
	}

	/// Find any printer, connected using any backend.
	pub fn find() -> Result<Self> {
		#[cfg(feature = "usb")] {
			match crate::usb::UsbBackend::list() {
				Ok(devs) => {
					if let Some(dev) = devs.first() {
						let backend = UsbBackend::open(dev)?;
						return Ok(Self::new(backend));
					}
				},
				Err(e) => log::error!("cannot get list of usb devices: {e}"),
			}
		}
		
		bail!("no printer found");
	}

	fn send(&mut self, buf: &[u8], timeout: u64) -> Result<()> {
		log::trace!("send({}{buf:x?}, {timeout}s);", buf.len());
		self.backend.send(buf, Duration::from_secs(timeout))
	}
	fn recv(&mut self, buf: &mut [u8], timeout: u64) -> Result<usize> {
		let n = self.backend.recv(buf, Duration::from_secs(timeout))?;
		log::trace!("recv({}, {timeout}s): {n}{:x?}", buf.len(), &buf[0..n]);
		Ok(n)
	}
	fn query(&mut self, cmd: &[u8]) -> Result<Vec<u8>> {
		self.send(cmd, 3).context("failed to send request")?;
		let mut buf = vec![0u8; 1024];
		let n = self.recv(&mut buf, 3).context("failed receive response")?;
		buf.truncate(n);
		Ok(buf)
	}
	fn query_string(&mut self, cmd: &[u8]) -> Result<String> {
		let buf = self.query(cmd)?;
		let s = String::from_utf8_lossy(&buf);
		Ok(s.into_owned())
	}

	/// Get printer's "IP" string.
	pub fn get_ip(&mut self) -> Result<String> {
		self.query_string(&[0x10, 0xff, 0x20, 0xf0])
	}

	/// Get printer's firmware version.
	pub fn get_firmware_ver(&mut self) -> Result<String> {
		self.query_string(&[0x10, 0xff, 0x20, 0xf1])
	}

	/// Get printer's serial number.
	pub fn get_serial(&mut self) -> Result<String> {
		self.query_string(&[0x10, 0xff, 0x20, 0xf2])
	}

	/// Get printer's hardware version.
	pub fn get_hardware_ver(&mut self) -> Result<String> {
		self.query_string(&[0x10, 0xff, 0x30, 0x10])
	}

	/// Get printer's name.
	pub fn get_name(&mut self) -> Result<String> {
		self.query_string(&[0x10, 0xff, 0x30, 0x11])
	}
	
	/// Get printer's MAC address.
	/// TODO: Return a MacAddr struct i
	pub fn get_mac(&mut self) -> Result<MacAddr> {
		let buf = self.query(&[0x10, 0xff, 0x30, 0x12])?;
		// for some reason the printer sends the MAC address twice
		if buf.len() < 6 {
			bail!("invalid MAC address response, got {} bytes: {:x?}", buf.len(), &buf);
		}
		let mut mac = [0u8; 6];
		mac.copy_from_slice(&buf[0..6]);
		Ok(MacAddr(mac))
	}

	/// Get printer's battery state.
	pub fn get_battery(&mut self) -> Result<u8> {
		let buf = self.query(&[0x10, 0xff, 0x50, 0xf1])?;
		if buf.len() != 2 {
			bail!("invalid battery response");
		}
		Ok(buf[1])
	}

	/// Set printing concentration, valid values are between `0..=2`.
	pub fn set_concentration(&mut self, c: u8) -> Result<()> {
		if c > 2 {
			bail!("invalid concentration: {c}");
		}

		self.send(&[0x10, 0xff, 0x10, 0x00, c], 1)
	}

	/// Reset the printer.
	/// This command has to be sent, before printing can be done.
	pub fn reset(&mut self) -> Result<()> {
		let buf = [
			0x10, 0xff, 0xfe, 0x01,
			0x00, 0x00, 0x00, 0x00,
			0x00, 0x00, 0x00, 0x00,
			0x00, 0x00, 0x00, 0x00,
		];
		self.send(&buf, 3)?;
		let mut buf = [0u8; 128];
		let _ = self.backend.recv(&mut buf, Duration::from_secs(1));
		Ok(())
	}

	/// Print ASCII text.
	/// Please don't use this, better use a font rasterizer, like [cosmic-text](https://docs.rs/cosmic-text).
	///
	/// # Printer Bugs (PeriPage A6)
	/// - Only ASCII, no Unicode
	/// - No ASCII escape sequences, except '\n' (line feed)
	/// - Line wrapping is very buggy, sometimes it works, sometimes it discards the rest of the line.
	/// - No font size/weight settings
	pub fn print_text(&mut self, text: &str) -> Result<()> {
		let text: Vec<u8> = text
			.chars()
			.filter(|ch| matches!(ch, '\n' | '\x20'..='\x7f'))
			.map(|ch| ch as u8)
			.collect();

		self.send(&text, 30)?;
		Ok(())
	}

	/// Print raw pixels.
	///
	/// # Overheating
	/// The printer can overheat, if too much black is being printed at once,
	/// therefore it's better to use the [`Printer::print_image_chunked()`] function instead.
	///
	/// # Printing Limitations
	/// While the printer has a density of 203dpi,
	/// printing very small things and thin lines should be avoided,
	/// as the printer is simply not precise enough.
	///
	/// # "Concentration"
	/// The printing concentration can be adjusted with [`Printer::set_concentration()`],
	/// to make the output brighter or darker.
	///
	/// # Format
	/// TODO: describe pixel format:
	/// - monochrome
	/// - 0=white, 1=black
	/// - MSB: left, LSB: right
	/// - must be multiples of `width/8` bytes
	/// - must not be longer than `65535` rows
	/// - due to accuracy constraints, printing single pixels should be avoided
	///
	/// # Notes
	/// Printing gray scale pictures is possible,
	/// by using [dithering](https://en.wikipedia.org/wiki/Dithering) to convert them to monochrome first.
	/// Similarly, color images must be first converted to gray scale.
	/// The [image](https://docs.rs/image/latest/image/) crate can be used, to do the conversions.
	pub fn print_image(&mut self, pixels: &[u8], width: u16) -> Result<()> {
		if width == 0 || width % 8 != 0 {
			bail!("width must be non-zero and divisible by 8");
		}
		
		let n = pixels.len() * 8;
		let w = width as usize;
		let h = n / w;

		if h > 0xff {
			bail!("document too long");
		}

		if pixels.len() != (w * h / 8) {
			bail!("invalid length of pixels: {}", pixels.len());
		}

		let rs = w / 8;

		let mut packet = vec![
			0x1d, 0x76, 0x30,
			(rs >> 8) as u8, (rs & 0xff) as u8,
			0x00, h as u8, 0x00,
		];
		packet.extend_from_slice(pixels);
		self.send(&packet, 60)?;

		// no idea what this does, but the Windows driver sends this after every print.
		self.send(&[0x10, 0xff, 0xfe, 0x45], 1)?;
		Ok(())
	}

	/// Just like [`Printer::print_image()`], but breaks the pixels into rows of `chunk_height`.
	/// This may be needed, to prevent the printer from overheating, while printing a long document.
	pub fn print_image_chunked_ext(&mut self, pixels: &[u8], width: u16, chunk_height: u16, delay: Duration) -> Result<()> {
		pixels
			.chunks(width as usize * chunk_height as usize / 8)
			.try_for_each(|chunk| {
				self.print_image(chunk, width)?;
				std::thread::sleep(delay);
				Ok(())
			})
	}

	pub fn print_image_chunked(&mut self, pixels: &[u8], width: u16) -> Result<()> {
		self.print_image_chunked_ext(pixels, width, 24, Duration::from_millis(50))
	}

	/// Push out `num` rows of paper.
	pub fn push(&mut self, num: u8) -> Result<()> {
		self.send(&[0x1b, 0x4a, num], 5)?;
		Ok(())
	}
}

impl Display for MacAddr {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let [x0, x1, x2, x3, x4, x5] = self.0;
		write!(f, "{x0:02x}:{x1:02x}:{x2:02x}:{x3:02x}:{x4:02x}:{x5:02x}")
	}
}

impl Debug for MacAddr {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		<Self as Display>::fmt(self, f)
	}
}
