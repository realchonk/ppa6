// Very helpful doc for USB: https://www.beyondlogic.org/usbnutshell/usb1.shtml
use anyhow::{Result, Context as _};
use std::{iter::repeat_n, time::Duration};
use rusb::{Direction, TransferType, UsbContext, Context, DeviceHandle};

const VENDOR_ID: u16 = 0x09c5;
const PRODUCT_ID: u16 = 0x0200;

struct Peripage {
	handle: DeviceHandle<Context>,
	ep: u8,
}

impl Peripage {
	fn find(ctx: &Context) -> Result<Self> {
		let dev = ctx
			.devices()
			.context("cannot read list of devices")?
			.iter()
			.find(|dev| {
				let Ok(desc) = dev.device_descriptor() else {
					eprintln!("cannot get device descriptor for Bus {dev:?}");
					return false
				};

				desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID
			})
			.context("no Peripage A6 found")?;

		let handle = dev.open().context("cannot open usb device")?;
		Self::connect(handle)
	}

	fn connect(handle: DeviceHandle<Context>) -> Result<Self> {
		let dev = handle.device();
		let _ = handle.set_auto_detach_kernel_driver(false);

		let dd = dev.device_descriptor().unwrap();
		println!("Device Descriptor = {dd:#?}");
		assert_eq!(dd.vendor_id(), VENDOR_ID);
		assert_eq!(dd.product_id(), PRODUCT_ID);
		if let Ok(s) = handle.read_manufacturer_string_ascii(&dd) {
			println!("Vendor: {s}");
		}
		if let Ok(s) = handle.read_product_string_ascii(&dd) {
			println!("Product: {s}");
		}
		if let Ok(s) = handle.read_serial_number_string_ascii(&dd) {
			println!("Serial: {s}");
		}

		assert_eq!(dd.num_configurations(), 1);
		let cd = dev.config_descriptor(0).unwrap();
		println!("Config Descriptor = {cd:#?}");

		assert_eq!(cd.num_interfaces(), 1);
		let int = cd.interfaces().next().unwrap();
		let id = int.descriptors().next().unwrap();
		println!("Interface Descriptor = {id:#?}");
		if let Some(sid) = id.description_string_index() {
			println!("Interface: {}", handle.read_string_descriptor_ascii(sid).unwrap());
		}
		let kactive = handle.kernel_driver_active(0);
		println!("Is kernel driver attached: {kactive:?}");

		assert_eq!(id.class_code(), 7); // Printer
		assert_eq!(id.sub_class_code(), 1); // Printer
		assert_eq!(id.protocol_code(), 2); // Bi-directional
		assert_eq!(id.num_endpoints(), 2);

		let mut epds = id.endpoint_descriptors();
		let epd0 = epds.next().unwrap();
		let epd1 = epds.next().unwrap();
		println!("Endpoint Descriptor 0: {epd0:#?}");
		println!("Endpoint Descriptor 1: {epd1:#?}");

		assert_eq!(epd0.address(), 129); // IN (128) + 1
		assert_eq!(epd0.direction(), Direction::In);
		assert_eq!(epd0.transfer_type(), TransferType::Bulk);

		assert_eq!(epd1.address(), 2); // OUT (0) + 2
		assert_eq!(epd1.direction(), Direction::Out);
		assert_eq!(epd1.transfer_type(), TransferType::Bulk);

		let ep = epd1.address();

		if let Ok(true) = kactive {
			handle
				.detach_kernel_driver(0)
				.context("failed to detach kernel driver")?;
		}

		handle
			.claim_interface(0)
			.context("failed to claim interface 0")?;

		Ok(Self {
			handle,
			ep,
		})
	}

	fn write(&mut self, buf: &[u8], timeout: u64) -> Result<()> {
		self.handle.write_bulk(self.ep, buf, Duration::from_secs(timeout))?;
		Ok(())
	}

	fn newline(&mut self) -> Result<()> {
		let buf = &[0x10, 0xff, 0xfe, 0x01];
		self.write(buf, 1)
	}

	fn confirm(&mut self) -> Result<()> {
		let buf = &[0x10, 0xff, 0xfe, 0x45];
		self.write(buf, 1)
	}
}

impl Drop for Peripage {
	fn drop(&mut self) {
		let _ = self.handle.release_interface(0);
	}
}

fn main() {
	println!("libusb version: {:?}", rusb::version());
	println!("Kernel supports detaching driver: {}", rusb::supports_detach_kernel_driver());

	
	let ctx = rusb::Context::new().expect("cannot connect to libusb");

	let mut pp = Peripage::find(&ctx).unwrap();

	let mut img = vec![0u8; 5 * 48 * 24];

	img
		.iter_mut()
		.enumerate()
		.filter(|(i, _)| i % 2 == 0 && (i / 48 / 4) % 2 == 0)
		.for_each(|(_, b)| *b = 0xff);

	let header = &[
		0x10, 0xff, 0xfe, 0x01,
		0x1b, 0x40, 0x00, 0x1b,
		0x4a, 0x60,
	];

	let mut packet = Vec::new();
	packet.extend_from_slice(header);

	const HEIGHT: u16 = 512;
	const BPC: usize = 48 * HEIGHT as usize;
	img
		.chunks(BPC)
		.chain(std::iter::repeat_n(&[0u8; BPC] as &[u8], 0))
		.for_each(|chunk| {
			packet.extend_from_slice(&[
				0x1d, 0x76, 0x30, 0x00, 0x30, 0x00,
			]);
			packet.extend_from_slice(&HEIGHT.to_le_bytes());
			packet.extend_from_slice(chunk);
			if chunk.len() < BPC {
				let z = repeat_n(0u8, BPC - chunk.len());
				packet.extend(z);
			}
		});

	// send image
	pp.write(&packet, 30).unwrap();
	pp.confirm().unwrap();

	pp.write(&packet, 30).unwrap();
	pp.confirm().unwrap();
}
