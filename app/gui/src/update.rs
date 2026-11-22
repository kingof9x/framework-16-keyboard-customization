//! Looks on GitHub for a newer release of the app, and downloads its installer.
//! Uses GitHub's public API: no sign-in needed, the repository is public.

// Off Windows only the parsing is used (by the tests).
#![cfg_attr(not(windows), allow(dead_code))]

#[cfg(windows)]
use std::io::Read;
use std::path::PathBuf;
#[cfg(windows)]
use std::time::Duration;

const LATEST: &str =
    "https://api.github.com/repos/kingof9x/framework-16-keyboard-customization/releases/latest";
pub const RELEASES_PAGE: &str =
    "https://github.com/kingof9x/framework-16-keyboard-customization/releases";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// The installer's file name in a release ends with this.
const INSTALLER_SUFFIX: &str = "-setup.exe";

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub page: String,
    pub installer: Option<String>,
}

/// "v1.2.3" or "1.2.3" -> (1, 2, 3)
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut p = s
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+']);
    Some((
        p.next()?.parse().ok()?,
        p.next().unwrap_or("0").parse().ok()?,
        p.next().unwrap_or("0").parse().ok()?,
    ))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// Every string value of `"key": "..."` in a JSON text (enough for GitHub's release answer).
fn json_strings(body: &str, key: &str) -> Vec<String> {
    let pat = format!("\"{key}\"");
    let mut out = vec![];
    let mut rest = body;
    while let Some(i) = rest.find(&pat) {
        rest = &rest[i + pat.len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix(':') else {
            continue;
        };
        let after = after.trim_start();
        let Some(after) = after.strip_prefix('"') else {
            continue;
        };
        let mut value = String::new();
        let mut chars = after.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(e) = chars.next() {
                        value.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                    }
                }
                c => value.push(c),
            }
        }
        out.push(value);
    }
    out
}

pub fn parse_release(body: &str) -> Option<Release> {
    let version = json_strings(body, "tag_name").into_iter().next()?;
    let page = json_strings(body, "html_url")
        .into_iter()
        .find(|u| u.contains("/releases/"))
        .unwrap_or_else(|| RELEASES_PAGE.to_string());
    let installer = json_strings(body, "browser_download_url")
        .into_iter()
        .find(|u| u.ends_with(INSTALLER_SUFFIX));
    Some(Release {
        version,
        page,
        installer,
    })
}

#[cfg(windows)]
fn agent() -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| format!("No secure connection: {e}"))?;
    Ok(ureq::AgentBuilder::new()
        .tls_connector(std::sync::Arc::new(tls))
        .timeout(Duration::from_secs(60))
        .user_agent(concat!(
            "framework-16-keyboard-control/",
            env!("CARGO_PKG_VERSION")
        ))
        .build())
}

/// The newest release, if it is newer than this app.
#[cfg(windows)]
pub fn check() -> Result<Option<Release>, String> {
    let body = agent()?
        .get(LATEST)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "No releases published yet.".to_string(),
            e => format!("Could not reach GitHub: {e}"),
        })?
        .into_string()
        .map_err(|e| format!("Could not read GitHub's answer: {e}"))?;
    let release = parse_release(&body).ok_or("GitHub's answer had no version in it.")?;
    Ok(is_newer(&release.version, CURRENT).then_some(release))
}

/// The download link of the release file called `file`, if the release has one.
pub fn asset_url(body: &str, file: &str) -> Option<String> {
    let end = format!("/{file}");
    json_strings(body, "browser_download_url")
        .into_iter()
        .find(|u| u.ends_with(&end))
}

#[cfg(windows)]
fn fetch(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let resp = agent()?
        .get(url)
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;
    let mut data = Vec::new();
    resp.into_reader()
        .take(limit)
        .read_to_end(&mut data)
        .map_err(|e| format!("Download failed: {e}"))?;
    Ok(data)
}

/// The newest release's version and its file called `file` (for example a device's firmware).
#[cfg(windows)]
pub fn latest_asset(file: &str) -> Result<(String, Vec<u8>), String> {
    let body = agent()?
        .get(LATEST)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "No releases published yet.".to_string(),
            e => format!("Could not reach GitHub: {e}"),
        })?
        .into_string()
        .map_err(|e| format!("Could not read GitHub's answer: {e}"))?;
    let release = parse_release(&body).ok_or("GitHub's answer had no version in it.")?;
    let url = asset_url(&body, file)
        .ok_or_else(|| format!("Release {} has no {file} in it.", release.version))?;
    Ok((release.version, fetch(&url, 16 * 1024 * 1024)?))
}

#[cfg(not(windows))]
pub fn latest_asset(_file: &str) -> Result<(String, Vec<u8>), String> {
    Err("Firmware is downloaded in the Windows app.".into())
}

/// Downloads the installer into the temp folder and returns where it is.
#[cfg(windows)]
pub fn download(url: &str) -> Result<PathBuf, String> {
    let data = fetch(url, 200 * 1024 * 1024)?;
    let name = url
        .rsplit('/')
        .next()
        .unwrap_or("framework-16-keyboard-control-setup.exe");
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, data).map_err(|e| format!("Could not save the installer: {e}"))?;
    Ok(path)
}

#[cfg(not(windows))]
pub fn check() -> Result<Option<Release>, String> {
    Err("Updates are checked in the Windows app.".into())
}

#[cfg(not(windows))]
pub fn download(_url: &str) -> Result<PathBuf, String> {
    Err("Updates are installed in the Windows app.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("0.10"), Some((0, 10, 0)));
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }

    #[test]
    fn reads_githubs_answer() {
        let body = r#"{"url":"x","html_url": "https://github.com/k/r/releases/tag/v0.2.0",
            "tag_name": "v0.2.0","assets":[
            {"name":"framework_ansi_f16custom.uf2","browser_download_url":"https://e/a.uf2"},
            {"name":"setup","browser_download_url":"https://e/framework-16-keyboard-control-0.2.0-setup.exe"}],
            "body":"Fixes \"quotes\""}"#;
        let r = parse_release(body).unwrap();
        assert_eq!(r.version, "v0.2.0");
        assert_eq!(r.page, "https://github.com/k/r/releases/tag/v0.2.0");
        assert_eq!(
            r.installer.as_deref(),
            Some("https://e/framework-16-keyboard-control-0.2.0-setup.exe")
        );
        assert_eq!(
            asset_url(body, "framework_ansi_f16custom.uf2"),
            None,
            "only a link that ends with the exact file name counts"
        );
        let body = body.replace("a.uf2", "framework_ansi_f16custom.uf2");
        assert_eq!(
            asset_url(&body, "framework_ansi_f16custom.uf2").as_deref(),
            Some("https://e/framework_ansi_f16custom.uf2")
        );
    }
}
