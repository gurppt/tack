use tack_update::*;
fn sample() -> Manifest {
    Manifest {
        schema: 1,
        version: "0.1.0-dev.10".into(),
        channel: Channel::Dev,
        git_sha: "a".repeat(40),
        protocol_major: PROTOCOL_MAJOR,
        assets: vec![Asset {
            platform: "linux-x86_64".into(),
            name: "tack-linux-x86_64.zip".into(),
            size: 123,
            sha256: "b".repeat(64),
        }],
    }
}
#[test]
fn identity_and_semver() -> Result<(), Error> {
    let m = sample();
    m.validate()?;
    assert!(m.newer_than("0.1.0-dev.2")?);
    assert!(!m.newer_than("0.1.0")?);
    assert!(!m.newer_than("0.1.0-dev.10")?);
    assert_eq!(Manifest::parse(&serde_json::to_vec(&m)?)?, m);
    assert!(
        m.url("update-manifest.json")
            .starts_with("https://github.com/gurppt/tack/releases/download/v0.1.0-dev.10/")
    );
    Ok(())
}
#[test]
fn malformed_source_and_schema_are_refused() {
    for change in 0..7 {
        let mut m = sample();
        match change {
            0 => m.schema = 2,
            1 => m.protocol_major += 1,
            2 => m.channel = Channel::Stable,
            3 => m.assets[0].name = "../tack.zip".into(),
            4 => m.assets[0].size = MAX_ARCHIVE + 1,
            5 => m.assets.push(m.assets[0].clone()),
            _ => m.git_sha = "bad".into(),
        };
        assert!(m.validate().is_err());
    }
    assert!(Manifest::parse(&vec![b' '; MAX_MANIFEST + 1]).is_err());
    assert!(Manifest::parse(br#"{"schema":1,"unknown":true}"#).is_err());
}
#[test]
fn stable_channel_does_not_accept_prerelease() -> Result<(), Error> {
    let mut m = sample();
    m.version = "0.1.0".into();
    m.channel = Channel::Stable;
    m.validate()?;
    m.version = "0.1.0-alpha.1".into();
    assert!(m.validate().is_err());
    Ok(())
}
