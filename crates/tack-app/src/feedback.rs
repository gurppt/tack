//! One-shot pixel acknowledgement and edit-only caret deadlines; no idle tick.
use std::time::{Duration, Instant};
pub const ACK_DURATION: Duration = Duration::from_millis(1200);
#[derive(Default)]
pub struct Feedback {
    deadline: Option<Instant>,
}
impl Feedback {
    pub fn acknowledge(&mut self, now: Instant) {
        self.deadline = Some(now + ACK_DURATION);
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
    pub fn active(&self) -> bool {
        self.deadline.is_some()
    }
    pub fn settle(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|at| now >= at) {
            self.deadline = None;
            true
        } else {
            false
        }
    }
}
#[derive(Default)]
pub struct Caret {
    deadline: Option<Instant>,
    pub visible: bool,
}
impl Caret {
    pub fn update(&mut self, editing: bool, now: Instant) -> bool {
        if !editing {
            let changed = self.deadline.take().is_some();
            self.visible = false;
            return changed;
        }
        if self.deadline.is_none_or(|at| now >= at) {
            self.visible = self.deadline.is_none() || !self.visible;
            self.deadline = Some(now + Duration::from_millis(600));
            true
        } else {
            false
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
}

pub fn edit_text(text: &str, caret: bool) -> std::borrow::Cow<'_, str> {
    if caret {
        std::borrow::Cow::Owned(format!("{text}█"))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}
