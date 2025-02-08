use std::{io::Read, path::PathBuf};
use ppa6::{usb_context, Document, Printer};

#[derive(Debug)]
struct Job {
	id: String,
	user: String,
	title: String,
	num: u32,
	options: String,
	path: Option<PathBuf>,
}

fn parse_cli() -> Option<Job> {
	let mut args = std::env::args();

	Some(Job {
		id: args.next()?,
		user: args.next()?,
		title: args.next()?,
		num: args.next()?.parse().ok()?,
		options: args.next()?,
		path: args.next().map(PathBuf::from),
	})
}

fn main() {
	let Some(job) = parse_cli() else {
		eprintln!("usage: ppa6 job_id user job_name ncopies options [file]");
		std::process::exit(1)
	};

	dbg!(&job);

	let ctx = usb_context().expect("failed to load libusb");
	let mut printer = Printer::find(&ctx).expect("no PeriPage A6 found");

	let pixels = match job.path.as_deref() {
		Some(path) => std::fs::read(path).expect("failed to read file"),
		None => {
			let mut buf = Vec::new();
			std::io::stdin().read_to_end(&mut buf).expect("failed to read stdin");
			buf
		}
	};
	let doc = Document::new(pixels).expect("failed to create document");

	for _ in 0..job.num {
		printer.print(&doc, true).expect("failed to print");
	}
}
