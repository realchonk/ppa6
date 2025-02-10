use std::time::Duration;
use anyhow::{Result, Context};
use rusb::{Direction, GlobalContext, TransferType};

const VENDOR_ID: u16 = 0x09c5;
const PRODUCT_ID: u16 = 0x0200;

use crate::Backend;

pub type Device = rusb::Device<GlobalContext>;
pub type DeviceHandle = rusb::DeviceHandle<GlobalContext>;

/// A USB backend for [`Printer`](crate::Printer).
pub struct UsbBackend {
	handle: DeviceHandle,
	epin: u8,
	epout: u8,
}

impl UsbBackend {
	/// Get a list of printer devices connected via usb.
	pub fn list() -> rusb::Result<Vec<Device>> {
		let devs = rusb::devices()?
			.iter()
			.filter(|dev| {
				let Ok(desc) = dev.device_descriptor() else {
					log::error!("cannot get device descriptor for device {dev:?}");
					return false
				};

				desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID
			})
			.collect();
		Ok(devs)
	}

	/// Open a USB printing device.
	pub fn open(dev: &Device) -> Result<Self> {
		let handle = dev
			.open()
			.context("cannot open usb device")?;

		// automatically steal the USB device from the kernel
		let _ = handle.set_auto_detach_kernel_driver(true);

		let dd = dev
			.device_descriptor()
			.context("cannot get usb device descriptor")?;

		log::debug!("USB device descriptor = {dd:#?}");
		if let Ok(s) = handle.read_manufacturer_string_ascii(&dd) {
			log::info!("USB Vendor: {s}");
		}
		if let Ok(s) = handle.read_product_string_ascii(&dd) {
			log::info!("USB Product: {s}");
		}
		if let Ok(s) = handle.read_serial_number_string_ascii(&dd) {
			log::info!("USB Serial: {s}");
		}

		// PeriPage A6 has only one config.
		debug_assert_eq!(dd.num_configurations(), 1);

		let cd = dev
			.config_descriptor(0)
			.context("cannot get usb config descriptor")?;
		log::debug!("USB configuration descriptor 0: {cd:#?}");

		// PeriPage A6 has only one interface.
		debug_assert_eq!(cd.num_interfaces(), 1);

		let int = cd.interfaces().next().unwrap();
		let id = int.descriptors().next().unwrap();
		log::debug!("USB interface descriptor 0 for configuration 0: {id:#?}");
		if let Some(sid) = id.description_string_index() {
			log::debug!("Interface: {}", handle.read_string_descriptor_ascii(sid)?);
		}

		log::info!("Is usb kernel driver active: {:?}", handle.kernel_driver_active(0));

		debug_assert_eq!(id.class_code(), 7); // Printer
		debug_assert_eq!(id.sub_class_code(), 1); // Printer
		debug_assert_eq!(id.protocol_code(), 2); // Bi-directional
		assert_eq!(id.num_endpoints(), 2); 

		let mut endps = id.endpoint_descriptors();
		let epd0 = endps.next().unwrap();
		let epd1 = endps.next().unwrap();
		debug_assert!(endps.next().is_none());

		log::debug!("USB endpoint descriptor 0: {epd0:#?}");
		log::debug!("USB endpoint descriptor 1: {epd1:#?}");

		debug_assert_eq!(epd0.address(), 129); // IN (128) + 1
		assert_eq!(epd0.direction(), Direction::In);
		assert_eq!(epd0.transfer_type(), TransferType::Bulk);

		debug_assert_eq!(epd1.address(), 2); // OUT (0) + 2
		assert_eq!(epd1.direction(), Direction::Out);
		assert_eq!(epd1.transfer_type(), TransferType::Bulk);

		let epin = epd0.address();
		let epout = epd1.address();

		handle.claim_interface(0).context("cannot claim usb interface 0")?;

		Ok(Self {
			handle,
			epin,
			epout,
		})
	}
}

impl Backend for UsbBackend {
	fn send(&mut self, buf: &[u8], timeout: Duration) -> anyhow::Result<()> {
		self.handle.write_bulk(self.epout, buf, timeout)?;
		Ok(())
	}

	fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> anyhow::Result<usize> {
		let n = self.handle.read_bulk(self.epin, buf, timeout)?;
		Ok(n)
	}
}

