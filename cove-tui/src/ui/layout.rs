use tui::layout::Rect;

pub fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let dx = (area.width - width) / 2;
    let dy = (area.height - height) / 2;
    Rect {
        x: area.x + dx,
        y: area.y + dy,
        width,
        height,
    }
}

pub fn centered_v(height: u16, area: Rect) -> Rect {
    let height = height.min(area.height);
    let dy = (area.height - height) / 2;
    Rect {
        y: area.y + dy,
        height,
        ..area
    }
}
