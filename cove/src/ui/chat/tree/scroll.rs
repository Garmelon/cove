use toss::widgets::EditorState;
use toss::WidthDb;

use crate::store::{Msg, MsgStore};
use crate::ui::chat::cursor::Cursor;
use crate::ui::ChatMsg;

use super::renderer::{TreeContext, TreeRenderer};
use super::TreeViewState;

impl<M, S> TreeViewState<M, S>
where
    M: Msg + ChatMsg + Send + Sync,
    M::Id: Send + Sync,
    S: MsgStore<M> + Send + Sync,
    S::Error: Send,
{
    fn last_context(&self) -> TreeContext<M::Id> {
        TreeContext {
            size: self.last_size,
            nick: self.last_nick.clone(),
            focused: true,
            last_cursor: self.last_cursor.clone(),
            last_cursor_top: self.last_cursor_top,
        }
    }

    pub async fn scroll_by(
        &mut self,
        cursor: &mut Cursor<M::Id>,
        editor: &mut EditorState,
        widthdb: &mut WidthDb,
        delta: i32,
    ) -> Result<(), S::Error> {
        let context = self.last_context();
        let mut renderer = TreeRenderer::new(context, &self.store, cursor, editor, widthdb);
        renderer.prepare_blocks_for_drawing().await?;

        renderer.scroll_by(delta).await?;

        renderer.update_render_info(
            &mut self.last_cursor,
            &mut self.last_cursor_top,
            &mut self.last_visible_msgs,
        );
        Ok(())
    }

    pub async fn center_cursor(
        &mut self,
        cursor: &mut Cursor<M::Id>,
        editor: &mut EditorState,
        widthdb: &mut WidthDb,
    ) -> Result<(), S::Error> {
        let context = self.last_context();
        let mut renderer = TreeRenderer::new(context, &self.store, cursor, editor, widthdb);
        renderer.prepare_blocks_for_drawing().await?;

        renderer.center_cursor();

        renderer.update_render_info(
            &mut self.last_cursor,
            &mut self.last_cursor_top,
            &mut self.last_visible_msgs,
        );
        Ok(())
    }
}
