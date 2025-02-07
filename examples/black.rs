use ppa6::{Document, Printer};

fn main() {
	let ctx = ppa6::usb_context().unwrap();
	let mut printer = Printer::find(&ctx).unwrap();

	let pixels = vec![0xffu8; 48 * 384];
	let doc = Document::new(pixels).unwrap();
	printer.print(&doc, true).unwrap();
}
