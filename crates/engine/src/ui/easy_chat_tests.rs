use super::*;

#[test]
fn unchanged_phrases_quit_and_the_questionnaire_confirms() {
    assert_eq!(EasyChat::new(0).try_confirm(), Prompt::Quit { cursor: 1 });
    assert_eq!(
        EasyChat::new(ASKS_CONFIRM).try_confirm(),
        Prompt::Confirm { cursor: 0 }
    );
}
