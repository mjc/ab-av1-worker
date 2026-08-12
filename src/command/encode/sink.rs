use std::fmt::Display;

/// Encode execution progress sink (shell adapts indicatif or test fakes).
pub trait ProgressSink {
    fn set_message(&self, msg: impl Display);
    fn set_position(&self, pos: u64);
    fn finish(&self);
}

impl ProgressSink for indicatif::ProgressBar {
    fn set_message(&self, msg: impl Display) {
        indicatif::ProgressBar::set_message(self, msg.to_string());
    }

    fn set_position(&self, pos: u64) {
        indicatif::ProgressBar::set_position(self, pos);
    }

    fn finish(&self) {
        indicatif::ProgressBar::finish(self);
    }
}
