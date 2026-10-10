use crate::{ContentHash, Error, Result, WireId, decode_hex, domain_error, encode_hex};
use serde::{Deserialize, Serialize};
use tack_core::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {
    pub source: WireId,
    pub revision: u64,
    pub hash: ContentHash,
    pub size: u64,
}
impl SourceBinding {
    pub fn validate(&self) -> Result<()> {
        if self.revision == 0 || self.size > crate::MAX_ASSET_BYTES {
            return Err(Error::Invalid("source binding revision/length"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentRecord {
    pub schema: u32,
    pub counts: [u32; 3],
    pub metadata: String,
}
impl DocumentRecord {
    pub fn from_document(document: &Document) -> Result<Self> {
        let document = shared_document(document)?;
        let (schema, counts, bytes) = tack_storage::encode_metadata(&document)?;
        if bytes.len() > crate::MAX_FRAME_BYTES / 2 - 4096 {
            return Err(Error::Invalid("LAN snapshot metadata budget"));
        }
        Ok(Self {
            schema,
            counts,
            metadata: encode_hex(&bytes),
        })
    }
    pub fn to_document(&self) -> Result<Document> {
        let bytes = decode_hex(&self.metadata, crate::MAX_FRAME_BYTES / 2)?;
        let document = tack_storage::decode_metadata(self.schema, self.counts, &bytes)?;
        if document
            .sources()
            .any(|s| !matches!(s.location(), SourceLocation::Embedded))
        {
            return Err(Error::Invalid("remote filesystem source path"));
        }
        Ok(document)
    }
}
/// Keep IDs/layout/semantics and remove machine-specific filesystem bindings.
/// Originals are resolved by SourceBinding, never by remote path text.
pub fn shared_document(document: &Document) -> Result<Document> {
    let mut result = Document::new(document.id(), document.limits());
    for source in document.sources() {
        result
            .apply(Command::AddSource(
                Source::from_descriptor(
                    source.id(),
                    SourceLocation::Embedded,
                    source.revision(),
                    None,
                )
                .map_err(domain_error)?,
            ))
            .map_err(domain_error)?;
    }
    for asset in document.assets() {
        result
            .apply(Command::AddAsset(*asset))
            .map_err(domain_error)?;
    }
    for (index, id) in document.object_order().iter().enumerate() {
        let object = document
            .object(*id)
            .ok_or(Error::Invalid("document object order"))?;
        result
            .apply(Command::AddObject {
                object: object.clone(),
                index,
            })
            .map_err(domain_error)?;
    }
    result
        .apply(Command::SetFrameLinks(
            document
                .frame_links()
                .map(|(child, parent)| (child, Some(parent)))
                .collect(),
        ))
        .map_err(domain_error)?;
    for group in document.groups() {
        result
            .apply(Command::AddGroup(group.clone()))
            .map_err(domain_error)?;
    }
    Ok(result)
}
