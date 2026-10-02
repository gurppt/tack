use tack_core::{ByteCache, Camera, Lod, WorldRect};

#[test]
fn cursor_anchor_survives_zoom_and_clamping() -> Result<(), Box<dyn std::error::Error>> {
    let mut camera = Camera::new([1920, 1080]);
    camera.set_view([123456.0, -78901.0], 0.25)?;
    let cursor = [1700.0, 320.0];
    let before = camera.screen_to_world(cursor);
    for factor in [4.0, 0.01, 100000.0, 0.000001] {
        camera.zoom_at(cursor, factor)?;
        let after = camera.screen_to_world(cursor);
        assert!((before[0] - after[0]).abs() < 1e-7);
        assert!((before[1] - after[1]).abs() < 1e-7);
    }
    Ok(())
}

#[test]
fn invalid_input_preserves_camera() {
    let mut camera = Camera::new([100, 100]);
    let before = camera.viewport();
    assert!(camera.zoom_at([f64::NAN, 0.0], 2.0).is_err());
    assert!(camera.pan([f64::INFINITY, 0.0]).is_err());
    assert!(camera.set_view([0.0, 0.0], -1.0).is_err());
    assert_eq!(before, camera.viewport());
}

#[test]
fn touching_edges_are_culled() -> Result<(), Box<dyn std::error::Error>> {
    let view = WorldRect::new(0.0, 0.0, 100.0, 100.0)?;
    assert!(!view.intersects(WorldRect::new(100.0, 0.0, 10.0, 10.0)?));
    assert!(view.intersects(WorldRect::new(99.0, 99.0, 10.0, 10.0)?));
    assert!(WorldRect::new(0.0, 0.0, f64::INFINITY, 2.0).is_err());
    Ok(())
}

#[test]
fn cache_touch_controls_eviction_and_oversize_is_non_destructive() {
    let mut cache = ByteCache::new(10);
    assert!(cache.insert(1, "one", 4));
    assert!(cache.insert(2, "two", 4));
    assert_eq!(cache.get(1), Some(&"one"));
    assert!(!cache.insert(3, "too large", 11));
    assert_eq!(cache.used_bytes(), 8);
    assert!(cache.insert(3, "three", 4));
    assert!(cache.contains(1));
    assert!(!cache.contains(2));
    assert_eq!(cache.evictions(), 1);
    assert!(cache.insert(1, "replacement", 8));
    assert_eq!(cache.used_bytes(), 8);
    assert_eq!(cache.get(1), Some(&"replacement"));
}

#[test]
fn lod_tracks_projected_size_not_original_size() {
    assert_eq!(Lod::for_projected_edge(40.0), Lod::Thumbnail);
    assert_eq!(Lod::for_projected_edge(128.0), Lod::Thumbnail);
    assert_eq!(Lod::for_projected_edge(129.0), Lod::Medium);
    assert_eq!(Lod::for_projected_edge(513.0), Lod::Detail);
}
