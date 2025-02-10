use ppa6::Printer;

fn main() {
	let mut printer = Printer::find().expect("no printer found");
	printer.reset().expect("failed to reset printer");
	let pixels = vec![0xffu8; 384 * 384 / 8];
	printer
		.print_image_chunked(&pixels, 384)
		.expect("failed to print black image");

}
