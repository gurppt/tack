//! Transient two-stage Frame picking; reuses the ordinary selection and marquee.
use super::*;
use crate::cursors::Kind;
use tack_core::{Command, ObjectId, ObjectKind};

pub(super) struct Stage {
    generation: u64,
    children: Vec<ObjectId>,
    anchors: Vec<[f64; 2]>,
    frames: Vec<(ObjectId, Transform)>,
    hover: Option<ObjectId>,
    pressed: Option<ObjectId>,
    forbidden: bool,
}
impl ImageInput {
    pub fn settle_link_feedback(&mut self, now: Instant, editor: Option<&DocumentEditor>) -> bool {
        if let Some((generation, frame, children)) = &self.link_pending
            && let Some(editor) = editor
            && editor.generation() != *generation
        {
            let accepted = children
                .iter()
                .all(|id| editor.document().frame_parent(*id) == Some(*frame));
            let frame = *frame;
            if accepted {
                self.link_pending = None;
                self.link_flash = Some(frame);
                self.link_feedback.acknowledge(now);
                self.status = "Linked to Frame".into();
                return true;
            }
            if let Some(pending) = &mut self.link_pending {
                pending.0 = editor.generation();
            }
        }
        if self.link_feedback.settle(now) {
            self.link_pending = None;
            self.link_flash = None;
            return true;
        }
        false
    }
    pub fn link_cursor(&self) -> Option<Kind> {
        self.link.as_ref().map(|s| {
            if s.forbidden {
                Kind::Forbidden
            } else {
                Kind::LinkOpen
            }
        })
    }
    fn choose_frame(&mut self, editor: &DocumentEditor) -> Result<(), AssetError> {
        let doc = editor.document();
        self.images.selection.expand_groups(doc);
        let children: Vec<_> = self.images.selection.ids().collect();
        if children.iter().any(|id| {
            self.images.blocked.contains(id)
                || doc
                    .object(*id)
                    .is_none_or(|o| matches!(o.kind(), ObjectKind::Frame(_)))
        }) {
            return Err("Link to Frame requires available non-Frame objects".into());
        }
        let frames: Vec<_> = doc
            .object_order()
            .iter()
            .rev()
            .filter_map(|id| {
                doc.object(*id)
                    .filter(|o| matches!(o.kind(), ObjectKind::Frame(_)))
                    .map(|o| (*id, o.transform()))
            })
            .collect();
        if frames.is_empty() {
            return Err("Create a Frame before linking objects".into());
        }
        let mut units = std::collections::BTreeSet::new();
        let anchors = children
            .iter()
            .filter_map(|id| {
                let unit = doc.group_for(*id).map_or(*id, |g| g.members()[0]);
                if !units.insert(unit) {
                    return None;
                }
                let members: Vec<_> = doc
                    .group_for(*id)
                    .map_or_else(|| vec![*id], |g| g.members().to_vec());
                let mut bounds = doc.object(unit)?.transform().bounds();
                for member in members {
                    let b = doc.object(member)?.transform().bounds();
                    let x = bounds.x.min(b.x);
                    let y = bounds.y.min(b.y);
                    bounds = WorldRect {
                        x,
                        y,
                        width: (bounds.x + bounds.width).max(b.x + b.width) - x,
                        height: (bounds.y + bounds.height).max(b.y + b.height) - y,
                    };
                }
                Some([bounds.x + bounds.width / 2., bounds.y + bounds.height / 2.])
            })
            .collect();
        self.link = Some(Stage {
            generation: editor.generation(),
            children,
            anchors,
            frames,
            hover: None,
            pressed: None,
            forbidden: false,
        });
        self.status = if self.images.selection.is_empty() {
            "Select objects to link"
        } else {
            "Choose a Frame — Escape cancels"
        }
        .into();
        Ok(())
    }
    pub(super) fn link_motion(&mut self, editor: &DocumentEditor, camera: &Camera) -> bool {
        let Some(stage) = &mut self.link else {
            return false;
        };
        if stage.generation != editor.generation() {
            self.link = None;
            self.marquee = None;
            self.active_token = None;
            self.status = "Board changed; link cancelled".into();
            return true;
        }
        let world = camera.screen_to_world(self.cursor);
        if let Some((_, end, _)) = &mut self.marquee {
            *end = world;
        }
        stage.hover = stage.frames.iter().find_map(|(id, t)| {
            (!self.images.blocked.contains(id) && crate::image_geometry::hit(*t, world))
                .then_some(*id)
        });
        stage.forbidden &= stage.hover.is_none();
        true
    }
    pub(super) fn link_dispatch(
        &mut self,
        event: ActionEvent,
        editor: &mut DocumentEditor,
        camera: &Camera,
    ) -> Result<bool, AssetError> {
        let ActionEvent { action, phase } = event;
        if phase == ActionPhase::Invoke {
            match action {
                Action::LinkToFrame => {
                    self.commit_drafts(editor)?;
                    self.cancel();
                    self.choose_frame(editor)?;
                    self.link_motion(editor, camera);
                    return Ok(true);
                }
                Action::UnlinkFromFrame => {
                    self.cancel();
                    let changes = self
                        .images
                        .selection
                        .ids()
                        .filter(|id| editor.document().frame_parent(*id).is_some())
                        .map(|id| (id, None))
                        .collect::<Vec<_>>();
                    if !changes.is_empty() {
                        editor.execute(Command::SetFrameLinks(changes))?;
                    }
                    return Ok(true);
                }
                Action::SelectLinkedObjects => {
                    self.cancel();
                    let children = self
                        .images
                        .selection
                        .ids()
                        .flat_map(|id| editor.document().linked_children(id))
                        .collect::<Vec<_>>();
                    self.images.selection.clear();
                    for child in children {
                        self.images.selection.select(Some(child), true);
                    }
                    self.images.selection.expand_groups(editor.document());
                    self.images.selection.prune(editor.document());
                    return Ok(true);
                }
                Action::CancelInteraction if self.link.is_some() => {
                    self.cancel();
                    self.status = "Link cancelled".into();
                    return Ok(true);
                }
                _ if self.link.is_some() => self.cancel(),
                _ => {}
            }
        }
        if self.link.is_none() {
            return Ok(false);
        }
        if !matches!(action, Action::ImagePointer | Action::ToggleSelection) {
            if matches!(phase, ActionPhase::Begin(_)) {
                self.cancel();
            }
            return Ok(false);
        }
        self.link_motion(editor, camera);
        let Some(stage) = &self.link else {
            return Ok(true);
        };
        let acquiring = stage.children.is_empty();
        match phase {
            ActionPhase::Begin(token) => {
                self.active_token = Some(token);
                if acquiring {
                    let world = camera.screen_to_world(self.cursor);
                    let hit = self.images.hit_with_tolerance(
                        editor.document(),
                        world,
                        6. / camera.zoom(),
                    );
                    self.images.selection.select_object(
                        editor.document(),
                        hit,
                        action == Action::ToggleSelection,
                    );
                    if hit.is_none() {
                        self.marquee = Some((world, world, action == Action::ToggleSelection));
                    }
                } else if let Some(stage) = &mut self.link {
                    stage.pressed = stage.hover;
                    stage.forbidden = stage.hover.is_none();
                    if stage.forbidden {
                        self.status = "Choose an available Frame".into();
                    }
                }
            }
            ActionPhase::End(token) if self.active_token == Some(token) => {
                self.active_token = None;
                if acquiring {
                    if let Some((a, b, additive)) = self.marquee.take()
                        && a[0] != b[0]
                        && a[1] != b[1]
                    {
                        self.images.selection.marquee(
                            editor.document(),
                            WorldRect::new(
                                a[0].min(b[0]),
                                a[1].min(b[1]),
                                (a[0] - b[0]).abs(),
                                (a[1] - b[1]).abs(),
                            )?,
                            additive,
                        );
                    }
                    let frames = self
                        .images
                        .selection
                        .ids()
                        .filter(|id| {
                            editor
                                .document()
                                .object(*id)
                                .is_some_and(|o| matches!(o.kind(), ObjectKind::Frame(_)))
                        })
                        .collect::<Vec<_>>();
                    for frame in frames {
                        self.images.selection.select(Some(frame), true);
                    }
                    self.choose_frame(editor)?;
                    self.link_motion(editor, camera);
                } else if let Some(stage) = &self.link
                    && let Some(frame) = stage.pressed.filter(|id| Some(*id) == stage.hover)
                {
                    let before = editor.generation();
                    let children = stage.children.clone();
                    let changes = stage.children.iter().map(|id| (*id, Some(frame))).collect();
                    editor.execute(Command::SetFrameLinks(changes))?;
                    // A queued shared edit is not an authoritative confirmation.
                    if editor.generation() != before {
                        self.link_flash = Some(frame);
                        self.link_feedback.acknowledge(Instant::now());
                    }
                    if editor.generation() == before
                        && children
                            .iter()
                            .any(|id| editor.document().frame_parent(*id) != Some(frame))
                    {
                        self.link_pending = Some((before, frame, children));
                        self.link_flash = None;
                        self.link_feedback.acknowledge(Instant::now());
                    }
                    self.link = None;
                    self.status = if editor.generation() == before {
                        "Link requested"
                    } else {
                        "Linked to Frame"
                    }
                    .into();
                }
            }
            ActionPhase::Cancel(_) => self.cancel(),
            _ => {}
        }
        Ok(true)
    }
    pub(super) fn link_overlay(&mut self, editor: &DocumentEditor, camera: &Camera) {
        self.link_preview = Default::default();
        if self.link.is_some() {
            // Selection outlines are transient too. Reserve target/preview/chrome room.
            self.gizmo
                .quads
                .truncate(tack_render::MAX_OVERLAY_QUADS - 512);
        }
        if let Some(frame) = self
            .link_flash
            .filter(|_| self.link_feedback.active())
            .and_then(|id| editor.document().object(id))
        {
            self.gizmo
                .outline(frame.transform(), camera, self.gizmo.style.selection);
        }
        let Some(stage) = &self.link else {
            return;
        };
        if let Some(frame) = stage.hover.and_then(|id| editor.document().object(id)) {
            self.gizmo
                .outline(frame.transform(), camera, self.gizmo.style.selection);
        }
        self.link_preview = crate::link_preview::draw(
            &mut self.gizmo,
            camera,
            &stage.anchors,
            self.cursor,
            tack_render::MAX_OVERLAY_QUADS - 384,
        );
        if self.link_preview.simplified {
            self.status = "Link preview simplified".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tack_core::*;
    #[test]
    fn disjoint_authority_acceptance_keeps_pending_link_until_matching_acceptance()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
        let frame = ObjectId::new(2)?;
        let child = ObjectId::new(3)?;
        let t = Transform::new([0., 0.], [100., 100.], 0., [false; 2])?;
        d.apply(Command::AddObject {
            object: DocumentObject::frame(frame, "Parent".into(), t)?,
            index: 0,
        })?;
        d.apply(Command::AddObject {
            object: DocumentObject::annotation(
                child,
                Annotation::new(AnnotationKind::Rect, AnnotationStyle::default()),
                t,
            )?,
            index: 1,
        })?;
        let mut e = DocumentEditor::new(d, 10);
        let mut i = ImageInput::new()?;
        let now = Instant::now();
        i.link_pending = Some((e.generation(), frame, vec![child]));
        i.link_feedback.acknowledge(now);
        e.execute(Command::SetFrameName {
            object: frame,
            name: "Remote title".into(),
        })?;
        assert!(!i.settle_link_feedback(now, Some(&e)));
        assert!(i.link_pending.is_some());
        assert!(i.link_flash.is_none());
        e.execute(Command::SetFrameLinks(vec![(child, Some(frame))]))?;
        assert!(i.settle_link_feedback(now, Some(&e)));
        assert!(i.link_pending.is_none());
        assert_eq!(i.link_flash, Some(frame));
        assert!(i.settle_link_feedback(now + std::time::Duration::from_secs(2), Some(&e)));
        assert!(i.link_flash.is_none());
        assert!(!i.settle_link_feedback(now + std::time::Duration::from_secs(3), Some(&e)));
        Ok(())
    }
}
