use ppa6::Printer;

fn main() {
    let mut printer = Printer::find().expect("no printer found");
    printer.reset().expect("cannot reset printer");
    printer
		.print_text("Hello \tWorld\nThis is a sample page of text\n\nThe printer also handles newlines, but wrapping seems to be partially broken for some reason.")
		.expect("cannot print text");
    printer.push(0x40).expect("cannot push printer paper");
}
