//! Actual GPU inventory regressions; explicitly run on a supported adapter.
use tack_app::supply_plan::displayed_lod;
use tack_assets::{AssetError, Decoded};
use tack_core::{Lod, SourceId};
use tack_render::{Gpu, ProductKey};
fn key(source: u128, lod: Lod) -> Result<ProductKey, AssetError> {
    Ok(ProductKey {
        asset: None,
        source: SourceId::new(source)?,
        revision: 1,
        lod,
        edge: lod.edge(),
    })
}
fn upload(gpu: &mut Gpu, source: u128, lod: Lod) -> Result<(), AssetError> {
    assert!(gpu.begin_frame()?);
    let edge = lod.edge();
    assert!(gpu.upload_product(
        key(source, lod)?,
        &Decoded {
            width: edge,
            height: edge,
            rgba: vec![255; (edge as usize).pow(2) * 4]
        }
    ));
    Ok(())
}
#[test]
#[ignore = "requires a supported GPU"]
fn admitted_residents_survive_late_upload_that_previously_evicted_first_draw()
-> Result<(), AssetError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    for protect in [false, true] {
        let mut gpu = pollster::block_on(Gpu::new(
            &instance,
            None,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            128 * 1024 * 1024,
        ))?;
        for s in 1..=3 {
            upload(&mut gpu, s, Lod::Medium)?;
        }
        for s in 1..=3 {
            upload(&mut gpu, s, Lod::Detail)?;
        }
        upload(&mut gpu, 4, Lod::Medium)?;
        if protect {
            for s in 1..=4 {
                gpu.touch_product(key(s, Lod::Detail)?);
            }
        }
        upload(&mut gpu, 4, Lod::Detail)?;
        assert_eq!(
            gpu.contains_product(key(1, Lod::Detail)?),
            protect,
            "old selection counted the first image before D4 evicted it"
        );
        if protect {
            for s in 1..=4 {
                assert!(gpu.contains_product(key(s, Lod::Detail)?));
            }
        }
    }
    Ok(())
}
#[test]
#[ignore = "requires a supported GPU"]
fn late_lower_publication_and_repeated_crossings_keep_adequate_quality() -> Result<(), AssetError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
    let mut gpu = pollster::block_on(Gpu::new(
        &instance,
        None,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        128 * 1024 * 1024,
    ))?;
    upload(&mut gpu, 1, Lod::Detail)?;
    for delayed in [true, false] {
        if !delayed {
            upload(&mut gpu, 1, Lod::Medium)?;
        }
        for _ in 0..20 {
            for edge in [2048., 513., 512., 129., 128., 129., 512., 513., 400., 80.] {
                let desired = Lod::for_projected_edge(edge);
                let displayed = displayed_lod(desired, |l| {
                    key(1, l).is_ok_and(|k| gpu.contains_product(k))
                });
                assert!(
                    displayed.is_some_and(|l| l >= desired),
                    "hole at {edge}: {displayed:?}"
                );
            }
        }
    }
    Ok(())
}
