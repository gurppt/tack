//! Small import receipts; private paths/error strings never enter diagnostics.
use std::{io::ErrorKind, path::Path};
use tack_assets::AssetError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Path,
    Fingerprint,
    Metadata,
    Admission,
    Staging,
    SourceChanged,
    Document,
}
impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Path => "path/file-open",
            Self::Fingerprint => "fingerprint",
            Self::Metadata => "metadata/header",
            Self::Admission => "encoded admission",
            Self::Staging => "staging/disk",
            Self::SourceChanged => "source-changed",
            Self::Document => "document admission",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Category {
    Unavailable,
    Permission,
    Budget,
    Variant,
    Malformed,
    Changed,
    Cancelled,
    Io,
    Other,
}
impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unavailable => "source unavailable",
            Self::Permission => "permission denied",
            Self::Budget => "resource limit",
            Self::Variant => "unsupported format/variant",
            Self::Malformed => "invalid/truncated image",
            Self::Changed => "source changed",
            Self::Cancelled => "cancelled",
            Self::Io => "I/O failure",
            Self::Other => "import failed",
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Facts {
    pub encoded_bytes: Option<u64>,
    pub pixels: Option<[u32; 2]>,
}
#[derive(Debug)]
pub struct Failure {
    pub stage: Stage,
    pub category: Category,
    pub code: &'static str,
    pub reason: &'static str,
    pub facts: Facts,
    pub os_code: Option<i32>,
    pub limit: Option<u64>,
}
impl Failure {
    pub fn from_error(stage: Stage, facts: Facts, error: &AssetError) -> Self {
        let mut f = Self {
            stage,
            facts,
            category: Category::Other,
            code: "import-error",
            reason: "Could not import this source",
            os_code: None,
            limit: None,
        };
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            f.os_code = io.raw_os_error();
            (f.category, f.code, f.reason) = match io.kind() {
                ErrorKind::NotFound | ErrorKind::NotADirectory => (
                    Category::Unavailable,
                    "source-unavailable",
                    "Source not found or folder unavailable",
                ),
                ErrorKind::PermissionDenied => (
                    Category::Permission,
                    "permission-denied",
                    "Permission denied while accessing the source or staging file",
                ),
                ErrorKind::StorageFull => (
                    Category::Budget,
                    "disk-full",
                    "Not enough disk space for staging",
                ),
                ErrorKind::UnexpectedEof | ErrorKind::InvalidData => (
                    Category::Malformed,
                    "truncated-or-invalid",
                    "Image data is truncated or invalid",
                ),
                ErrorKind::TimedOut => (Category::Io, "io-timeout", "Source access timed out"),
                _ => (Category::Io, "io-error", "File read/write failed"),
            };
            return f;
        }
        // Only curated messages are shown. An unknown provider may include
        // absolute paths in its error text; retain its fingerprint in DEV logs.
        let text = error.to_string();
        (f.category, f.code, f.reason) = match text.as_str() {
            "image exceeds 64 MiB encoded-input limit" => {
                f.limit = Some(super::MAX_ENCODED_IMAGE);
                (
                    Category::Budget,
                    "encoded-limit",
                    "Encoded original exceeds the 64 MiB import limit",
                )
            }
            "empty image original" => (
                Category::Malformed,
                "empty-original",
                "Image original is empty",
            ),
            "embedded batch exceeds 2 GiB staging limit" => {
                f.limit = Some(super::MAX_IMPORT_BYTES);
                (
                    Category::Budget,
                    "batch-staging-limit",
                    "Embedded batch exceeds the 2 GiB staging limit",
                )
            }
            "embedded session exceeds 4 GiB staging limit" => {
                f.limit = Some(super::MAX_SESSION_IMPORT_BYTES);
                (
                    Category::Budget,
                    "session-staging-limit",
                    "Embedded session exceeds the 4 GiB staging limit",
                )
            }
            "embedded import exceeds 64 MiB/image, 2 GiB/batch or 4 GiB/session" => (
                Category::Budget,
                "staging-limit",
                "Embedded staging exceeds the image, batch or session byte limit",
            ),
            "bounded huge JPEG requires 8-bit single-scan baseline RGB/grayscale; progressive/multiscan requires a coefficient-buffer strategy" => {
                (
                    Category::Variant,
                    "jpeg-streamed-variant",
                    "Large JPEG needs 8-bit single-scan baseline RGB/grayscale; progressive/multiscan is not supported by this bounded decoder",
                )
            }
            "interlaced PNG unsupported by streamed prototype" => (
                Category::Variant,
                "png-interlaced",
                "Large interlaced PNG needs a decoder strategy not yet supported",
            ),
            "animated large/huge PNG is outside static streamed support"
            | "animated PNG is outside static streamed support" => (
                Category::Variant,
                "png-animated",
                "Animated PNG is not supported by the static streamed decoder",
            ),
            "JPEG must have one or three components" => (
                Category::Variant,
                "jpeg-components",
                "JPEG needs grayscale or RGB components; this variant is unsupported",
            ),
            "JPEG dimensions exceed the codec's 65500-axis limit" => (
                Category::Budget,
                "jpeg-axis-limit",
                "JPEG exceeds the codec's 65,500-pixel axis limit",
            ),
            "JPEG scanline scratch exceeds 16 MiB bound" => (
                Category::Budget,
                "jpeg-scratch-limit",
                "JPEG scanline scratch exceeds 16 MiB",
            ),
            "JPEG marker header exceeds 1 MiB parser budget" => (
                Category::Budget,
                "jpeg-header-limit",
                "JPEG header exceeds the 1 MiB inspection limit",
            ),
            "PNG row exceeds streamed scratch budget" => (
                Category::Budget,
                "png-row-limit",
                "PNG row exceeds the bounded decoder's scratch limit",
            ),
            "image dimensions exceed bounded streamed policy"
            | "supported image dimensions are 1..6000 x 1..4500" => (
                Category::Budget,
                "dimension-limit",
                "Image dimensions exceed the supported decoder limits",
            ),
            "source grew during import"
            | "source shortened during import"
            | "source changed during import" => (
                Category::Changed,
                "source-changed",
                "Source changed during import; retry once the writer has finished",
            ),
            "source is not a regular file" => (
                Category::Unavailable,
                "not-a-file",
                "Source is not a regular file",
            ),
            "import cancelled" => (Category::Cancelled, "cancelled", "Import cancelled"),
            _ if tack_assets::image_failure_kind(error) == "unsupported-format" => (
                Category::Variant,
                "unsupported-format",
                "Image format is not supported by the installed decoder",
            ),
            _ if tack_assets::image_failure_kind(error) == "decoder-resource-limit" => (
                Category::Budget,
                "decoder-resource-limit",
                "Image exceeds the decoder's resource limit",
            ),
            _ if tack_assets::image_failure_kind(error) == "decoder-io" => {
                (Category::Io, "decoder-io", "Image data could not be read")
            }
            _ if stage == Stage::Metadata => (
                Category::Malformed,
                "header-unavailable",
                "Image header is invalid or could not be read",
            ),
            _ => (
                Category::Other,
                "import-error",
                "Could not import this source",
            ),
        };
        f
    }
    pub fn description(&self) -> String {
        let size = self
            .facts
            .encoded_bytes
            .map_or(String::new(), |n| format!(" ({n} bytes)"));
        format!("{}: {}{size}", self.stage.label(), self.reason)
    }
}
#[derive(Default)]
pub struct FailureSummary {
    counts: [usize; 9],
    samples: Vec<String>,
    total: usize,
}
impl FailureSummary {
    pub const MAX_SAMPLES: usize = 16;
    pub fn clipboard_rejected(&mut self, count: usize) {
        self.counts[Category::Unavailable as usize] += count;
        self.total += count;
        if self.samples.len() < Self::MAX_SAMPLES {
            self.samples.push(format!(
                "Clipboard: {count} unsupported or non-file references omitted"
            ));
        }
    }
    pub fn record(&mut self, path: &Path, failure: &Failure) {
        self.counts[failure.category as usize] += 1;
        self.total += 1;
        if self.samples.len() < Self::MAX_SAMPLES {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let name: String = name
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or_default()
                .chars()
                .filter(|c| !c.is_control())
                .take(64)
                .collect();
            self.samples
                .push(format!("{name}: {}", failure.description()));
        }
    }
    pub fn rows(&self, admitted: usize, cancelled: bool) -> Vec<String> {
        let mut rows = vec![format!(
            "Import: {admitted} admitted; {} failed",
            self.total
        )];
        if cancelled {
            rows.push("Cancelled; admitted items are retained".into());
        }
        for category in [
            Category::Unavailable,
            Category::Permission,
            Category::Budget,
            Category::Variant,
            Category::Malformed,
            Category::Changed,
            Category::Cancelled,
            Category::Io,
            Category::Other,
        ] {
            let count = self.counts[category as usize];
            if count > 0 {
                rows.push(format!("{count}: {}", category.label()));
            }
        }
        rows.extend(self.samples.iter().cloned());
        if self.total > self.samples.len() {
            rows.push(format!(
                "{} further failures; samples limited to 16",
                self.total - self.samples.len()
            ));
        }
        rows
    }
}

#[cfg(test)]
mod tests;
