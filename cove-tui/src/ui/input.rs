use crossterm::event::KeyEvent;

pub trait EventHandler {
    type Reaction;

    fn handle_key(&mut self, event: KeyEvent) -> Option<Self::Reaction>;

    // TODO Add method to show currently accepted keys for F1 help
}
