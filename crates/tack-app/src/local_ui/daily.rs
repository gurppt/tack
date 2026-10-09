//! Temporary daily-use panels; allocated only while a panel is open.
use super::*;
use tack_core::{BookmarkId, CameraBookmark};
#[derive(Default)]
pub struct DailyPanel {
    pub choices: Vec<crate::sharing::Choice>,
    pub bookmarks: Vec<CameraBookmark>,
    pub id: Option<BookmarkId>,
    pub text: String,
    pub rows: Vec<String>,
    pub shared: bool,
    pub replace: bool,
}
impl LocalUi {
    pub fn daily(panel: Panel, data: DailyPanel) -> Self {
        let mut ui = Self::new(panel);
        if panel == Panel::Sharing {
            ui.selected = data
                .choices
                .iter()
                .position(|c| *c != crate::sharing::Choice::None)
                .unwrap_or(0);
        }
        ui.daily = Some(Box::new(data));
        ui
    }
    pub fn daily_data(&mut self) -> Option<&mut DailyPanel> {
        self.daily.as_deref_mut()
    }
    pub(super) fn daily_activate(&mut self) -> Option<UiResult> {
        let data = self.daily.as_ref()?;
        match self.panel {
            Panel::Sharing => data
                .choices
                .get(self.selected)
                .copied()
                .map(UiResult::Sharing),
            Panel::Server if self.selected == 1 => Some(UiResult::Server(data.text.clone())),
            Panel::Server if self.selected == 2 => Some(UiResult::Dismiss),
            Panel::Bookmarks => data
                .bookmarks
                .get(self.selected)
                .map(|b| UiResult::JumpBookmark(b.id())),
            Panel::BookmarkName | Panel::Join if self.selected == 2 => Some(UiResult::Dismiss),
            Panel::BookmarkName if self.selected == 1 => {
                Some(UiResult::SaveBookmark(data.id, data.text.clone()))
            }
            Panel::Join if self.selected == 1 => Some(UiResult::Join(data.text.clone())),
            _ => None,
        }
    }
    pub(super) fn daily_command(&mut self, rename: bool) -> Option<UiResult> {
        let data = self.daily.as_ref()?;
        if data.shared {
            return None;
        }
        let id = data.bookmarks.get(self.selected)?.id();
        Some(if rename {
            UiResult::RenameBookmark(id)
        } else {
            UiResult::DeleteBookmark(id)
        })
    }
    pub(super) fn daily_event(&mut self, event: &WindowEvent) -> Option<Option<UiResult>> {
        let data = self.daily.as_mut()?;
        let WindowEvent::KeyboardInput { event, .. } = event else {
            return None;
        };
        if event.state != ElementState::Pressed {
            return None;
        }
        let key = event.physical_key;
        if key == PhysicalKey::Code(KeyCode::Escape) {
            return Some(Some(UiResult::Dismiss));
        }
        if self.panel == Panel::Bookmarks {
            if key == PhysicalKey::Code(KeyCode::F2) {
                return Some(self.daily_command(true));
            }
            if key == PhysicalKey::Code(KeyCode::Delete) {
                return Some(self.daily_command(false));
            }
            return None;
        }
        if !matches!(
            self.panel,
            Panel::BookmarkName | Panel::Join | Panel::Server
        ) {
            return None;
        }
        if key == PhysicalKey::Code(KeyCode::Enter) {
            if self.focus == Some(Command::Close) {
                return Some(Some(UiResult::Dismiss));
            }
            self.selected = 1;
            return Some(self.daily_activate());
        }
        if self.modifiers.contains(Modifiers::CONTROL) {
            match key {
                PhysicalKey::Code(KeyCode::KeyA) => data.replace = true,
                PhysicalKey::Code(KeyCode::KeyV)
                    if matches!(self.panel, Panel::Join | Panel::Server) =>
                {
                    return Some(Some(UiResult::PasteAddress));
                }
                _ => {}
            }
            return Some(None);
        }
        if matches!(key, PhysicalKey::Code(KeyCode::Backspace | KeyCode::Delete)) {
            if data.replace {
                data.text.clear();
                data.replace = false;
            } else {
                data.text.pop();
            }
        } else if let Some(text) = &event.text {
            let limit = if matches!(self.panel, Panel::Join | Panel::Server) {
                256
            } else {
                tack_core::MAX_BOOKMARK_NAME_BYTES
            };
            let size = if data.replace { 0 } else { data.text.len() };
            if size + text.len() <= limit && !text.chars().any(char::is_control) {
                if data.replace {
                    data.text.clear();
                    data.replace = false;
                }
                data.text.push_str(text);
            }
        }
        Some(None)
    }
    pub(super) fn daily_rows(&self) -> Vec<String> {
        let Some(data) = &self.daily else {
            return Vec::new();
        };
        match self.panel {
            Panel::Sharing => data.rows.clone(),
            Panel::Bookmarks => data.bookmarks.iter().map(|b| b.name().to_owned()).collect(),
            Panel::BookmarkName | Panel::Join | Panel::Server => vec![
                data.text.clone(),
                "Confirm (Enter)".into(),
                "Cancel (Escape)".into(),
            ],
            Panel::Info => data
                .rows
                .iter()
                .flat_map(|line| {
                    let chars: Vec<_> = line.chars().collect();
                    chars
                        .chunks(32)
                        .map(|c| c.iter().collect::<String>())
                        .collect::<Vec<_>>()
                })
                .collect(),
            Panel::Connecting if !data.rows.is_empty() => data.rows.clone(),
            Panel::Connecting => vec![
                "Connecting in another window...".into(),
                "Cancel (Escape)".into(),
            ],
            _ => Vec::new(),
        }
    }
}
