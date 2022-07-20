use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};

use super::Widget;

pub struct Segment {
    widget: Box<dyn Widget + Send>,
    expanding: bool,
}

impl Segment {
    pub fn new<W: 'static + Widget + Send>(widget: W) -> Self {
        Self {
            widget: Box::new(widget),
            expanding: false,
        }
    }

    pub fn expanding(mut self, active: bool) -> Self {
        self.expanding = active;
        self
    }
}

fn expand(amounts: &mut [(u16, bool)], total: u16) {
    // Weirdly, rustc needs this type annotation while rust-analyzer manages to
    // derive the correct type in an inlay hint.
    let actual: u16 = amounts.iter().map(|(a, _)| *a).sum();
    if actual < total {
        let mut remaining = total - actual;
        while remaining > 0 {
            for (amount, expanding) in amounts.iter_mut() {
                if *expanding {
                    if remaining > 0 {
                        *amount += 1;
                        remaining -= 1;
                    } else {
                        break;
                    }
                }
            }
        }
    }
}

/// Place multiple widgets next to each other horizontally.
pub struct HJoin {
    segments: Vec<Segment>,
}

impl HJoin {
    pub fn new(segments: Vec<Segment>) -> Self {
        Self { segments }
    }
}

#[async_trait]
impl Widget for HJoin {
    fn size(&self, frame: &mut Frame, _max_width: Option<u16>, max_height: Option<u16>) -> Size {
        let mut size = Size::ZERO;
        for segment in &self.segments {
            let widget_size = segment.widget.size(frame, None, max_height);
            size.width += widget_size.width;
            size.height = size.height.max(widget_size.height);
        }
        size
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();
        let mut widths = self
            .segments
            .iter()
            .map(|s| {
                let width = s.widget.size(frame, None, Some(size.height)).width;
                (width, s.expanding)
            })
            .collect::<Vec<_>>();
        expand(&mut widths, size.width);

        let mut x = 0;
        for (segment, (width, _)) in self.segments.into_iter().zip(widths.into_iter()) {
            frame.push(Pos::new(x, 0), Size::new(width, size.height));
            segment.widget.render(frame).await;
            frame.pop();
            x += width as i32;
        }
    }
}

/// Place multiple widgets next to each other vertically.
pub struct VJoin {
    segments: Vec<Segment>,
}

impl VJoin {
    pub fn new(segments: Vec<Segment>) -> Self {
        Self { segments }
    }
}

#[async_trait]
impl Widget for VJoin {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, _max_height: Option<u16>) -> Size {
        let mut size = Size::ZERO;
        for segment in &self.segments {
            let widget_size = segment.widget.size(frame, max_width, None);
            size.width = size.width.max(widget_size.width);
            size.height += widget_size.height;
        }
        size
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();
        let mut heights = self
            .segments
            .iter()
            .map(|s| {
                let height = s.widget.size(frame, Some(size.width), None).height;
                (height, s.expanding)
            })
            .collect::<Vec<_>>();
        expand(&mut heights, size.height);

        let mut y = 0;
        for (segment, (height, _)) in self.segments.into_iter().zip(heights.into_iter()) {
            frame.push(Pos::new(0, y), Size::new(size.width, height));
            segment.widget.render(frame).await;
            frame.pop();
            y += height as i32;
        }
    }
}
