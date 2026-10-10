//! Explicit, bounded HTTPS requests. This process does not exist at app idle.
use std::{
    io::{Read, Write},
    path::Path,
    time::Duration,
};
use tack_update::{Channel, Error, Manifest, Offer, REPOSITORY, Response};
fn agent(seconds: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(3)
        .timeout_global(Some(Duration::from_secs(seconds)))
        .timeout_connect(Some(Duration::from_secs(8)))
        .build()
        .into()
}
fn bytes(url: &str, limit: u64) -> Result<Vec<u8>, Error> {
    let mut response = agent(20)
        .get(url)
        .header("User-Agent", "Tack/manual-updater")
        .call()?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("Update response size limit".into());
    }
    Ok(bytes)
}
pub fn download(url: &str, path: &Path, limit: u64) -> Result<(), Error> {
    let mut response = agent(120)
        .get(url)
        .header("User-Agent", "Tack/manual-updater")
        .call()?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let copied = std::io::copy(
        &mut response.body_mut().as_reader().take(limit + 1),
        &mut file,
    )?;
    if copied != limit {
        return Err("Downloaded update size mismatch".into());
    }
    file.flush()?;
    file.sync_all()?;
    Ok(())
}
pub fn check(channel: Channel, current: &str) -> Result<Response, Error> {
    let current = semver_parse(current)?;
    // At most four bounded pages. A full final page is an explicit incomplete
    // search, never an incorrect "up to date" response for an older Stable.
    let mut candidates = Vec::new();
    let mut complete = false;
    for page in 1..=4 {
        let list = bytes(
            &format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=30&page={page}"),
            512 * 1024,
        )?;
        let releases: Vec<serde_json::Value> = serde_json::from_slice(&list)?;
        if releases.len() > 30 {
            return Err("Release response count exceeded".into());
        }
        complete = releases.len() < 30;
        for r in releases {
            if r.get("draft").and_then(|v| v.as_bool()) != Some(false) {
                continue;
            }
            let Some(tag) = r
                .get("tag_name")
                .and_then(|v| v.as_str())
                .and_then(|t| t.strip_prefix('v'))
            else {
                continue;
            };
            let Ok(version) = semver_parse(tag) else {
                continue;
            };
            if !channel.accepts(&version) || version <= current {
                continue;
            }
            if r.get("prerelease").and_then(|v| v.as_bool()) != Some(channel == Channel::Dev) {
                continue;
            }
            let expected = format!(
                "https://github.com/{REPOSITORY}/releases/download/v{tag}/update-manifest.json"
            );
            if !r.get("assets").and_then(|v| v.as_array()).is_some_and(|a| {
                a.iter().any(|v| {
                    v.get("name").and_then(|v| v.as_str()) == Some("update-manifest.json")
                        && v.get("browser_download_url").and_then(|v| v.as_str())
                            == Some(expected.as_str())
                })
            }) {
                continue;
            }
            candidates.push((
                version,
                expected,
                r.get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(640)
                    .filter(|c| !c.is_control() || *c == '\n')
                    .collect::<String>(),
            ));
        }
        if complete {
            break;
        }
    }
    if !complete {
        return Err("Release search exceeded 120 entries; download manually".into());
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let Some((version, url, notes)) = candidates.into_iter().next() else {
        return Ok(Response::Current);
    };
    let manifest = Manifest::parse(&bytes(&url, tack_update::MAX_MANIFEST as u64)?)?;
    if manifest.channel != channel || manifest.version != version.to_string() {
        return Err("Release/manifest identity mismatch".into());
    }
    manifest.asset(tack_update::platform()?)?;
    Ok(Response::Available(Offer { manifest, notes }))
}
fn semver_parse(s: &str) -> Result<semver::Version, Error> {
    if s.len() > 64 {
        return Err("Version too long".into());
    }
    Ok(semver::Version::parse(s)?)
}
