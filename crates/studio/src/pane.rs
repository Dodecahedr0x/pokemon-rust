//! A pane: a game (native session or mGBA) owned by its own thread, driven by commands and
//! publishing a framebuffer and a status (plan: "pane.rs"). The trait and `RunMode` are the
//! Wave 0 contract; the Wave 0 spike adds `spawn`, `PaneHandle`, `StatusSlot` and `Pacer`.

/// How a pane advances on each 60 Hz tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunMode {
    Play,
    Pause,
    /// `n` frames per tick, `2..=16`.
    Fast(u8),
}

/// Lives on its pane thread, so it need not be `Send`.
pub trait Pane {
    type Cmd: Send + 'static;
    type Status: Clone + Send + 'static;
    fn handle(&mut self, cmd: Self::Cmd);
    /// One 60 Hz tick: steps 0..n frames depending on the run mode.
    fn tick(&mut self);
    /// The last stepped frame, RGB555, `SCREEN_W * SCREEN_H`.
    fn framebuffer(&mut self) -> &[u16];
    fn status(&self) -> Self::Status;
}
