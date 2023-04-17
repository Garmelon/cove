use async_trait::async_trait;
use toss::widgets::{Background, Border, Float, Layer2, Padding, Text};
use toss::{AsyncWidget, Frame, Size, Style, Styled, WidgetExt, WidthDb};

type Body<I> = Background<Border<Padding<I>>>;
type Title = Float<Padding<Background<Padding<Text>>>>;

pub struct Popup<I>(Float<Layer2<Body<I>, Title>>);

impl<I> Popup<I> {
    pub fn new<S: Into<Styled>>(inner: I, title: S) -> Self {
        let title = Text::new(title)
            .padding()
            .with_horizontal(1)
            // The background displaces the border without affecting the style
            .background()
            .with_style(Style::new())
            .padding()
            .with_horizontal(2)
            .float()
            .with_top()
            .with_left();

        let body = inner.padding().with_horizontal(1).border().background();

        Self(title.above(body).float().with_center())
    }

    pub fn with_border_style(mut self, style: Style) -> Self {
        let border = &mut self.0.inner.first.inner;
        border.style = style;
        self
    }
}

#[async_trait]
impl<E, I> AsyncWidget<E> for Popup<I>
where
    E: Send,
    I: AsyncWidget<E> + Send + Sync,
{
    async fn size(
        &self,
        widthdb: &mut WidthDb,
        max_width: Option<u16>,
        max_height: Option<u16>,
    ) -> Result<Size, E> {
        self.0.size(widthdb, max_width, max_height).await
    }

    async fn draw(self, frame: &mut Frame) -> Result<(), E> {
        self.0.draw(frame).await
    }
}
