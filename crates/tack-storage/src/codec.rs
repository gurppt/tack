use crate::{BlobRange, MAX_RECORDS, OverviewEntry, Result, StorageError};
use tack_core::*;

pub(crate) struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Decoder<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    pub fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(size)
            .ok_or(StorageError::Invalid("record overflow"))?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(StorageError::Invalid("truncated record"))?;
        self.pos = end;
        Ok(slice)
    }
    pub fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| StorageError::Invalid("u16"))?,
        ))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| StorageError::Invalid("u32"))?,
        ))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| StorageError::Invalid("u64"))?,
        ))
    }
    pub fn id(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(
            self.take(16)?
                .try_into()
                .map_err(|_| StorageError::Invalid("ID"))?,
        ))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.u64()?))
    }
    pub fn boolean(&mut self) -> Result<bool> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(StorageError::Invalid("boolean")),
        }
    }
    pub fn version(&mut self) -> Result<()> {
        if self.u16()? == 1 {
            Ok(())
        } else {
            Err(StorageError::Unsupported("record version"))
        }
    }
    pub fn finish(&self) -> Result<()> {
        if self.pos == self.bytes.len() {
            Ok(())
        } else {
            Err(StorageError::Unsupported("additional record fields"))
        }
    }
}
fn invalid(_: impl std::fmt::Display) -> StorageError {
    StorageError::Invalid("domain/reference validation")
}
pub(crate) fn encode_document(doc: &Document) -> Result<Vec<u8>> {
    if [
        doc.sources().count(),
        doc.assets().count(),
        doc.object_order().len(),
    ]
    .into_iter()
    .any(|n| n > MAX_RECORDS)
    {
        return Err(StorageError::Invalid("record count"));
    }
    let estimated = 16usize
        + doc
            .sources()
            .map(|s| {
                64 + match s.location() {
                    SourceLocation::Embedded => 0,
                    SourceLocation::Linked(p) => p.bytes().len(),
                }
            })
            .sum::<usize>()
        + doc.assets().count() * 42
        + doc.object_order().len() * 135
        + doc
            .objects()
            .map(|o| match o.kind() {
                ObjectKind::Frame(n) => n.len() + 4,
                ObjectKind::Annotation(a) => crate::annotation_codec::estimate(a),
                _ => 0,
            })
            .sum::<usize>()
        + doc
            .groups()
            .map(|g| 22 + g.members().len() * 16)
            .sum::<usize>();
    if estimated > crate::MAX_METADATA_BYTES {
        return Err(StorageError::Invalid("metadata budget"));
    }
    let mut out = Vec::new();
    out.extend(doc.id().value().to_le_bytes());
    for s in doc.sources() {
        let mut record = Vec::new();
        record.extend(1u16.to_le_bytes());
        record.extend(s.id().value().to_le_bytes());
        record.extend(s.revision().to_le_bytes());
        record.push(u8::from(s.fingerprint().is_some()));
        if let Some(f) = s.fingerprint() {
            record.extend(f.size.to_le_bytes());
            record.extend(f.modified_seconds.to_le_bytes());
            record.extend(f.modified_nanos.to_le_bytes());
        }
        match s.location() {
            SourceLocation::Embedded => record.push(0),
            SourceLocation::Linked(p) => {
                record.push(match p.platform() {
                    PathPlatform::Unix => 1,
                    PathPlatform::Windows => 2,
                });
                record.push(u8::from(p.is_absolute()));
                record.extend((p.bytes().len() as u32).to_le_bytes());
                record.extend(p.bytes());
            }
        }
        out.extend((record.len() as u32).to_le_bytes());
        out.extend(record);
    }
    for a in doc.assets() {
        out.extend(1u16.to_le_bytes());
        out.extend(a.id().value().to_le_bytes());
        out.extend(a.source_id().value().to_le_bytes());
        for v in a.pixel_size() {
            out.extend(v.to_le_bytes());
        }
    }
    for id in doc.object_order() {
        let o = doc
            .object(*id)
            .ok_or(StorageError::Invalid("object order"))?;
        if let ObjectKind::Annotation(a) = o.kind() {
            crate::annotation_codec::encode(&mut out, o, a);
            continue;
        }
        if let ObjectKind::Frame(name) = o.kind() {
            out.extend(1u16.to_le_bytes());
            out.extend(2u16.to_le_bytes());
            out.extend(id.value().to_le_bytes());
            let t = o.transform();
            for v in t.center().into_iter().chain(t.size()) {
                out.extend(v.to_le_bytes());
            }
            out.extend((name.len() as u16).to_le_bytes());
            out.extend(name.as_bytes());
            continue;
        }
        let ObjectKind::Image(i) = o.kind() else {
            return Err(StorageError::Invalid("object kind"));
        };
        out.extend(1u16.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(id.value().to_le_bytes());
        out.extend(i.asset_id().value().to_le_bytes());
        let t = o.transform();
        for v in t.center().into_iter().chain(t.size()).chain([t.rotation()]) {
            out.extend(v.to_le_bytes());
        }
        for v in t.flips() {
            out.push(u8::from(v));
        }
        for v in i.crop().uv_rect() {
            out.extend(v.to_le_bytes());
        }
        out.extend(i.opacity().value().to_le_bytes());
        out.push(match i.filtering() {
            ImageFiltering::Default => 0,
            ImageFiltering::Smooth => 1,
            ImageFiltering::Nearest => 2,
        });
    }
    for id in doc.object_order() {
        out.extend(id.value().to_le_bytes());
    }
    if spatial_schema(doc) >= 2 {
        out.extend((doc.groups().count() as u32).to_le_bytes());
        for g in doc.groups() {
            out.extend(1u16.to_le_bytes());
            out.extend(g.id().value().to_le_bytes());
            out.extend((g.members().len() as u32).to_le_bytes());
            for id in g.members() {
                out.extend(id.value().to_le_bytes());
            }
        }
    }
    Ok(out)
}
pub(crate) fn spatial_schema(doc: &Document) -> u32 {
    if doc
        .objects()
        .any(|o| matches!(o.kind(), ObjectKind::Annotation(_)))
    {
        return 3;
    }
    if doc.groups().next().is_some()
        || doc
            .objects()
            .any(|o| matches!(o.kind(), ObjectKind::Frame(_)))
    {
        2
    } else {
        1
    }
}
pub(crate) fn decode_document(
    d: &mut Decoder<'_>,
    counts: [usize; 3],
    schema: u32,
) -> Result<Document> {
    // Counts and enclosing byte length already checked before allocation by reader.
    let mut doc = Document::new(
        DocumentId::new(d.id()?).map_err(invalid)?,
        DocumentLimits::default(),
    );
    for _ in 0..counts[0] {
        let len = d.u32()? as usize;
        if len > 8192 {
            return Err(StorageError::Invalid("source record length"));
        }
        let mut r = Decoder::new(d.take(len)?);
        r.version()?;
        let id = SourceId::new(r.id()?).map_err(invalid)?;
        let revision = r.u64()?;
        let fingerprint = if r.boolean()? {
            Some(SourceFingerprint {
                size: r.u64()?,
                modified_seconds: r.u64()? as i64,
                modified_nanos: r.u32()?,
            })
        } else {
            None
        };
        let kind = r.byte()?;
        let loc = match kind {
            0 => SourceLocation::Embedded,
            1 | 2 => {
                let absolute = r.boolean()?;
                let len = r.u32()? as usize;
                if len > MAX_SOURCE_PATH_BYTES {
                    return Err(StorageError::Invalid("linked path length"));
                }
                SourceLocation::Linked(
                    LinkedPath::encoded(
                        if kind == 1 {
                            PathPlatform::Unix
                        } else {
                            PathPlatform::Windows
                        },
                        absolute,
                        r.take(len)?,
                    )
                    .map_err(invalid)?,
                )
            }
            _ => return Err(StorageError::Unsupported("source kind")),
        };
        r.finish()?;
        doc.apply(Command::AddSource(
            Source::from_descriptor(id, loc, revision, fingerprint).map_err(invalid)?,
        ))
        .map_err(invalid)?;
    }
    for _ in 0..counts[1] {
        d.version()?;
        let id = AssetId::new(d.id()?).map_err(invalid)?;
        let source = SourceId::new(d.id()?).map_err(invalid)?;
        let pixels = [d.u32()?, d.u32()?];
        doc.apply(Command::AddAsset(
            ImageAsset::new(id, source, pixels).map_err(invalid)?,
        ))
        .map_err(invalid)?;
    }
    for index in 0..counts[2] {
        d.version()?;
        let kind = d.u16()?;
        if kind != 1 && !(schema >= 2 && kind == 2) && !(schema == 3 && (3..=8).contains(&kind)) {
            return Err(StorageError::Unsupported("object kind"));
        }
        let id = ObjectId::new(d.id()?).map_err(invalid)?;
        if kind >= 3 {
            let object = crate::annotation_codec::decode(d, id, kind)?;
            doc.apply(Command::AddObject { object, index })
                .map_err(invalid)?;
            continue;
        }
        if kind == 2 {
            let center = [d.f64()?, d.f64()?];
            let size = [d.f64()?, d.f64()?];
            let len = d.u16()? as usize;
            if len > MAX_FRAME_NAME_BYTES {
                return Err(StorageError::Invalid("frame name length"));
            }
            let name = std::str::from_utf8(d.take(len)?)
                .map_err(invalid)?
                .to_owned();
            let object = DocumentObject::frame(
                id,
                name,
                Transform::new(center, size, 0., [false; 2]).map_err(invalid)?,
            )
            .map_err(invalid)?;
            doc.apply(Command::AddObject { object, index })
                .map_err(invalid)?;
            continue;
        }
        let asset = AssetId::new(d.id()?).map_err(invalid)?;
        let center = [d.f64()?, d.f64()?];
        let size = [d.f64()?, d.f64()?];
        let rotation = d.f64()?;
        let flips = [d.boolean()?, d.boolean()?];
        let t = Transform::new(center, size, rotation, flips).map_err(invalid)?;
        let crop = Crop::new(d.f64()?, d.f64()?, d.f64()?, d.f64()?).map_err(invalid)?;
        let opacity = Opacity::new(d.f64()?).map_err(invalid)?;
        let filtering = match d.byte()? {
            0 => ImageFiltering::Default,
            1 => ImageFiltering::Smooth,
            2 => ImageFiltering::Nearest,
            _ => return Err(StorageError::Unsupported("filtering value")),
        };
        for c in [
            Command::AddObject {
                object: DocumentObject::image(id, asset, t),
                index,
            },
            Command::SetCrop { object: id, crop },
            Command::SetOpacity {
                object: id,
                opacity,
            },
            Command::SetImageFiltering {
                object: id,
                filtering,
            },
        ] {
            doc.apply(c).map_err(invalid)?;
        }
    }
    for id in doc.object_order() {
        if d.id()? != id.value() {
            return Err(StorageError::Invalid("ordering metadata"));
        }
    }
    if schema >= 2 {
        let count = d.u32()? as usize;
        if count > MAX_RECORDS || count > counts[2] / 2 {
            return Err(StorageError::Invalid("group count"));
        }
        let mut total = 0usize;
        for _ in 0..count {
            d.version()?;
            let id = GroupId::new(d.id()?).map_err(invalid)?;
            let len = d.u32()? as usize;
            total = total
                .checked_add(len)
                .ok_or(StorageError::Invalid("group members"))?;
            if len < 2 || total > counts[2] {
                return Err(StorageError::Invalid("group members"));
            }
            let bytes = d.take(len * 16)?;
            let mut r = Decoder::new(bytes);
            let mut members = Vec::with_capacity(len);
            for _ in 0..len {
                members.push(ObjectId::new(r.id()?).map_err(invalid)?);
            }
            doc.apply(Command::AddGroup(Group::new(id, members).map_err(invalid)?))
                .map_err(invalid)?;
        }
    }
    Ok(doc)
}
pub(crate) fn encode_entry(entry: &OverviewEntry, kind: u16) -> Vec<u8> {
    let mut b = Vec::with_capacity(64);
    b.extend(1u16.to_le_bytes());
    b.extend(kind.to_le_bytes());
    b.extend(entry.id.to_le_bytes());
    b.extend(entry.revision.to_le_bytes());
    b.extend(entry.role.to_le_bytes());
    b.extend(entry.encoding.to_le_bytes());
    b.extend(entry.generator.to_le_bytes());
    b.extend(entry.width.to_le_bytes());
    b.extend(entry.height.to_le_bytes());
    b.extend(entry.range.offset.to_le_bytes());
    b.extend(entry.range.len.to_le_bytes());
    b.extend(entry.range.crc32.to_le_bytes());
    b
}
pub(crate) fn decode_entry(d: &mut Decoder<'_>, kind: u16) -> Result<OverviewEntry> {
    d.version()?;
    if d.u16()? != kind {
        return Err(StorageError::Unsupported("directory kind"));
    }
    let entry = OverviewEntry {
        id: d.id()?,
        revision: d.u64()?,
        role: d.u16()?,
        encoding: d.u16()?,
        generator: d.u32()?,
        width: d.u32()?,
        height: d.u32()?,
        range: BlobRange {
            offset: d.u64()?,
            len: d.u64()?,
            crc32: d.u32()?,
        },
    };
    if entry.id == 0 || entry.revision == 0 {
        return Err(StorageError::Invalid("directory identity/revision"));
    }
    Ok(entry)
}
