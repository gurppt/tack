//! Conflict dependencies for semantic edits; never a second document authority.
use crate::{CommandDto, WireId};
use std::collections::BTreeSet;
use tack_core::{Document, ObjectKind};

#[derive(Default)]
pub struct CommandScope {
    pub objects: BTreeSet<WireId>,
    pub reads_dependencies: bool,
    pub writes_dependencies: bool,
    pub reads_order: bool,
    pub writes_order: bool,
}
impl CommandDto {
    pub fn scope(&self, document: &Document) -> CommandScope {
        let mut scope = CommandScope::default();
        self.collect_scope(document, &mut scope);
        scope
    }
    fn collect_scope(&self, document: &Document, scope: &mut CommandScope) {
        use CommandDto::*;
        match self {
            Batch { edits } => {
                for edit in edits {
                    edit.collect_scope(document, scope);
                }
            }
            AddObject { object, .. } => {
                // The DTO's validated core object supplies its stable ID.
                if let Ok(object) = object.to_object() {
                    if let Ok(id) = WireId::new(object.id().value()) {
                        scope.objects.insert(id);
                    }
                    scope.reads_dependencies |= matches!(object.kind(), ObjectKind::Image(_));
                }
                scope.writes_order = true;
            }
            RemoveObject { object } => {
                scope.objects.insert(*object);
                scope.writes_order = true;
            }
            SetTransform { object, .. }
            | SetCrop { object, .. }
            | SetOpacity { object, .. }
            | SetImageFiltering { object, .. }
            | SetFrameName { object, .. }
            | SetFrameColor { object, .. }
            | SetAnnotationStyle { object, .. }
            | SetText { object, .. } => {
                scope.objects.insert(*object);
            }
            SetZOrder { object, .. } => {
                scope.objects.insert(*object);
                scope.reads_order = true;
                scope.writes_order = true;
            }
            AddGroup { members, .. } => {
                scope.objects.extend(members);
                scope.reads_dependencies = true;
                scope.writes_dependencies = true;
            }
            RemoveGroup { group } => {
                if let Ok(id) = tack_core::GroupId::new(group.value())
                    && let Some(group) = document.group(id)
                {
                    scope.objects.extend(
                        group
                            .members()
                            .iter()
                            .filter_map(|id| WireId::new(id.value()).ok()),
                    );
                }
                scope.reads_dependencies = true;
                scope.writes_dependencies = true;
            }
            AddSource { .. } | AddAsset { .. } => {
                scope.reads_dependencies = true;
            }
            RemoveSource { source } => scope.descriptor(document, Some(*source), None),
            SetSource { source } => {
                if let Ok(source) = source.to_source() {
                    scope.descriptor(document, WireId::new(source.id().value()).ok(), None);
                }
            }
            RemoveAsset { asset } => scope.descriptor(document, None, Some(*asset)),
            SetAsset { asset } => {
                if let Ok(asset) = asset.to_asset() {
                    scope.descriptor(document, None, WireId::new(asset.id().value()).ok());
                }
            }
        }
    }
}
impl CommandScope {
    fn descriptor(&mut self, document: &Document, source: Option<WireId>, asset: Option<WireId>) {
        self.reads_dependencies = true;
        self.writes_dependencies = true;
        for object in document.objects() {
            if let ObjectKind::Image(image) = object.kind()
                && let Some(descriptor) = document.asset(image.asset_id())
                && (asset.is_some_and(|id| id.value() == descriptor.id().value())
                    || source.is_some_and(|id| id.value() == descriptor.source_id().value()))
                && let Ok(id) = WireId::new(object.id().value())
            {
                self.objects.insert(id);
            }
        }
    }
}
