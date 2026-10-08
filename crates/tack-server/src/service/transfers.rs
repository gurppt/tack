use super::refusal;
use crate::{AssetStore, Result, Upload};
use tack_shared::{Message, RefusalCode};
pub(super) fn asset_message(
    message: Message,
    assets: &AssetStore,
    upload: &mut Option<Upload>,
) -> Result<Option<Message>> {
    let result: Result<Option<Message>> = (|| match message {
        Message::AssetBegin { hash, size } => {
            if upload.is_some() {
                return Err("one upload per connection".into());
            }
            *upload = assets.begin(hash.clone(), size)?;
            Ok(Some(Message::AssetStatus {
                hash,
                size,
                present: upload.is_none(),
            }))
        }
        Message::AssetChunk {
            hash,
            offset,
            bytes,
        } => {
            let active = upload.as_mut().ok_or("upload begin required")?;
            if active.hash() != hash {
                return Err("upload hash mismatch".into());
            }
            let bytes = tack_shared::decode_hex(&bytes, tack_shared::MAX_CHUNK_BYTES)
                .map_err(|e| e.to_string())?;
            let offset = active.append(offset, &bytes)?;
            Ok(Some(Message::AssetProgress { hash, offset }))
        }
        Message::AssetCommit { hash } => {
            if upload.as_ref().is_some_and(|u| u.hash() != hash) {
                return Err("upload hash mismatch".into());
            }
            let size = upload.take().ok_or("upload begin required")?.commit()?;
            Ok(Some(Message::AssetReady { hash, size }))
        }
        Message::AssetCancel { hash } => {
            if upload.as_ref().is_some_and(|u| u.hash() == hash) {
                upload.take();
            }
            Ok(Some(Message::AssetCancelled { hash }))
        }
        Message::AssetGet {
            hash,
            offset,
            length,
        } => {
            let (size, bytes) = assets.range(hash.clone(), offset, length)?;
            Ok(Some(Message::AssetData {
                hash,
                offset,
                size,
                bytes: tack_shared::encode_hex(&bytes),
            }))
        }
        _ => Ok(None),
    })();
    match result {
        Ok(response) => Ok(response),
        Err(e) => {
            upload.take();
            Ok(Some(refusal(None, 0, RefusalCode::InvalidAsset, &e)))
        }
    }
}
