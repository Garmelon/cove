use async_trait::async_trait;
use toss::frame::{Frame, Pos, Size};

use super::{BoxedWidget, Widget};

pub struct Segment {
    widget: BoxedWidget,
    expanding: bool,
    priority: Option<u8>,
}

impl Segment {
    pub fn new<W: Into<BoxedWidget>>(widget: W) -> Self {
        Self {
            widget: widget.into(),
            expanding: false,
            priority: None,
        }
    }

    /// Expand this segment into the remaining space after all segment minimum
    /// sizes have been determined. The remaining space is split up evenly.
    pub fn expanding(mut self, active: bool) -> Self {
        self.expanding = active;
        self
    }

    /// The size of segments with a priority is calculated in order of
    /// increasing priority, using the remaining available space as maximum
    /// space for the widget during size calculations.
    ///
    /// Widgets without priority are processed first without size restrictions.
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = Some(priority);
        self
    }
}

struct SizedSegment {
    idx: usize,
    size: Size,
    expanding: bool,
    priority: Option<u8>,
}

impl SizedSegment {
    pub fn new(idx: usize, segment: &Segment) -> Self {
        Self {
            idx,
            size: Size::ZERO,
            expanding: segment.expanding,
            priority: segment.priority,
        }
    }
}

fn sizes_horiz(
    segments: &[Segment],
    frame: &mut Frame,
    max_width: Option<u16>,
    max_height: Option<u16>,
) -> Vec<SizedSegment> {
    let mut sized = segments
        .iter()
        .enumerate()
        .map(|(i, s)| SizedSegment::new(i, s))
        .collect::<Vec<_>>();
    sized.sort_by_key(|s| s.priority);

    let mut total_width = 0;
    for s in &mut sized {
        let available_width = max_width
            .filter(|_| s.priority.is_some())
            .map(|w| w.saturating_sub(total_width));
        s.size = segments[s.idx]
            .widget
            .size(frame, available_width, max_height);
        if let Some(available_width) = available_width {
            s.size.width = s.size.width.min(available_width);
        }
        total_width += s.size.width;
    }

    sized
}

fn sizes_vert(
    segments: &[Segment],
    frame: &mut Frame,
    max_width: Option<u16>,
    max_height: Option<u16>,
) -> Vec<SizedSegment> {
    let mut sized = segments
        .iter()
        .enumerate()
        .map(|(i, s)| SizedSegment::new(i, s))
        .collect::<Vec<_>>();
    sized.sort_by_key(|s| s.priority);

    let mut total_height = 0;
    for s in &mut sized {
        let available_height = max_height
            .filter(|_| s.priority.is_some())
            .map(|w| w.saturating_sub(total_height));
        s.size = segments[s.idx]
            .widget
            .size(frame, max_width, available_height);
        if let Some(available_height) = available_height {
            s.size.height = s.size.height.min(available_height);
        }
        total_height += s.size.height;
    }

    sized
}

fn expand_horiz(segments: &mut [SizedSegment], available_width: u16) {
    if !segments.iter().any(|s| s.expanding) {
        return;
    }

    // Interestingly, rustc needs this type annotation while rust-analyzer
    // manages to derive the correct type in an inlay hint.
    let current_width = segments.iter().map(|s| s.size.width).sum::<u16>();
    if current_width < available_width {
        let mut remaining_width = available_width - current_width;
        while remaining_width > 0 {
            for segment in segments.iter_mut() {
                if segment.expanding {
                    if remaining_width > 0 {
                        segment.size.width += 1;
                        remaining_width -= 1;
                    } else {
                        break;
                    }
                }
            }
        }
    }
}

fn expand_vert(segments: &mut [SizedSegment], available_height: u16) {
    if !segments.iter().any(|s| s.expanding) {
        return;
    }

    // Interestingly, rustc needs this type annotation while rust-analyzer
    // manages to derive the correct type in an inlay hint.
    let current_height = segments.iter().map(|s| s.size.height).sum::<u16>();
    if current_height < available_height {
        let mut remaining_height = available_height - current_height;
        while remaining_height > 0 {
            for segment in segments.iter_mut() {
                if segment.expanding {
                    if remaining_height > 0 {
                        segment.size.height += 1;
                        remaining_height -= 1;
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
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        let sizes = sizes_horiz(&self.segments, frame, max_width, max_height);
        let width = sizes.iter().map(|s| s.size.width).sum::<u16>();
        let height = sizes.iter().map(|s| s.size.height).max().unwrap_or(0);
        Size::new(width, height)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();

        let mut sizes = sizes_horiz(&self.segments, frame, Some(size.width), Some(size.height));
        expand_horiz(&mut sizes, size.width);

        sizes.sort_by_key(|s| s.idx);
        let mut x = 0;
        for (segment, sized) in self.segments.into_iter().zip(sizes.into_iter()) {
            frame.push(Pos::new(x, 0), Size::new(sized.size.width, size.height));
            segment.widget.render(frame).await;
            frame.pop();

            x += sized.size.width as i32;
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
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        let sizes = sizes_vert(&self.segments, frame, max_width, max_height);
        let width = sizes.iter().map(|s| s.size.width).max().unwrap_or(0);
        let height = sizes.iter().map(|s| s.size.height).sum::<u16>();
        Size::new(width, height)
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        let size = frame.size();

        let mut sizes = sizes_vert(&self.segments, frame, Some(size.width), Some(size.height));
        expand_vert(&mut sizes, size.height);

        sizes.sort_by_key(|s| s.idx);
        let mut y = 0;
        for (segment, sized) in self.segments.into_iter().zip(sizes.into_iter()) {
            frame.push(Pos::new(0, y), Size::new(size.width, sized.size.height));
            segment.widget.render(frame).await;
            frame.pop();

            y += sized.size.height as i32;
        }
    }
}
