//! Metadata-only resource plan. Decoder read/scratch/output caps stay independent.
use crate::{
    AssetError,
    representation::{self, BudgetReader, CountRead, Format, sniff},
};
use image::{ImageReader, Limits};
use std::{fs::File, io::BufReader, path::Path};
pub struct ImportPlan {
    pub pixels: [u32; 2],
    pub header_reads: u64,
    pub max_encoded: u64,
    pub format: &'static str,
    pub streamed: bool,
}
pub fn image_metadata(path: &Path) -> Result<([u32; 2], u64), AssetError> {
    let plan = inspect(path, None)?;
    Ok((plan.pixels, plan.header_reads))
}
pub fn image_import_plan(path: &Path, encoded_bytes: u64) -> Result<ImportPlan, AssetError> {
    inspect(path, Some(encoded_bytes))
}
fn inspect(path: &Path, encoded_bytes: Option<u64>) -> Result<ImportPlan, AssetError> {
    let mut source = CountRead {
        inner: BufReader::with_capacity(8192, File::open(path)?),
        count: 0,
    };
    let format = sniff(&mut source)?;
    let force_streamed = encoded_bytes.is_some_and(|n| n > representation::ENCODED_LIMIT);
    let mut streamed = false;
    let (size, read_budget) = match format {
        Format::Jpeg => {
            let header = crate::jpeg_header::read(&mut source)?;
            streamed = !header.ordinary() || force_streamed;
            if streamed {
                header.require_streamed()?;
            }
            (
                header.size,
                if streamed {
                    crate::jpeg_header::ENCODED_LIMIT
                } else {
                    representation::ENCODED_LIMIT
                },
            )
        }
        Format::Png => {
            let remaining = crate::huge_image::PNG_ENCODED_LIMIT.saturating_sub(source.count);
            let (size, requires_streaming) = crate::huge_image::png_layout_mode(
                BudgetReader::with_limit(&mut source, remaining, None),
                force_streamed,
            )?;
            streamed = requires_streaming;
            (
                size,
                if streamed {
                    crate::huge_image::PNG_ENCODED_LIMIT
                } else {
                    representation::ENCODED_LIMIT
                },
            )
        }
        Format::Other => {
            let mut reader = ImageReader::new(BufReader::new(BudgetReader::new(&mut source)))
                .with_guessed_format()?;
            let mut limits = Limits::default();
            limits.max_image_width = Some(6000);
            limits.max_image_height = Some(4500);
            limits.max_alloc = Some(192 * 1024 * 1024);
            reader.limits(limits);
            let (w, h) = reader.into_dimensions()?;
            if w == 0 || h == 0 || w > 6000 || h > 4500 {
                return Err("supported image dimensions are 1..6000 x 1..4500".into());
            }
            ([w, h], representation::ENCODED_LIMIT)
        }
    };
    // The existing renderer probes layout then rewinds for derivation. Reserve
    // that bounded header read plus two 8 KiB buffers instead of promising the
    // entire cumulative read budget as an encoded-original allowance.
    let max_encoded = if streamed {
        read_budget.saturating_sub(source.count.saturating_add(16 * 1024))
    } else {
        read_budget
    };
    Ok(ImportPlan {
        pixels: size,
        header_reads: source.count,
        max_encoded,
        format: match format {
            Format::Jpeg => "jpeg",
            Format::Png => "png",
            Format::Other => "other",
        },
        streamed,
    })
}
