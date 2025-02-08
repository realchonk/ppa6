use std::sync::Arc;

use iced::{advanced::{layout, renderer::Quad, Widget}, Color, Element, Length, Rectangle, Size};

use crate::Document;


pub struct DocumentView(pub Arc<Document>);

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
