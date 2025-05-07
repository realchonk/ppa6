use ppa6::Printer;

fn main() {
    let mut printer = Printer::find().expect("no printer found");
    printer.reset().expect("failed to reset printer");
    let mut pixels = vec![0u8; 384 * 384 / 8];

    pixels
        .iter_mut()
        .enumerate()
        .filter(|(i, _)| (i % 2 == 0))
        .for_each(|(_, p)| *p = 0xff);

    printer
        .print_image_chunked(&pixels, 384)
        .expect("failed to print image");

    printer.push(0x60).expect("failed to push paper");
}
