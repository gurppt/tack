//! Controlled local archive: no release publication, download or user installation.
use crate::{apply, package, test_support::Temp, transaction};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
use tack_update::{Asset, Channel, Error, Manifest};

fn fixture(install: &Path) -> Result<(std::path::PathBuf, Manifest), Error> {
    let stage = install.join(".tack-update-0.1.0-dev.2");
    std::fs::create_dir(&stage)?;
    let archive = stage.join("package.zip");
    let f = std::fs::File::create(&archive)?;
    let mut zip = zip::ZipWriter::new(f);
    let record = serde_json::json!({"commit":"a".repeat(40),"version":"0.1.0-dev.2","channel":"dev","protocol_major":tack_update::PROTOCOL_MAJOR});
    for (name, bytes) in [
        (
            "tack",
            b"#!/bin/sh\nprintf new > update-launched.txt\n".as_slice(),
        ),
        ("tack-server", b"new server"),
        ("tack-updater", b"new helper"),
        ("tack-jpeg-decoder", b"new decoder"),
        ("tack-about.png", b"about"),
        ("tack-about-logo.png", b"logo"),
        ("gfx/icons/pointer.png", b"default icon"),
        ("BUILD.json", record.to_string().as_bytes()),
    ] {
        zip.start_file(
            format!("tack-linux-x86_64/{name}"),
            zip::write::SimpleFileOptions::default(),
        )?;
        zip.write_all(bytes)?;
    }
    zip.finish()?;
    let data = std::fs::read(&archive)?;
    let manifest = Manifest {
        schema: 1,
        version: "0.1.0-dev.2".into(),
        channel: Channel::Dev,
        git_sha: "a".repeat(40),
        protocol_major: tack_update::PROTOCOL_MAJOR,
        assets: vec![Asset {
            platform: "linux-x86_64".into(),
            name: "tack-linux-x86_64.zip".into(),
            size: data.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&data)),
        }],
    };
    std::fs::write(stage.join("manifest.json"), serde_json::to_vec(&manifest)?)?;
    std::fs::write(install.join("tack"), b"old executable")?;
    std::fs::write(install.join("artist.tack"), b"board")?;
    std::fs::write(install.join("preferences.json"), b"profile")?;
    std::fs::create_dir_all(install.join("gfx/icons"))?;
    std::fs::write(install.join("gfx/icons/pointer.png"), b"user pixels")?;
    Ok((stage, manifest))
}
fn exited_pid() -> Result<u32, Error> {
    let mut child = std::process::Command::new("true").spawn()?;
    let pid = child.id();
    child.wait()?;
    Ok(pid)
}
#[test]
fn verified_archive_is_reextracted_then_applied_and_personal_data_survives() -> Result<(), Error> {
    let root = Temp::new()?;
    let (stage, _) = fixture(&root.0)?;
    // Neither pre-extracted tampering nor an interrupted verification is trusted.
    std::fs::create_dir(stage.join("package"))?;
    std::fs::write(stage.join("package/tack"), b"tampered")?;
    std::fs::create_dir(stage.join("verified"))?;
    std::fs::write(stage.join("verified/tack"), b"interrupted")?;
    apply::apply(&stage, &root.0, exited_pid()?)?;
    let until = Instant::now() + Duration::from_secs(2);
    while !root.0.join("update-launched.txt").exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read(root.0.join("update-launched.txt"))?, b"new");
    assert_eq!(
        std::fs::read(root.0.join(".tack-previous/tack"))?,
        b"old executable"
    );
    assert_eq!(
        std::fs::read(root.0.join("gfx/icons/pointer.png"))?,
        b"user pixels"
    );
    assert_eq!(std::fs::read(root.0.join("artist.tack"))?, b"board");
    assert_eq!(std::fs::read(root.0.join("preferences.json"))?, b"profile");
    assert!(!stage.exists());
    assert!(!transaction::recover(&root.0)?);
    Ok(())
}
#[test]
fn bad_archive_hash_is_refused_before_installation_changes() -> Result<(), Error> {
    let root = Temp::new()?;
    let (stage, _) = fixture(&root.0)?;
    std::fs::write(stage.join("package.zip"), b"wrong bytes")?;
    assert!(apply::apply(&stage, &root.0, exited_pid()?).is_err());
    assert_eq!(std::fs::read(root.0.join("tack"))?, b"old executable");
    assert!(!root.0.join(".tack-previous").exists());
    assert!(!stage.join("verified").exists());
    Ok(())
}
#[test]
fn package_identity_mismatch_is_not_accepted() -> Result<(), Error> {
    let root = Temp::new()?;
    let (stage, mut manifest) = fixture(&root.0)?;
    let unpacked = stage.join("package");
    package::extract(&stage.join("package.zip"), &unpacked)?;
    manifest.git_sha = "b".repeat(40);
    assert!(package::identity(&unpacked, &manifest).is_err());
    Ok(())
}
