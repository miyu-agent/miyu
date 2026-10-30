use super::unused;

fn heads(ids: &[&str]) -> Vec<String> {
    ids.iter().map(ToString::to_string).collect()
}

#[test]
fn terminal_items_need_the_terminal_head() {
    assert!(unused("terminal.persona", &heads(&["miyu-webui"])));
    assert!(unused("terminal.idle_new_after", &heads(&[])));
    assert!(!unused("terminal.preset", &heads(&["miyu-tui"])));
}

#[test]
fn the_default_head_needs_a_choice() {
    assert!(unused("ui.head", &heads(&["miyu-webui"])));
    assert!(unused("ui.head", &heads(&[])));
    assert!(!unused("ui.head", &heads(&["miyu-tui", "miyu-webui"])));
}

#[test]
fn other_items_are_always_used() {
    assert!(!unused("ui.language", &heads(&[])));
    assert!(!unused("usage.currency", &heads(&[])));
}
