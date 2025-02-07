// Very helpful doc for USB: https://www.beyondlogic.org/usbnutshell/usb1.shtml
use std::{iter::repeat_n, time::Duration};

use rusb::{Context, DeviceHandle, Direction, TransferType, UsbContext};
use thiserror::Error;

pub use crate::doc::{Document, DocumentError};
pub use rusb as usb;

/// USB vendor ID of the PeriPage A6.
pub const VENDOR_ID: u16 = 0x09c5;

/// USB product ID of the PeriPage A6.
pub const PRODUCT_ID: u16 = 0x0200;

#[derive(Debug, Error)]
pub enum Error {
	#[error("USB problem")]
	Usb(#[from] rusb::Error),

	#[error("failed to claim the USB device")]
	Claim(#[source] rusb::Error),
	
	#[error("no PeriPage A6 found")]
	NoPrinter,
}

pub type Result<T> = core::result::Result<T, Error>;

mod doc;

pub struct Printer {
	handle: DeviceHandle<Context>,
	epin: u8,
	epout: u8,
}

impl Printer {
	pub fn find(ctx: &Context) -> Result<Self> {
		let dev = ctx
			.devices()?
			.iter()
			.find(|dev| {
				let Ok(desc) = dev.device_descriptor() else {
					log::warn!("cannot get device descriptor for Bus {dev:?}");
					return false
				};

				desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID
			})
			.ok_or(Error::NoPrinter)?;

		Self::open(dev.open()?)
	}
	pub fn open(handle: DeviceHandle<Context>) -> Result<Self> {
		let dev = handle.device();

		// automatically steal the USB device from the kernel
		let _ = handle.set_auto_detach_kernel_driver(true);

		let dd = dev.device_descriptor()?;
		log::trace!("USB device descriptor = {dd:#?}");
		if let Ok(s) = handle.read_manufacturer_string_ascii(&dd) {
			log::debug!("USB Vendor: {s}");
		}
		if let Ok(s) = handle.read_product_string_ascii(&dd) {
			log::debug!("USB Product: {s}");
		}
		if let Ok(s) = handle.read_serial_number_string_ascii(&dd) {
			log::debug!("USB Serial: {s}");
		}

		// PeriPage A6 has only one config.
		debug_assert_eq!(dd.num_configurations(), 1);

		let cd = dev.config_descriptor(0)?;
		log::trace!("USB configuration descriptor 0: {cd:#?}");

		// PeriPage A6 has only one interface.
		debug_assert_eq!(cd.num_interfaces(), 1);

		let int = cd.interfaces().next().unwrap();
		let id = int.descriptors().next().unwrap();
		log::trace!("USB interface descriptor 0 for configuration 0: {id:#?}");
		if let Some(sid) = id.description_string_index() {
			log::trace!("Interface: {}", handle.read_string_descriptor_ascii(sid)?);
		}

		log::debug!("Is kernel driver active: {:?}", handle.kernel_driver_active(0));

		debug_assert_eq!(id.class_code(), 7); // Printer
		debug_assert_eq!(id.sub_class_code(), 1); // Printer
		debug_assert_eq!(id.protocol_code(), 2); // Bi-directional
		assert_eq!(id.num_endpoints(), 2); 

		let mut endps = id.endpoint_descriptors();
		let epd0 = endps.next().unwrap();
		let epd1 = endps.next().unwrap();
		debug_assert!(endps.next().is_none());

		log::trace!("USB endpoint descriptor 0: {epd0:#?}");
		log::trace!("USB endpoint descriptor 1: {epd1:#?}");

		debug_assert_eq!(epd0.address(), 129); // IN (128) + 1
		assert_eq!(epd0.direction(), Direction::In);
		assert_eq!(epd0.transfer_type(), TransferType::Bulk);

		debug_assert_eq!(epd1.address(), 2); // OUT (0) + 2
		assert_eq!(epd1.direction(), Direction::Out);
		assert_eq!(epd1.transfer_type(), TransferType::Bulk);

		Ok(Self {
			handle,
			epin: epd0.address(),
			epout: epd1.address(),
		})
	}

	/// Run action `f` while the interface is claimed.
	fn run<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
		self.handle.claim_interface(0)
			.map_err(|e| Error::Claim(e))?;
		let x = f(self);
		if let Err(e) = self.handle.release_interface(0) {
			log::error!("failed to unclaim device: {e}");
		}
		x
	}

	/// Write data to the USB device.
	/// NOTE: This function must be run inside of `Self::run()`
	fn write(&mut self, buf: &[u8], timeout: u64) -> Result<()> {
		self.handle.write_bulk(self.epout, buf, Duration::from_secs(timeout))?;
		Ok(())
	}

	pub fn print(&mut self, doc: &Document, extra: bool) -> Result<()> {
		let mut packet = vec![
			0x10, 0xff, 0xfe, 0x01,
			0x1b, 0x40, 0x00, 0x1b,
			0x4a, 0x60,
		];

		let chunk_width = doc.width() / 8;
		let chunk_height  = 24; // This number was derived from USB traffic.
		let chunk_size = chunk_width * chunk_height;

		// TODO: allow pages smaller than 384px
		assert_eq!(chunk_width, 48);

		let page_header = &[
			0x1d, 0x76, 0x30, 0x00, 0x30, 0x00,
		];

		// Group the pixels into pages, because that's how the Windows driver does it.
		doc
			.pixels()
			.chunks(chunk_size)
			.for_each(|chunk| {
				packet.extend_from_slice(page_header);
				packet.extend_from_slice(&u16::to_le_bytes(chunk_height as u16));
				packet.extend_from_slice(chunk);
				if chunk.len() < chunk_size {
					packet.extend(repeat_n(0u8, chunk_size - chunk.len()));
				}
			});

		if extra {
			let height = 3 * 24;
			packet.extend_from_slice(page_header);
			packet.extend_from_slice(&u16::to_le_bytes(height));
			packet.extend(repeat_n(0u8, 48 * height as usize));
		}
		
		self.run(|s| {
			s.write(&packet, 30)?;
			s.write(&[0x10, 0xff, 0xfe, 0x45], 1)?;
			Ok(())
		})
	}

	pub fn handle(&mut self) -> &DeviceHandle<Context> {
		&mut self.handle
	}
	pub fn endpoint_in(&self) -> u8 {
		self.epin
	}
	pub fn endpoint_out(&self) -> u8 {
		self.epout
	}
}

pub fn usb_context() -> usb::Result<usb::Context> {
	Context::new()
}
