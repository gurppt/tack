//! Explicit semantic DTOs, independent of Rust layout and private domain fields.
mod object;
use crate::{Error, Result, WireId, domain_error};
pub use object::{AssetDto, ObjectDto, SourceDto, StyleDto, TextDto, TransformDto};
use serde::{Deserialize, Serialize};
use tack_core::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandDto {
    Batch {
        edits: Vec<CommandDto>,
    },
    AddSource {
        source: SourceDto,
    },
    RemoveSource {
        source: WireId,
    },
    SetSource {
        source: SourceDto,
    },
    AddAsset {
        asset: AssetDto,
    },
    RemoveAsset {
        asset: WireId,
    },
    SetAsset {
        asset: AssetDto,
    },
    AddObject {
        object: ObjectDto,
        index: u32,
    },
    RemoveObject {
        object: WireId,
    },
    SetTransform {
        object: WireId,
        transform: TransformDto,
    },
    SetCrop {
        object: WireId,
        crop: [f64; 4],
    },
    SetOpacity {
        object: WireId,
        opacity: f64,
    },
    SetImageFiltering {
        object: WireId,
        filtering: u8,
    },
    AddGroup {
        group: WireId,
        members: Vec<WireId>,
    },
    RemoveGroup {
        group: WireId,
    },
    SetFrameName {
        object: WireId,
        name: String,
    },
    SetAnnotationStyle {
        object: WireId,
        style: StyleDto,
    },
    SetText {
        object: WireId,
        text: TextDto,
    },
    SetZOrder {
        object: WireId,
        index: u32,
    },
}
fn object_id(id: WireId) -> Result<ObjectId> {
    ObjectId::new(id.value()).map_err(domain_error)
}
fn wire_id(value: u128) -> Result<WireId> {
    WireId::new(value)
}
fn index_wire(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| Error::Invalid("object index"))
}
impl CommandDto {
    pub fn from_command(command: &Command) -> Result<Self> {
        use Command::*;
        Ok(match command {
            Batch(edits) => {
                if edits.len() > 4096 || edits.iter().any(|e| matches!(e, Batch(_))) {
                    return Err(Error::Invalid("flat LAN batch limit"));
                }
                Self::Batch {
                    edits: edits
                        .iter()
                        .map(Self::from_command)
                        .collect::<Result<_>>()?,
                }
            }
            AddSource(source) => Self::AddSource {
                source: SourceDto::from_source(source)?,
            },
            RemoveSource(source) => Self::RemoveSource {
                source: wire_id(source.value())?,
            },
            SetSource(source) => Self::SetSource {
                source: SourceDto::from_source(source)?,
            },
            AddAsset(asset) => Self::AddAsset {
                asset: AssetDto::from_asset(*asset)?,
            },
            RemoveAsset(asset) => Self::RemoveAsset {
                asset: wire_id(asset.value())?,
            },
            SetAsset(asset) => Self::SetAsset {
                asset: AssetDto::from_asset(*asset)?,
            },
            AddObject { object, index } => Self::AddObject {
                object: ObjectDto::from_object(object)?,
                index: index_wire(*index)?,
            },
            RemoveObject(object) => Self::RemoveObject {
                object: wire_id(object.value())?,
            },
            SetTransform { object, transform } => Self::SetTransform {
                object: wire_id(object.value())?,
                transform: TransformDto::from_transform(*transform),
            },
            SetCrop { object, crop } => Self::SetCrop {
                object: wire_id(object.value())?,
                crop: crop.uv_rect(),
            },
            SetOpacity { object, opacity } => Self::SetOpacity {
                object: wire_id(object.value())?,
                opacity: opacity.value(),
            },
            SetImageFiltering { object, filtering } => Self::SetImageFiltering {
                object: wire_id(object.value())?,
                filtering: object::encode_filtering(*filtering),
            },
            AddGroup(group) => Self::AddGroup {
                group: wire_id(group.id().value())?,
                members: group
                    .members()
                    .iter()
                    .map(|id| wire_id(id.value()))
                    .collect::<Result<_>>()?,
            },
            RemoveGroup(group) => Self::RemoveGroup {
                group: wire_id(group.value())?,
            },
            SetFrameName { object, name } => Self::SetFrameName {
                object: wire_id(object.value())?,
                name: name.clone(),
            },
            SetAnnotationStyle { object, style } => Self::SetAnnotationStyle {
                object: wire_id(object.value())?,
                style: StyleDto::from_style(*style),
            },
            SetText { object, text } => Self::SetText {
                object: wire_id(object.value())?,
                text: TextDto::from_text(text),
            },
            SetZOrder { object, index } => Self::SetZOrder {
                object: wire_id(object.value())?,
                index: index_wire(*index)?,
            },
        })
    }
    pub fn to_command(&self) -> Result<Command> {
        use CommandDto::*;
        Ok(match self {
            Batch { edits } => {
                if edits.len() > 4096 || edits.iter().any(|e| matches!(e, Batch { .. })) {
                    return Err(Error::Invalid("flat LAN batch limit"));
                }
                Command::Batch(edits.iter().map(Self::to_command).collect::<Result<_>>()?)
            }
            AddSource { source } => Command::AddSource(source.to_source()?),
            RemoveSource { source } => {
                Command::RemoveSource(SourceId::new(source.value()).map_err(domain_error)?)
            }
            SetSource { source } => Command::SetSource(source.to_source()?),
            AddAsset { asset } => Command::AddAsset(asset.to_asset()?),
            RemoveAsset { asset } => {
                Command::RemoveAsset(AssetId::new(asset.value()).map_err(domain_error)?)
            }
            SetAsset { asset } => Command::SetAsset(asset.to_asset()?),
            AddObject { object, index } => Command::AddObject {
                object: object.to_object()?,
                index: *index as usize,
            },
            RemoveObject { object } => Command::RemoveObject(object_id(*object)?),
            SetTransform { object, transform } => Command::SetTransform {
                object: object_id(*object)?,
                transform: transform.to_transform()?,
            },
            SetCrop { object, crop } => Command::SetCrop {
                object: object_id(*object)?,
                crop: Crop::new(crop[0], crop[1], crop[2], crop[3]).map_err(domain_error)?,
            },
            SetOpacity { object, opacity } => Command::SetOpacity {
                object: object_id(*object)?,
                opacity: Opacity::new(*opacity).map_err(domain_error)?,
            },
            SetImageFiltering { object, filtering } => Command::SetImageFiltering {
                object: object_id(*object)?,
                filtering: object::decode_filtering(*filtering)?,
            },
            AddGroup { group, members } => {
                if members.len() > 100_000 {
                    return Err(Error::Invalid("group member limit"));
                }
                Command::AddGroup(
                    Group::new(
                        GroupId::new(group.value()).map_err(domain_error)?,
                        members
                            .iter()
                            .map(|id| object_id(*id))
                            .collect::<Result<_>>()?,
                    )
                    .map_err(domain_error)?,
                )
            }
            RemoveGroup { group } => {
                Command::RemoveGroup(GroupId::new(group.value()).map_err(domain_error)?)
            }
            SetFrameName { object, name } => {
                validate_frame_name(name).map_err(domain_error)?;
                Command::SetFrameName {
                    object: object_id(*object)?,
                    name: name.clone(),
                }
            }
            SetAnnotationStyle { object, style } => Command::SetAnnotationStyle {
                object: object_id(*object)?,
                style: style.to_style()?,
            },
            SetText { object, text } => Command::SetText {
                object: object_id(*object)?,
                text: text.to_text()?,
            },
            SetZOrder { object, index } => Command::SetZOrder {
                object: object_id(*object)?,
                index: *index as usize,
            },
        })
    }
}
