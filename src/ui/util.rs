use std::sync::Arc;

use parking_lot::FairMutex;
use toss::terminal::Terminal;

pub fn prompt(
    terminal: &mut Terminal,
    crossterm_lock: &Arc<FairMutex<()>>,
    initial_text: &str,
) -> Option<String> {
    let content = {
        let _guard = crossterm_lock.lock();
        terminal.suspend().expect("could not suspend");
        let content = edit::edit(initial_text);
        terminal.unsuspend().expect("could not unsuspend");
        content
    };

    // TODO Don't swipe this error under the rug
    let content = content.ok()?;

    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}
