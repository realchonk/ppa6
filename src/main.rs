// Very helpful doc for USB: https://www.beyondlogic.org/usbnutshell/usb1.shtml
use rusb::{Direction, TransferType, UsbContext};

const VENDOR_ID: u16 = 0x09c5;
const PRODUCT_ID: u16 = 0x0200;

fn main() {
	println!("libusb version: {:?}", rusb::version());
	println!("Kernel supports detaching driver: {}", rusb::supports_detach_kernel_driver());

	
	let ctx = rusb::Context::new().expect("cannot connect to libusb");

	// Find Peripage A6
	let dev = ctx
		.devices()
		.expect("cannot read list of devices")
		.iter()
		.find(|dev| {
			let Ok(desc) = dev.device_descriptor() else {
				eprintln!("cannot get device descriptor for Bus {dev:?}");
				return false
			};

			desc.vendor_id() == VENDOR_ID && desc.product_id() == PRODUCT_ID
		})
		.expect("No Peripage A6 found")
		;

	let handle = dev.open().expect("cannot open usb device");

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
	println!("Is kernel driver attached: {:?}", handle.kernel_driver_active(0));

	assert_eq!(id.class_code(), 7);
	assert_eq!(id.sub_class_code(), 1);
	assert_eq!(id.protocol_code(), 2);
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

	handle.claim_interface(0).expect("failed to claim interface 0");
	handle.release_interface(0).expect("failed to release interface 0");
}
