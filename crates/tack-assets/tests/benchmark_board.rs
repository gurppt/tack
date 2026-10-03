use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};
use tack_assets::{AssetError, BenchmarkBoard};
use tack_core::WorldRect;

#[test]
fn manifest_identity_order_geometry_and_path_containment_are_preserved() -> Result<(), AssetError> {
    let root = std::env::temp_dir().join(format!(
        "tack-board-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(&root)?;
    let result = (|| -> Result<(), AssetError> {
        fs::write(root.join("source.jpg"), b"manifest does not decode")?;
        let object = |id: u32, x: f64| serde_json::json!({"id":id,"path":"source.jpg","source_sha256":"a".repeat(64),"x":x,"y":0.0,"width":10.0,"height":10.0});
        let mut manifest = serde_json::json!({"schema":1,"objects":[object(7, 0.0),object(2, 10.0),object(9, 5.0)]});
        let path = root.join("manifest.json");
        fs::write(&path, serde_json::to_vec(&manifest)?)?;
        let board = BenchmarkBoard::read_manifest(&path)?;
        let visible: Vec<_> = board
            .visible(WorldRect::new(0.0, 0.0, 10.0, 10.0)?)
            .map(|o| o.id)
            .collect();
        assert_eq!(visible, vec![7, 9]);
        assert_eq!(board.objects[1].id, 2);
        assert_eq!(
            board.objects[0].path,
            root.join("source.jpg").canonicalize()?
        );
        manifest["objects"][1]["id"] = 7.into();
        fs::write(&path, serde_json::to_vec(&manifest)?)?;
        assert!(BenchmarkBoard::read_manifest(&path).is_err());
        manifest["objects"][1]["id"] = 2.into();
        manifest["objects"][0]["path"] = "/etc/passwd".into();
        fs::write(&path, serde_json::to_vec(&manifest)?)?;
        assert!(BenchmarkBoard::read_manifest(&path).is_err());
        Ok(())
    })();
    fs::remove_dir_all(root)?;
    result
}
