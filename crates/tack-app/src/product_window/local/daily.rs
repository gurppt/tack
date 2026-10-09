//! Small daily-use flows on the existing editor, temporary panel and operation worker.
use super::*;
use tack_app::local_ui::DailyPanel;
use tack_core::{BookmarkId, CameraBookmark, Command, SourceLocation};
impl App {
    pub(in super::super) fn daily_action(&mut self, action: Action) -> Result<(), AssetError> {
        match action {
            Action::JoinSharedBoard => self.daily_panel(Panel::Join, DailyPanel::default()),
            Action::CopySharedBoardAddress => {
                let shared = self.shared.as_ref().ok_or("Join a shared board first")?;
                let address = tack_app::shared_address::SharedAddress::parse(&format!(
                    "{} {}",
                    shared.address, shared.board
                ))?;
                self.operation(Operation::CopyAddress(address.canonical()))?;
            }
            Action::DuplicateSelection => {
                if self.shared.is_some() || self.offline.is_some() {
                    return Err("Duplicate is local-only in this phase".into());
                }
                let editor = self.editor.as_mut().ok_or("document unavailable")?;
                let step = tack_app::spatial_snap::grid_spacing(
                    self.camera.zoom(),
                    self.camera.ui_scale(),
                );
                let (command, ids) = tack_app::duplicate::selection(
                    editor.document(),
                    self.input.images.selection.ids(),
                    [step; 2],
                )?;
                if !ids.is_empty() {
                    editor.execute(command)?;
                    self.input.images.selection.clear();
                    for id in ids {
                        self.input.images.selection.select(Some(id), true);
                    }
                }
            }
            Action::AddCameraBookmark => {
                if self.shared.is_some() || self.offline.is_some() {
                    return Err("Bookmark editing is local-only in this phase".into());
                }
                let doc = self
                    .editor
                    .as_ref()
                    .ok_or("document unavailable")?
                    .document();
                if doc.bookmarks().len() == tack_core::MAX_CAMERA_BOOKMARKS {
                    return Err("A board supports at most 64 bookmarks".into());
                }
                self.daily_panel(
                    Panel::BookmarkName,
                    DailyPanel {
                        text: format!("View {}", doc.bookmarks().len() + 1),
                        replace: true,
                        ..Default::default()
                    },
                );
            }
            Action::CameraBookmarks => self.bookmarks_panel()?,
            Action::SourceInfo => {
                if self.input.images.selection.len() != 1 {
                    return Err("Information requires one selected image".into());
                }
                let id = self
                    .input
                    .images
                    .selection
                    .ids()
                    .next()
                    .ok_or("selected image")?;
                let doc = self
                    .editor
                    .as_ref()
                    .ok_or("document unavailable")?
                    .document();
                let source = doc
                    .object_render_data(id)
                    .and_then(|i| doc.asset(i.asset_id))
                    .and_then(|a| doc.source(a.source_id()))
                    .ok_or("selected image source")?;
                let bytes = self
                    .local
                    .originals
                    .get(&(source.id(), source.revision()))
                    .and_then(|p| match p {
                        tack_storage::Payload::Stored { range, .. } => Some(range.len),
                        _ => None,
                    })
                    .or_else(|| {
                        matches!(source.location(), SourceLocation::Embedded)
                            .then(|| {
                                self.board
                                    .as_ref()?
                                    .originals
                                    .get(&source.id())
                                    .map(|o| o.range.len)
                            })
                            .flatten()
                    });
                let rows = tack_app::source_info::rows(
                    doc,
                    id,
                    self.assets
                        .as_ref()
                        .and_then(|a| a.states.get(&source.id()).copied()),
                    bytes,
                )?;
                self.daily_panel(
                    Panel::Info,
                    DailyPanel {
                        rows,
                        ..Default::default()
                    },
                );
            }
            _ => {}
        }
        Ok(())
    }
    pub(in super::super) fn daily_panel(&mut self, panel: Panel, data: DailyPanel) {
        self.panel(panel);
        self.local.ui = Some(Box::new(LocalUi::daily(panel, data)));
    }
    fn bookmarks_panel(&mut self) -> Result<(), AssetError> {
        let bookmarks = self
            .editor
            .as_ref()
            .ok_or("document unavailable")?
            .document()
            .bookmarks()
            .to_vec();
        self.daily_panel(
            Panel::Bookmarks,
            DailyPanel {
                bookmarks,
                shared: self.shared.is_some() || self.offline.is_some(),
                ..Default::default()
            },
        );
        Ok(())
    }
    pub(super) fn daily_result(&mut self, result: UiResult) -> Result<(), AssetError> {
        match result {
            UiResult::Sharing(choice) => self.sharing_choice(choice)?,
            UiResult::Server(text) => {
                let addr: std::net::SocketAddr = text
                    .trim()
                    .parse()
                    .map_err(|_| "Advanced server address must be numeric IP:port")?;
                tack_app::shared_address::SharedAddress::parse(&format!(
                    "{} 00000000000000000000000000000001",
                    addr
                ))?;
                self.share_remote = Some(addr.to_string());
                self.sharing_choice(tack_app::sharing::Choice::Start)?;
            }
            UiResult::JumpBookmark(id) => {
                let doc = self
                    .editor
                    .as_ref()
                    .ok_or("document unavailable")?
                    .document();
                doc.bookmarks()
                    .iter()
                    .find(|b| b.id() == id)
                    .ok_or("bookmark unavailable")?
                    .jump(&mut self.camera)?;
                self.local.ui = None;
            }
            UiResult::RenameBookmark(id) => {
                let doc = self
                    .editor
                    .as_ref()
                    .ok_or("document unavailable")?
                    .document();
                let text = doc
                    .bookmarks()
                    .iter()
                    .find(|b| b.id() == id)
                    .ok_or("bookmark unavailable")?
                    .name()
                    .to_owned();
                self.daily_panel(
                    Panel::BookmarkName,
                    DailyPanel {
                        id: Some(id),
                        text,
                        replace: true,
                        ..Default::default()
                    },
                );
            }
            UiResult::SaveBookmark(id, name) => self.save_bookmark(id, name)?,
            UiResult::DeleteBookmark(id) => {
                self.local_bookmark_edit()?;
                let editor = self.editor.as_mut().ok_or("document unavailable")?;
                let values = editor
                    .document()
                    .bookmarks()
                    .iter()
                    .filter(|b| b.id() != id)
                    .cloned()
                    .collect();
                editor.execute(Command::SetCameraBookmarks(values))?;
                self.bookmarks_panel()?;
            }
            UiResult::Join(text) => {
                let address = match tack_app::shared_address::SharedAddress::parse(&text) {
                    Ok(address) => address,
                    Err(error) => {
                        if let Some(ui) = &mut self.local.ui {
                            ui.message = error.to_string();
                        }
                        return Ok(());
                    }
                };
                self.operation(Operation::Join {
                    address,
                    work: self.work.join("helpers"),
                })?;
                self.daily_panel(Panel::Connecting, DailyPanel::default());
            }
            UiResult::PasteAddress => {
                self.operation(Operation::PasteAddress(self.work.join("helpers")))?
            }
            _ => {}
        }
        self.dirty = true;
        Ok(())
    }
    fn local_bookmark_edit(&self) -> Result<(), AssetError> {
        if self.shared.is_some() || self.offline.is_some() {
            Err("Bookmark editing is local-only in this phase".into())
        } else {
            Ok(())
        }
    }
    fn save_bookmark(&mut self, id: Option<BookmarkId>, name: String) -> Result<(), AssetError> {
        self.local_bookmark_edit()?;
        let editor = self.editor.as_mut().ok_or("document unavailable")?;
        let mut values = editor.document().bookmarks().to_vec();
        let bookmark = if let Some(id) = id {
            let old = values
                .iter()
                .find(|b| b.id() == id)
                .ok_or("bookmark unavailable")?;
            CameraBookmark::new(id, name, old.center(), old.zoom())
        } else {
            let screen = self.camera.screen_size().map(|v| f64::from(v) / 2.);
            CameraBookmark::new(
                tack_storage::new_bookmark_id()?,
                name,
                self.camera.screen_to_world(screen),
                self.camera.zoom(),
            )
        };
        let bookmark = match bookmark {
            Ok(value) => value,
            Err(_) => {
                if let Some(ui) = &mut self.local.ui {
                    ui.message = "Use a nonempty name, at most 128 UTF-8 bytes".into();
                }
                return Ok(());
            }
        };
        if let Some(index) = values.iter().position(|b| b.id() == bookmark.id()) {
            values[index] = bookmark;
        } else {
            values.push(bookmark);
        }
        editor.execute(Command::SetCameraBookmarks(values))?;
        self.bookmarks_panel()
    }
}
