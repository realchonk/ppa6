use std::{convert::identity, fmt::{self, Display, Formatter}, sync::Arc};

use iced::{advanced::{layout, renderer::Quad, Widget}, application, widget::{button, column, pick_list, row, text, text_input}, Color, Element, Length, Rectangle, Size, Task};
use ppa6::{usb_context, Context, Device, Printer};

type Document = ppa6::Document<'static>;

struct DocumentView(Arc<Document>);

impl<Msg, Theme, Renderer> Widget<Msg, Theme, Renderer> for DocumentView
where
	Renderer: iced::advanced::renderer::Renderer
{
	fn size(&self) -> iced::Size<iced::Length> {
		Size {
			width: Length::Fill,
			height: Length::Fill,
		}
	}
	fn layout(
        &self,
        _tree: &mut iced::advanced::widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
		let mut w = self.0.width() as f32;
		let mut h = self.0.height() as f32;
		let s = limits.max();

		h *= s.width / w;
		w = s.width;
		if h > s.height {
			w *= s.height / h;
			h = s.height;
		}

		layout::Node::new(Size::new(w, h))
	}

	fn draw(
        &self,
        _tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
		let doc = &self.0;
		let b = layout.bounds();
		let w = b.width / doc.width() as f32;
		let h = b.height / doc.height() as f32;
		for y in 0..doc.height() {
			for x in 0..doc.width() {
				let pixel = self.0.get(x, y).unwrap();
				let color = if pixel { Color::BLACK } else { Color::WHITE };
				renderer.fill_quad(Quad {
					bounds: Rectangle {
						x: b.x + w * x as f32,
						y: b.y + h * y as f32,
						width: w,
						height: h,
					},
					..Quad::default()
				}, color);
			}
		}
	}
}

impl<'a, Msg, Theme, Renderer> From<DocumentView> for Element<'a, Msg, Theme, Renderer>
where
	Renderer: iced::advanced::renderer::Renderer
{
	fn from(value: DocumentView) -> Self {
		Self::new(value)
	}
}

struct App {
	ctx: Arc<Context>,
	printer: Option<Printer>,
	page: Page,
}

enum Page {
	Loading,
	Select {
		options: Vec<PrinterOption>,
		selected: Option<PrinterOption>,
	},
	Menu,
	PreviewPrint {
		doc: Arc<Document>,
		copies: String,
	},
	Error(String),
}

#[derive(Debug, Clone)]
enum Message {
	Error(String),
	PrintersLoaded(Vec<Device>),
	PrinterSelected(PrinterOption),
	RefreshPrinters,
	ConnectPrinter(PrinterOption),
	PrintPage(Arc<Document>),
	Back,
	SetNumCopies(String),
}

fn update(app: &mut App, msg: Message) -> Task<Message> {
	match msg {
		Message::Error(e) => app.page = Page::Error(e),
		Message::PrintersLoaded(p) => {
			app.page = if p.len() > 0 {
				let options = p
					.into_iter()
					.map(PrinterOption)
					.collect::<Vec<_>>();
				Page::Select {
					selected: Some(options[0].clone()),
					options,
				}
			} else {
				Page::Error("No printers available".into())
			};
		},
		Message::PrinterSelected(p) => {
			match &mut app.page {
				Page::Select { selected, .. } => *selected = Some(p),
				_ => unreachable!(),
			}
		},
		Message::RefreshPrinters => {
			let ctx = Arc::clone(&app.ctx);
			app.page = Page::Loading;
			return Task::perform(async move {
				match Printer::list(&ctx) {
					Ok(printers) => Message::PrintersLoaded(printers),
					Err(e) => Message::Error(e.to_string()),
				}
			}, identity);
		},
		Message::ConnectPrinter(p) => {
			match p.0.open() {
				Ok(p) => match Printer::open(p) {
					Ok(printer) => {
						app.printer = Some(printer);
						app.page = Page::Menu;
					},
					Err(e) => app.page = Page::Error(e.to_string()),
				},
				Err(e) => app.page = Page::Error(e.to_string()),
			}
		},
		Message::PrintPage(doc) => {
			app.page = Page::PreviewPrint {
				doc,
				copies: "1".into(),
			};
		},
		Message::Back => {
			match app.page {
				Page::PreviewPrint { .. } => app.page = Page::Menu,
				_ => unreachable!(),
			}
		},
		Message::SetNumCopies(cp) => match &mut app.page {
			Page::PreviewPrint { copies, .. } => *copies = cp,
			_ => unreachable!(),
		},
	}
	Task::none()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrinterOption(Device);

fn test_page() -> Arc<Document> {
	let mut pixels = vec![0u8; 48 * 384];
	pixels
		.iter_mut()
		.enumerate()
		.filter(|(i, _)| (i % 2 == 0) && (i / 48 / 8 % 2 == 0))
		.for_each(|(_, b)| *b = 0xff);
	Arc::new(Document::new(pixels).unwrap())
}

impl Display for PrinterOption {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		write!(f, "{:?}", self.0)
	}
}

fn view(app: &App) -> Element<Message> {
	match &app.page {
		Page::Error(e) => {
			column![
				text(format!("An error occured: {e}")),
				button("Restart")
					.on_press(Message::RefreshPrinters)
			].into()
		},
		Page::Loading => text("Loading printers...").into(),
		Page::Select { options, selected } => {
			column! [
				text("Printers:"),
				pick_list(
					options.clone(),
					selected.clone(),
					Message::PrinterSelected
				),
				row! [
					button("Refresh")
						.on_press(Message::RefreshPrinters),
					button("Connect")
						.on_press_maybe({
							selected
								.as_ref()
								.cloned()
								.map(Message::ConnectPrinter)
						})
				]
			].into()
		},
		Page::Menu => {
			column![
				text("Main Menu"),
				button("Test Page")
					.on_press(Message::PrintPage(test_page()))
			].into()
		},
		Page::PreviewPrint { doc, copies } => {
			column! [
				text("Printing Preview"),
				DocumentView(Arc::clone(doc)),
				row! [
					button("Back")
						.on_press(Message::Back),
					text_input("number of copies", &copies)
						.on_input(Message::SetNumCopies),
					button("Print"),
				]
			].into()
		},
	}
}

fn main() -> iced::Result {
	let ctx = usb_context().expect("failed to load libusb");
	application("PeriPage A6 App", update, view)
		.run_with(|| {
			let ctx = Arc::new(ctx);
			let app = App {
				ctx: ctx.clone(),
				page: Page::Loading,
				printer: None,
			};

			let task = Task::perform(async move {
				match Printer::list(&ctx) {
					Ok(printers) => Message::PrintersLoaded(printers),
					Err(e) => Message::Error(e.to_string()),
				}
			}, identity);
			
			(app, task)
		})
}
