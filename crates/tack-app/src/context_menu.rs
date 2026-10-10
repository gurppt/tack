//! On-demand bitmap command lists. No timer, thread, widget tree or texture cache.
use crate::{
    actions::{Action, Tool},
    annotation_tool::StyleAction,
    bindings::{Keymap, ModifierMatch, Trigger},
    image_gizmo::ImageGizmo,
    image_interaction::SelectionState,
    input::{Modifiers, PhysicalControl},
    selection_commands::{Alpha, Order as ZOrder},
    spatial_layout::Layout as Arrangement,
};
use tack_core::{
    AnnotationKind, Camera, DocumentEditor, DocumentQuery, ImageFiltering, ObjectKind,
    SourceLocation,
};
use winit::{
    event::{ElementState, MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};
const ROW: i32 = 18;
const PAD: i32 = 2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextKind {
    Canvas,
    Image,
    Multiple,
    Note,
    Frame,
    Annotation,
    Application,
}
#[derive(Clone, Copy)]
pub struct Context {
    pub kind: ContextKind,
    selection_kind: ContextKind,
    shared: bool,
    hosted: bool,
    pub generation: u64,
    undo: bool,
    redo: bool,
    any: bool,
    all_images: bool,
    group: bool,
    ungroup: bool,
    align: bool,
    linked: bool,
    forward: bool,
    backward: bool,
    filtering: Option<ImageFiltering>,
    opacity: Option<f64>,
    grid: bool,
    snapping: bool,
    has_objects: bool,
    has_frames: bool,
    annotation_fill: bool,
}
impl Context {
    pub fn selection(
        editor: &DocumentEditor,
        selection: &SelectionState,
        grid: bool,
        snapping: bool,
    ) -> Self {
        let doc = editor.document();
        let ids = || selection.ids().filter(|id| doc.object(*id).is_some());
        let count = ids().count();
        let first_id = ids().next();
        let all_images = count > 0 && ids().all(|id| doc.object_render_data(id).is_some());
        let kind = if count > 1 {
            ContextKind::Multiple
        } else {
            match first_id.and_then(|id| doc.object(id)).map(|o| o.kind()) {
                Some(ObjectKind::Image(_)) => ContextKind::Image,
                Some(ObjectKind::Frame(_)) => ContextKind::Frame,
                Some(ObjectKind::Annotation(a)) if matches!(a.kind(), AnnotationKind::Text(_)) => {
                    ContextKind::Note
                }
                Some(ObjectKind::Annotation(_)) => ContextKind::Annotation,
                None => ContextKind::Canvas,
            }
        };
        let unit = |id| doc.group_for(id).map_or(id, |g| g.members()[0]);
        let first_unit = first_id.map(unit);
        let multiple_units = ids().any(|id| Some(unit(id)) != first_unit);
        let ungroup = ids().any(|id| doc.group_for(id).is_some());
        let linked = first_id
            .and_then(|id| doc.object_render_data(id))
            .and_then(|d| doc.asset(d.asset_id))
            .and_then(|a| doc.source(a.source_id()))
            .is_some_and(
                |s| matches!(s.location(), SourceLocation::Linked(p) if p.to_native().is_some()),
            );
        // Checks are only shown for a single image; no selection-sized scratch tree.
        let first = first_id
            .filter(|_| count == 1)
            .and_then(|id| doc.object_render_data(id));
        let filtering = first.map(|a| a.filtering);
        let opacity = first.map(|a| a.opacity.value());
        let order = doc.object_order();
        let forward = order
            .windows(2)
            .any(|pair| selection.contains(pair[0]) && !selection.contains(pair[1]));
        let backward = order
            .windows(2)
            .any(|pair| !selection.contains(pair[0]) && selection.contains(pair[1]));
        Self {
            kind,
            selection_kind: kind,
            shared: false,
            hosted: false,
            generation: editor.generation(),
            undo: editor.undo_len() > 0,
            redo: editor.redo_len() > 0,
            any: count > 0,
            all_images,
            group: all_images && multiple_units,
            ungroup,
            align: multiple_units,
            linked,
            forward,
            backward,
            filtering,
            opacity,
            grid,
            snapping,
            has_objects: !order.is_empty(),
            has_frames: doc.frame_count() > 0,
            annotation_fill: first_id.and_then(|id| doc.object(id)).is_some_and(|o| matches!(o.kind(), ObjectKind::Annotation(a) if matches!(a.kind(), AnnotationKind::Rect))),
        }
    }
    pub fn with_shared(mut self, shared: bool) -> Self {
        self.shared = shared;
        self
    }
    pub fn with_hosted(mut self, hosted: bool) -> Self {
        self.hosted = hosted;
        self
    }
    pub fn enabled(self, action: Action) -> bool {
        match action {
            Action::DuplicateSelection => self.any,
            Action::AddCameraBookmark => true,
            Action::CopySharedBoardAddress => self.shared,
            Action::StopSharing => self.hosted,
            Action::SourceInfo => self.selection_kind == ContextKind::Image,
            Action::Undo => self.undo,
            Action::Redo => self.redo,
            Action::SelectAll => self.has_objects,
            Action::NextFrame | Action::PreviousFrame => self.has_frames,
            Action::AnnotationStyle(StyleAction::Fill) => self.annotation_fill,
            Action::DeleteSelection | Action::Order(_) => {
                self.any
                    && match action {
                        Action::Order(ZOrder::Forward | ZOrder::Front) => self.forward,
                        Action::Order(ZOrder::Backward | ZOrder::Back) => self.backward,
                        _ => true,
                    }
            }
            Action::ResetAspectRatio => self.all_images,
            Action::CropMode => self.selection_kind == ContextKind::Image,
            Action::FlipHorizontal
            | Action::FlipVertical
            | Action::Filtering(_)
            | Action::Opacity(_) => self.all_images,
            Action::GroupSelection => self.group,
            Action::UngroupSelection => self.ungroup,
            Action::Layout(Arrangement::SnapToGrid) => self.any,
            Action::Layout(_) => self.align,
            Action::RenameFrame => {
                matches!(self.selection_kind, ContextKind::Note | ContextKind::Frame)
            }
            Action::FocusFrame => self.selection_kind == ContextKind::Frame,
            Action::RelinkSource | Action::SaveOriginalAs => {
                self.selection_kind == ContextKind::Image
            }
            Action::OpenSource | Action::RevealSource | Action::CopySourcePath => {
                self.selection_kind == ContextKind::Image && self.linked
            }
            _ => true,
        }
    }
    fn checked(self, action: Action) -> bool {
        match action {
            Action::Filtering(f) => self.filtering == Some(f),
            Action::Opacity(alpha) => self.opacity == Some(alpha.value()),
            Action::ToggleGrid => self.grid,
            Action::ToggleSnapping => self.snapping,
            _ => false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Sampling,
    Opacity,
    Order,
    Arrange,
    Align,
    Style,
    Source,
    File,
    Edit,
    View,
    Tools,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Action(Action),
    Submenu(Group),
    Heading,
}
#[derive(Clone, Debug)]
pub struct Item {
    pub label: &'static str,
    pub command: Command,
    pub enabled: bool,
    pub checked: bool,
    pub shortcut: String,
}
/// Only real configured press bindings are advertised; unassigned actions stay blank.
pub fn binding_label(b: &crate::bindings::Binding) -> String {
    let (mods, qualifier) = match b.modifiers {
        ModifierMatch::Exact(m) => (m, ""),
        ModifierMatch::Contains(m) => (m, "+other"),
        ModifierMatch::Any => (Modifiers::NONE, "+any"),
    };
    let mut label = String::new();
    for (modifier, text) in [
        (Modifiers::CONTROL, "Ctrl+"),
        (Modifiers::SHIFT, "Shift+"),
        (Modifiers::ALT, "Alt+"),
        (Modifiers::SUPER, "Super+"),
    ] {
        if mods.contains(modifier) {
            label.push_str(text);
        }
    }
    match b.control {
        PhysicalControl::LogicalKey(crate::input::LogicalKey::Character(c)) => {
            label.push(c.to_ascii_uppercase())
        }
        PhysicalControl::LogicalKey(crate::input::LogicalKey::Named(n)) => {
            let name = match n {
                winit::keyboard::NamedKey::Delete => "Del",
                winit::keyboard::NamedKey::Escape => "Esc",
                winit::keyboard::NamedKey::PageUp => "PgUp",
                winit::keyboard::NamedKey::PageDown => "PgDn",
                winit::keyboard::NamedKey::ArrowLeft => "Left",
                winit::keyboard::NamedKey::ArrowRight => "Right",
                winit::keyboard::NamedKey::ArrowUp => "Up",
                winit::keyboard::NamedKey::ArrowDown => "Down",
                _ => {
                    label.push_str(&format!("{n:?}"));
                    ""
                }
            };
            label.push_str(name);
        }
        PhysicalControl::Key(key) => label.push_str(&format!("pos:{key:?}")),
        PhysicalControl::Pointer(crate::input::PointerButton::Mouse(button)) => {
            let text = match button {
                winit::event::MouseButton::Left => "Mouse1",
                winit::event::MouseButton::Middle => "Mouse2",
                winit::event::MouseButton::Right => "Mouse3",
                winit::event::MouseButton::Back => "Mouse4",
                winit::event::MouseButton::Forward => "Mouse5",
                winit::event::MouseButton::Other(n) => {
                    label.push_str(&format!("Mouse({n})"));
                    ""
                }
            };
            label.push_str(text);
        }
        PhysicalControl::Pointer(crate::input::PointerButton::Stylus(n)) => {
            label.push_str(&format!("Stylus({n})"))
        }
        PhysicalControl::Wheel(axis) => label.push_str(match axis {
            crate::input::WheelAxis::Horizontal => "WheelX",
            crate::input::WheelAxis::Vertical => "WheelY",
        }),
    }
    label.push_str(qualifier);
    label
}
pub fn shortcut(keymap: &Keymap, action: Action) -> String {
    keymap
        .for_action(action)
        .find(|b| {
            matches!(
                b.control,
                PhysicalControl::Key(_) | PhysicalControl::LogicalKey(_)
            ) && matches!(b.modifiers, ModifierMatch::Exact(_))
                && b.trigger == Trigger::Press
        })
        .map(binding_label)
        .unwrap_or_default()
}

fn action(label: &'static str, a: Action, context: Context, keymap: &Keymap) -> Item {
    Item {
        label,
        command: Command::Action(a),
        enabled: context.enabled(a),
        checked: context.checked(a),
        shortcut: shortcut(keymap, a),
    }
}
fn group(label: &'static str, g: Group, enabled: bool) -> Item {
    Item {
        label,
        command: Command::Submenu(g),
        enabled,
        checked: false,
        shortcut: String::new(),
    }
}
/// One shared application catalog, prefixed to every contextual root.
pub fn items(context: Context, group: Option<Group>, keymap: &Keymap) -> Vec<Item> {
    if group.is_some() || context.kind == ContextKind::Application {
        return context_items(context, group, keymap);
    }
    let mut root = context_items(
        Context {
            kind: ContextKind::Application,
            ..context
        },
        None,
        keymap,
    );
    let label = match context.kind {
        ContextKind::Canvas => "Canvas",
        ContextKind::Image => "Image",
        ContextKind::Multiple => "Selection",
        ContextKind::Note => "Note",
        ContextKind::Frame => "Frame",
        ContextKind::Annotation => "Annotation",
        ContextKind::Application => "Tack",
    };
    root.push(heading(label));
    root.extend(context_items(context, None, keymap));
    root
}
fn heading(label: &'static str) -> Item {
    Item {
        label,
        command: Command::Heading,
        enabled: false,
        checked: false,
        shortcut: String::new(),
    }
}
fn context_items(context: Context, submenu: Option<Group>, keymap: &Keymap) -> Vec<Item> {
    use Action::*;
    let a = |label, command| action(label, command, context, keymap);
    if let Some(g) = submenu {
        return match g {
            Group::Sampling => [
                ("Nearest", ImageFiltering::Nearest),
                ("Smooth", ImageFiltering::Smooth),
                ("Default", ImageFiltering::Default),
            ]
            .into_iter()
            .map(|(l, f)| a(l, Filtering(f)))
            .collect(),
            Group::Opacity => [
                ("100%", Alpha::Full),
                ("75%", Alpha::ThreeQuarters),
                ("50%", Alpha::Half),
                ("25%", Alpha::Quarter),
            ]
            .into_iter()
            .map(|(l, v)| a(l, Opacity(v)))
            .collect(),
            Group::Order => [
                ("Bring forward", ZOrder::Forward),
                ("Bring to front", ZOrder::Front),
                ("Send backward", ZOrder::Backward),
                ("Send to back", ZOrder::Back),
            ]
            .into_iter()
            .map(|(l, v)| a(l, Action::Order(v)))
            .collect(),
            Group::Arrange => vec![
                a("Arrange in Grid", Action::Layout(Arrangement::Grid)),
                a("Snap to Grid", Action::Layout(Arrangement::SnapToGrid)),
                a("Align left", Action::Layout(Arrangement::Left)),
                a(
                    "Align center",
                    Action::Layout(Arrangement::HorizontalCenter),
                ),
                a("Align right", Action::Layout(Arrangement::Right)),
                a("Align top", Action::Layout(Arrangement::Top)),
                a("Align middle", Action::Layout(Arrangement::VerticalCenter)),
                a("Align bottom", Action::Layout(Arrangement::Bottom)),
                a(
                    "Distribute horizontal",
                    Action::Layout(Arrangement::DistributeHorizontal),
                ),
                a(
                    "Distribute vertical",
                    Action::Layout(Arrangement::DistributeVertical),
                ),
                a(
                    "Pack horizontal",
                    Action::Layout(Arrangement::PackHorizontal),
                ),
                a("Pack vertical", Action::Layout(Arrangement::PackVertical)),
            ],
            Group::Align => [
                ("Left", Arrangement::Left),
                ("Center", Arrangement::HorizontalCenter),
                ("Right", Arrangement::Right),
                ("Top", Arrangement::Top),
                ("Middle", Arrangement::VerticalCenter),
                ("Bottom", Arrangement::Bottom),
                ("Distribute horizontal", Arrangement::DistributeHorizontal),
                ("Distribute vertical", Arrangement::DistributeVertical),
                ("Pack horizontal", Arrangement::PackHorizontal),
                ("Pack vertical", Arrangement::PackVertical),
            ]
            .into_iter()
            .map(|(l, v)| a(l, Action::Layout(v)))
            .collect(),
            Group::Style => {
                let mut v = vec![a("Color", AnnotationStyle(StyleAction::Color))];
                if context.kind == ContextKind::Note {
                    v.extend([
                        a("Larger text", AnnotationStyle(StyleAction::LargerText)),
                        a("Smaller text", AnnotationStyle(StyleAction::SmallerText)),
                        a("Text alignment", AnnotationStyle(StyleAction::AlignText)),
                    ]);
                } else {
                    v.extend([
                        a("Fill", AnnotationStyle(StyleAction::Fill)),
                        a("Thicker", AnnotationStyle(StyleAction::Wider)),
                        a("Thinner", AnnotationStyle(StyleAction::Narrower)),
                    ]);
                }
                v.extend([
                    a("More opaque", AnnotationStyle(StyleAction::OpacityUp)),
                    a("Less opaque", AnnotationStyle(StyleAction::OpacityDown)),
                ]);
                v
            }
            Group::Source => vec![
                a("Image information...", SourceInfo),
                a("Open original", OpenSource),
                a("Reveal original", RevealSource),
                a("Relink...", RelinkSource),
                a("Save Original As...", SaveOriginalAs),
                a("Copy source path", CopySourcePath),
            ],
            Group::File => vec![
                a("New board", NewBoard),
                a("Close board", CloseBoard),
                a("Open...", OpenBoard),
                a("Import images...", ImportImages),
                a("Save", Save),
                a("Save As...", SaveAs),
                a("Save to Local...", SaveToLocal),
                a("Recent boards", RecentBoards),
                a("Share Board...", ShareBoard),
                a("Stop Sharing", StopSharing),
                a("Join shared board...", JoinSharedBoard),
                a("Copy Invite", CopySharedBoardAddress),
            ],
            Group::Edit => vec![
                a("Undo", Undo),
                a("Redo", Redo),
                a("Paste", Paste),
                a("Duplicate selection", DuplicateSelection),
                a("Select all", SelectAll),
                a("Delete", DeleteSelection),
            ],
            Group::View => vec![
                a("Show / hide toolbar", ToggleToolbar),
                a("Show / hide status bar", ToggleStatusBar),
                a("Grid", ToggleGrid),
                a("Snapping", ToggleSnapping),
                a("Focus frame", FocusFrame),
                a("Next frame", NextFrame),
                a("Previous frame", PreviousFrame),
                a("Add bookmark...", AddCameraBookmark),
                a("Camera bookmarks...", CameraBookmarks),
            ],
            Group::Tools => vec![
                a("Edit Toolbar...", EditToolbar),
                a("Pointer", SelectTool(Tool::Pointer)),
                a("Note (click/drag)", SelectTool(Tool::Text)),
                a("Rectangle", SelectTool(Tool::Rectangle)),
                a("Line", SelectTool(Tool::Line)),
                a("Arrow", SelectTool(Tool::Arrow)),
                a("Freehand", SelectTool(Tool::Scribble)),
                a("New frame", CreateFrame),
            ],
        };
    }
    if context.kind == ContextKind::Application {
        return vec![
            heading("Tack"),
            group("File", Group::File, true),
            group("Edit", Group::Edit, true),
            group("View", Group::View, true),
            group("Tools", Group::Tools, true),
            a("Preferences...", Preferences),
            a("Keymap...", KeymapEditor),
            a("About Tack", About),
        ];
    }
    if context.kind == ContextKind::Canvas {
        return vec![
            a("Paste", Paste),
            a("Import images...", ImportImages),
            a("New note (click/drag)", SelectTool(Tool::Text)),
            a("New frame", CreateFrame),
            a("Select all", SelectAll),
        ];
    }
    let mut v = Vec::new();
    match context.kind {
        ContextKind::Image => v.extend([
            a("Reset aspect ratio", ResetAspectRatio),
            a("Crop", CropMode),
            a("Flip horizontal", FlipHorizontal),
            a("Flip vertical", FlipVertical),
            group("Sampling", Group::Sampling, true),
            group("Opacity", Group::Opacity, true),
        ]),
        ContextKind::Note => v.extend([
            a("Edit note", RenameFrame),
            group("Text style", Group::Style, true),
        ]),
        ContextKind::Frame => v.extend([a("Rename", RenameFrame), a("Focus frame", FocusFrame)]),
        ContextKind::Annotation => v.push(group("Style", Group::Style, true)),
        ContextKind::Multiple => v.push(group("Arrange", Group::Arrange, context.any)),
        _ => {}
    }
    v.push(group(
        "Order",
        Group::Order,
        context.forward || context.backward,
    ));
    if context.all_images || context.kind == ContextKind::Multiple {
        v.extend([a("Group", GroupSelection), a("Ungroup", UngroupSelection)]);
    }
    if context.kind == ContextKind::Image {
        v.extend([
            a("Relink...", RelinkSource),
            group("Source", Group::Source, true),
        ]);
    }
    v.extend([
        a("Delete", DeleteSelection),
        a("Undo", Undo),
        a("Redo", Redo),
    ]);
    v
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl Rect {
    fn contains(self, p: [i32; 2]) -> bool {
        p[0] >= self.x
            && p[0] < self.x + self.width
            && p[1] >= self.y
            && p[1] < self.y + self.height
    }
}
struct List {
    items: Vec<Item>,
    selected: Option<usize>,
    first: usize,
    rect: Rect,
    scroll: crate::ui_scroll::Scrollbar,
    reveal_row: bool,
}
impl List {
    fn new(items: Vec<Item>) -> Self {
        Self {
            items,
            selected: None,
            first: 0,
            rect: Rect::default(),
            scroll: Default::default(),
            reveal_row: false,
        }
    }
    fn visible(&self) -> usize {
        ((self.rect.height - 2 * PAD) / ROW).max(1) as usize
    }
    fn layout(&mut self, anchor: [i32; 2], screen: [i32; 2]) {
        let width = self
            .items
            .iter()
            .map(|i| ((i.label.len() + i.shortcut.len()) * 8 + 36) as i32)
            .max()
            .unwrap_or(100)
            .clamp(100, 320)
            .min(((screen[0] - 8) / 2).max(24));
        let count = self
            .items
            .len()
            .min(((screen[1] - 8 - 2 * PAD) / ROW).max(1) as usize);
        let height = count as i32 * ROW + 2 * PAD;
        self.rect = Rect {
            x: anchor[0].clamp(2, (screen[0] - width - 2).max(2)),
            y: anchor[1].clamp(2, (screen[1] - height - 2).max(2)),
            width,
            height,
        };
        if self.reveal_row
            && let Some(selected) = self.selected
        {
            self.first = crate::ui_scroll::reveal(self.first, selected, count, self.items.len());
        }
        self.reveal_row = false;
        self.first = self.first.min(self.items.len().saturating_sub(count));
    }
    fn hit(&self, p: [i32; 2]) -> Option<usize> {
        if !self.rect.contains(p) || p[1] < self.rect.y + PAD {
            return None;
        }
        let row = ((p[1] - self.rect.y - PAD) / ROW) as usize;
        (row < self.visible() && self.first + row < self.items.len()).then_some(self.first + row)
    }
    fn step(&mut self, delta: i32) {
        self.reveal_row = true;
        if self.items.is_empty() {
            return;
        }
        self.selected = Some(match self.selected {
            None => {
                if delta < 0 {
                    self.items.len() - 1
                } else {
                    0
                }
            }
            Some(i) => (i as i32 + delta).rem_euclid(self.items.len() as i32) as usize,
        });
        if self
            .selected
            .is_some_and(|i| self.items[i].command == Command::Heading)
        {
            self.step(delta);
        }
    }
    fn draw(
        &mut self,
        gizmo: &mut ImageGizmo,
        camera: &Camera,
        budget: &mut usize,
        palette: crate::ui_theme::Palette,
    ) {
        let s = camera.ui_scale();
        let r = self.rect;
        let point = |x: i32, y: i32| [x as f64 * s, y as f64 * s];
        gizmo.pixel_rect(
            camera,
            point(r.x, r.y),
            point(r.x + r.width, r.y + r.height),
            palette.menu_border,
            None,
        );
        gizmo.pixel_rect(
            camera,
            point(r.x + 1, r.y + 1),
            point(r.x + r.width - 1, r.y + r.height - 1),
            palette.menu_bg,
            None,
        );
        for (i, item) in self
            .items
            .iter()
            .enumerate()
            .skip(self.first)
            .take(self.visible())
        {
            let y = r.y + PAD + (i - self.first) as i32 * ROW;
            if self.selected == Some(i) {
                gizmo.pixel_rect(
                    camera,
                    point(r.x + 2, y),
                    point(r.x + r.width - 2, y + ROW),
                    palette.selection,
                    None,
                );
            }
            if item.command == Command::Heading {
                if i > 0 {
                    gizmo.pixel_rect(
                        camera,
                        point(r.x + 4, y),
                        point(r.x + r.width - 4, y + 1),
                        palette.menu_border,
                        None,
                    );
                }
                gizmo.ui_text(
                    camera,
                    point(r.x + 8, y + 2),
                    f64::from(r.width - 20),
                    item.label,
                    palette.accent_primary,
                    budget,
                );
                continue;
            }
            let color = if item.enabled
                && matches!(
                    item.command,
                    Command::Action(
                        Action::SelectTool(_)
                            | Action::SourceInfo
                            | Action::CopySharedBoardAddress
                            | Action::CameraBookmarks
                    )
                ) {
                palette.accent_secondary
            } else if item.enabled
                && matches!(
                    item.command,
                    Command::Action(Action::DeleteSelection | Action::StopSharing)
                )
            {
                [0.94, 0.26, 0.29, 1.]
            } else if item.enabled {
                palette.text_primary
            } else {
                palette.text_disabled
            };
            let suffix = if matches!(item.command, Command::Submenu(_)) {
                ">"
            } else {
                &item.shortcut
            };
            let right_padding = if self.items.len() > self.visible() {
                16
            } else {
                8
            };
            let shortcut_width = (suffix.len() * 8) as i32;
            let show_shortcut =
                shortcut_width > 0 && r.width - 28 - shortcut_width >= item.label.len() as i32 * 8;
            let label_width = if show_shortcut {
                r.width - 32 - shortcut_width
            } else {
                r.width - 20
            };
            if item.checked {
                gizmo.ui_text(camera, point(r.x + 3, y + 1), 8., "*", color, budget);
            }
            gizmo.ui_text(
                camera,
                point(r.x + 14, y + 1),
                f64::from(label_width),
                item.label,
                color,
                budget,
            );
            if show_shortcut {
                gizmo.ui_text(
                    camera,
                    point(r.x + r.width - right_padding - shortcut_width, y + 1),
                    f64::from(shortcut_width),
                    suffix,
                    if item.enabled {
                        palette.accent_attention
                    } else {
                        palette.text_disabled
                    },
                    budget,
                );
            }
        }
        let visible = self.visible();
        self.scroll.draw(
            gizmo,
            camera,
            palette,
            [
                f64::from(r.x + PAD),
                f64::from(r.y + PAD),
                f64::from(r.x + r.width - PAD),
                f64::from(r.y + r.height - PAD),
            ],
            (self.first, visible, self.items.len()),
        );
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Result {
    None,
    Dismiss,
    Action(Action),
}
pub struct ContextMenu {
    pub context: Context,
    anchor: [f64; 2],
    cursor: [f64; 2],
    screen: [u32; 2],
    scale: f64,
    root: List,
    child: Option<List>,
    child_group: Option<Group>,
    child_focus: bool,
    modifiers: Modifiers,
    pub theme: crate::ui_theme::Theme,
}
impl ContextMenu {
    pub fn new(context: Context, anchor: [f64; 2], camera: &Camera, keymap: &Keymap) -> Self {
        let mut m = Self {
            context,
            anchor,
            cursor: anchor,
            screen: camera.screen_size(),
            scale: camera.ui_scale(),
            root: List::new(items(context, None, keymap)),
            child: None,
            child_group: None,
            child_focus: false,
            modifiers: Modifiers::NONE,
            theme: crate::ui_theme::Theme::default(),
        };
        m.relayout(camera);
        m
    }
    pub fn application(mut context: Context, camera: &Camera, keymap: &Keymap) -> Self {
        context.kind = ContextKind::Application;
        Self::new(
            context,
            [4. * camera.ui_scale(), 24. * camera.ui_scale()],
            camera,
            keymap,
        )
    }
    pub fn root_items(&self) -> &[Item] {
        &self.root.items
    }
    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }
    /// Developer accounting of owned menu/list/string allocations, not RSS.
    pub fn allocated_bytes(&self) -> usize {
        let list = |l: &List| {
            l.items.capacity() * std::mem::size_of::<Item>()
                + l.items.iter().map(|i| i.shortcut.capacity()).sum::<usize>()
        };
        std::mem::size_of::<Self>() + list(&self.root) + self.child.as_ref().map_or(0, list)
    }
    pub fn rectangles(&self) -> Vec<Rect> {
        std::iter::once(self.root.rect)
            .chain(self.child.as_ref().map(|c| c.rect))
            .collect()
    }
    fn logical_screen(&self) -> [i32; 2] {
        self.screen.map(|v| (v as f64 / self.scale).floor() as i32)
    }
    fn logical(&self, p: [f64; 2]) -> [i32; 2] {
        p.map(|v| (v / self.scale).floor() as i32)
    }
    pub fn relayout(&mut self, camera: &Camera) {
        self.screen = camera.screen_size();
        self.scale = camera.ui_scale();
        let screen = self.logical_screen();
        self.root.layout(self.logical(self.anchor), screen);
        if let Some(child) = &mut self.child {
            let r = self.root.rect;
            let y = r.y
                + PAD
                + (self
                    .root
                    .selected
                    .unwrap_or(self.root.first)
                    .saturating_sub(self.root.first)) as i32
                    * ROW;
            child.layout([r.x + r.width, y], screen);
            if r.x + r.width + child.rect.width + 2 > screen[0] {
                if r.x >= child.rect.width + 2 {
                    child.layout([r.x - child.rect.width, y], screen);
                } else {
                    // Neither side fits at this anchor: move the bounded pair
                    // together, preserving a fully accessible parent column.
                    self.root.rect.x = 2;
                    self.anchor[0] = 2. * self.scale;
                    child.layout([2 + r.width, y], screen);
                }
            }
        }
    }
    fn open_child(&mut self, g: Group, keymap: &Keymap, camera: &Camera) {
        if self.child_group != Some(g) {
            self.child = Some(List::new(items(self.context, Some(g), keymap)));
            self.child_group = Some(g);
        }
        self.relayout(camera);
    }
    pub fn hovered_action(&self) -> Option<Action> {
        self.child
            .as_ref()
            .or(Some(&self.root))
            .and_then(|l| l.selected.and_then(|i| l.items.get(i)))
            .and_then(|item| {
                if let Command::Action(a) = item.command {
                    Some(a)
                } else {
                    None
                }
            })
    }
    fn close_child(&mut self) {
        self.child = None;
        self.child_group = None;
        self.child_focus = false;
    }
    fn activate(&mut self, child: bool, keymap: &Keymap, camera: &Camera) -> Result {
        let list = if child {
            self.child.as_ref()
        } else {
            Some(&self.root)
        };
        let Some(item) = list
            .and_then(|l| l.selected.and_then(|i| l.items.get(i)))
            .cloned()
        else {
            return Result::None;
        };
        if !item.enabled {
            return Result::None;
        }
        match item.command {
            Command::Heading => Result::None,
            Command::Action(action) => Result::Action(action),
            Command::Submenu(g) => {
                self.open_child(g, keymap, camera);
                self.child_focus = true;
                if let Some(c) = &mut self.child {
                    c.step(1);
                }
                self.relayout(camera);
                Result::None
            }
        }
    }
    pub fn move_pointer(&mut self, p: [f64; 2], keymap: &Keymap, camera: &Camera) {
        self.cursor = p;
        let point = self.logical(p);
        let logical = point.map(f64::from);
        if let Some(c) = &mut self.child
            && let Some(first) = c.scroll.motion(logical)
        {
            c.first = first;
            return;
        }
        if let Some(first) = self.root.scroll.motion(logical) {
            self.root.first = first;
            return;
        }
        if let Some(c) = &mut self.child
            && let Some(row) = c.hit(point)
        {
            c.selected = Some(row);
            self.child_focus = true;
            return;
        }
        if let Some(row) = self.root.hit(point) {
            self.root.selected = Some(row);
            self.child_focus = false;
            if let Command::Submenu(g) = self.root.items[row].command
                && self.root.items[row].enabled
            {
                self.open_child(g, keymap, camera);
            } else {
                self.close_child();
            }
        }
    }
    pub fn click(&mut self, keymap: &Keymap, camera: &Camera) -> Result {
        let p = self.logical(self.cursor);
        if let Some(c) = &mut self.child
            && c.scroll.press(p.map(f64::from))
        {
            if let Some(first) = c.scroll.motion(p.map(f64::from)) {
                c.first = first;
            }
            return Result::None;
        }
        if self.root.scroll.press(p.map(f64::from)) {
            if let Some(first) = self.root.scroll.motion(p.map(f64::from)) {
                self.root.first = first;
            }
            return Result::None;
        }
        if let Some(c) = &mut self.child
            && let Some(row) = c.hit(p)
        {
            c.selected = Some(row);
            return self.activate(true, keymap, camera);
        }
        if let Some(row) = self.root.hit(p) {
            self.root.selected = Some(row);
            return self.activate(false, keymap, camera);
        }
        Result::Dismiss
    }
    pub fn key(&mut self, key: KeyCode, keymap: &Keymap, camera: &Camera) -> Result {
        let control = crate::input::LogicalKey::from_legacy(key)
            .map(PhysicalControl::LogicalKey)
            .unwrap_or(PhysicalControl::Key(PhysicalKey::Code(key)));
        self.key_control(key, control, keymap, camera)
    }
    fn key_control(
        &mut self,
        key: KeyCode,
        control: PhysicalControl,
        keymap: &Keymap,
        camera: &Camera,
    ) -> Result {
        match key {
            KeyCode::Escape if self.child.is_some() => self.close_child(),
            KeyCode::Escape => return Result::Dismiss,
            KeyCode::ArrowLeft if self.child.is_some() => self.close_child(),
            KeyCode::ArrowDown | KeyCode::ArrowUp => {
                let list = if self.child_focus {
                    self.child.as_mut().unwrap_or(&mut self.root)
                } else {
                    &mut self.root
                };
                list.step(if key == KeyCode::ArrowUp { -1 } else { 1 });
                if !self.child_focus {
                    self.close_child();
                }
            }
            KeyCode::Enter => return self.activate(self.child_focus, keymap, camera),
            KeyCode::ArrowRight => {
                if !self.child_focus
                    && self
                        .root
                        .selected
                        .and_then(|i| self.root.items.get(i))
                        .is_some_and(|i| matches!(i.command, Command::Submenu(_)))
                {
                    return self.activate(false, keymap, camera);
                }
            }
            _ => {
                if let Some(b) = keymap
                    .matching(control, self.modifiers, Trigger::Press)
                    .next()
                    && self.context.enabled(b.action)
                {
                    return Result::Action(b.action);
                }
            }
        }
        self.relayout(camera);
        Result::None
    }
    pub fn handle(&mut self, event: &WindowEvent, keymap: &Keymap, camera: &Camera) -> Result {
        match event {
            WindowEvent::Focused(false) | WindowEvent::Occluded(true) => Result::Dismiss,
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state().into();
                Result::None
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.move_pointer([position.x, position.y], keymap, camera);
                Result::None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.click(keymap, camera),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => Result::Dismiss,
            WindowEvent::MouseInput {
                state: ElementState::Released,
                ..
            } => {
                self.root.scroll.release();
                if let Some(c) = &mut self.child {
                    c.scroll.release();
                }
                Result::None
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let logical = self.logical(self.cursor).map(f64::from);
                let dy = crate::input::wheel_steps(*delta)[1];
                if let Some(c) = &mut self.child
                    && let Some(first) = c.scroll.wheel(logical, dy)
                {
                    c.first = first;
                } else if let Some(first) = self.root.scroll.wheel(logical, dy) {
                    self.root.first = first;
                    self.close_child();
                }
                Result::None
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                if let PhysicalKey::Code(key) = event.physical_key {
                    self.key_control(
                        key,
                        keymap.keyboard_control(
                            event.physical_key,
                            crate::input::logical_key(event),
                            self.modifiers,
                        ),
                        keymap,
                        camera,
                    )
                } else {
                    Result::None
                }
            }
            _ => Result::None,
        }
    }
    pub fn draw(&mut self, gizmo: &mut ImageGizmo, camera: &Camera) {
        self.relayout(camera);
        let mut budget = 900;
        self.root
            .draw(gizmo, camera, &mut budget, self.theme.palette());
        if let Some(c) = &mut self.child {
            c.draw(gizmo, camera, &mut budget, self.theme.palette());
        }
    }
}
