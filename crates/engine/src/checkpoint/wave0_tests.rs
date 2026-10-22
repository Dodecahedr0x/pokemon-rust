use crate::script::Runner;
use crate::text::TextBox;

#[test]
fn restored_runner_is_idle_and_keeps_issued() {
    let r = Runner::restore_idle(7);
    assert!(!r.is_running());
    assert!(r.active().is_none());
    assert_eq!(r.issued(), 7);
}

#[test]
fn new_text_box_is_pristine() {
    assert!(TextBox::new().is_pristine());
}
