//! Battle windows and menus (`BattleWindows`). **Task F.**
//!
//! Message box (B_WIN_MSG), action prompt and menu (`HandleInputChooseAction`), move menu with
//! PP colors (`HandleInputChooseMove`), yes/no on BG1, learn-move view. Every redraw rewrites
//! rows 15..20 of the TEXT BG (`video::layout::battle::TEXT`).
