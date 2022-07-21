use async_trait::async_trait;
use toss::frame::{Frame, Size};

use super::{BoxedWidget, Widget};

pub struct Layer {
    layers: Vec<BoxedWidget>,
}

impl Layer {
    pub fn new(layers: Vec<BoxedWidget>) -> Self {
        Self { layers }
    }
}

#[async_trait]
impl Widget for Layer {
    fn size(&self, frame: &mut Frame, max_width: Option<u16>, max_height: Option<u16>) -> Size {
        let mut max_size = Size::ZERO;
        for layer in &self.layers {
            let size = layer.size(frame, max_width, max_height);
            max_size.width = max_size.width.max(size.width);
            max_size.height = max_size.height.max(size.height);
        }
        max_size
    }

    async fn render(self: Box<Self>, frame: &mut Frame) {
        for layer in self.layers {
            layer.render(frame).await;
        }
    }
}
