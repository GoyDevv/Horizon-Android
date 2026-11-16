//! In-app GitHub release update detection and installer for Android.
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub const CURRENT_VERSION: &str = "1.0.1";
pub const REPO: &str = "GoyDevv/Horizon-Android";

#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub title: String,
    pub changelog: String,
    pub download_url: Option<String>,
    pub asset_name: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    Available(ReleaseInfo),
    Downloading {
        progress: f32,
        downloaded: u64,
        total: u64,
    },
    Downloaded {
        path: PathBuf,
        version: String,
    },
    Error(String),
    Dismissed,
}

static STATUS: Mutex<Option<UpdateStatus>> = Mutex::new(None);
static CHECKING: AtomicBool = AtomicBool::new(false);
static DOWNLOADING: AtomicBool = AtomicBool::new(false);

pub fn get_status() -> UpdateStatus {
    STATUS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or(UpdateStatus::Idle)
}

pub fn set_status(new_status: UpdateStatus) {
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = Some(new_status);
}

pub fn dismiss() {
    set_status(UpdateStatus::Dismissed);
}

/// Parses version string into numeric tuple (major, minor, patch, prerelease).
pub fn parse_version(raw: &str) -> (u32, u32, u32, u32) {
    let clean = raw.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = clean.split(&['.', '-', '+'][..]);
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let build = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch, build)
}

/// Returns true if `remote` is strictly higher than `current`.
pub fn is_newer_version(remote: &str, current: &str) -> bool {
    parse_version(remote) > parse_version(current)
}

/// Fetches the latest release from the repository and parses release metadata.
pub fn check_for_update() -> Result<Option<ReleaseInfo>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = ureq::get(&url)
        .set("User-Agent", &format!("Horizon-Android/{CURRENT_VERSION}"))
        .set("Accept", "application/vnd.github.v3+json")
        .timeout(std::time::Duration::from_secs(12))
        .call()
        .map_err(|e| format!("Could not reach GitHub Releases: {e}"))?;

    let val: Value = resp
        .into_json()
        .map_err(|e| format!("Invalid release JSON response: {e}"))?;

    let tag = val["tag_name"].as_str().unwrap_or("").trim().to_owned();
    let name = val["name"].as_str().unwrap_or(&tag).trim().to_owned();
    let changelog = val["body"]
        .as_str()
        .map(str::trim)
        .filter(|body| !body.is_empty())
        .unwrap_or("This release did not include release notes.")
        .to_owned();

    let remote_ver = tag.trim_start_matches('v').trim_start_matches('V');
    if !is_newer_version(remote_ver, CURRENT_VERSION) {
        return Ok(None);
    }

    let mut download_url = None;
    let mut asset_name = format!("horizon-{tag}.apk");
    let mut size_bytes = 0u64;

    if let Some(assets) = val["assets"].as_array() {
        for asset in assets {
            let a_name = asset["name"].as_str().unwrap_or("");
            if a_name.ends_with(".apk") {
                asset_name = a_name.to_owned();
                download_url = asset["browser_download_url"].as_str().map(str::to_owned);
                size_bytes = asset["size"].as_u64().unwrap_or(0);
                break;
            }
        }
    }

    Ok(Some(ReleaseInfo {
        version: remote_ver.to_owned(),
        tag_name: tag,
        title: name,
        changelog,
        download_url,
        asset_name,
        size_bytes,
    }))
}

/// Initiates an update check on a background worker thread.
pub fn check_in_background() {
    if CHECKING.swap(true, Ordering::SeqCst) {
        return;
    }
    set_status(UpdateStatus::Checking);
    let spawned = std::thread::Builder::new()
        .name("UpdateChecker".into())
        .spawn(|| {
            match check_for_update() {
                Ok(Some(info)) => {
                    log::info!("New update available: {}", info.version);
                    set_status(UpdateStatus::Available(info));
                }
                Ok(None) => {
                    log::info!("Horizon is up to date (version {CURRENT_VERSION})");
                    set_status(UpdateStatus::Idle);
                }
                Err(err) => {
                    log::warn!("Update check failed: {err}");
                    set_status(UpdateStatus::Error(err));
                }
            }
            CHECKING.store(false, Ordering::SeqCst);
        });
    if let Err(error) = spawned {
        CHECKING.store(false, Ordering::SeqCst);
        set_status(UpdateStatus::Error(format!(
            "Could not start update check: {error}"
        )));
    }
}

/// Downloads the APK in the background; installation is an explicit user action.
pub fn download_and_install_in_background(info: ReleaseInfo, home: PathBuf) {
    if DOWNLOADING.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(url) = info.download_url.clone() else {
        set_status(UpdateStatus::Error("No APK asset found in release.".into()));
        DOWNLOADING.store(false, Ordering::SeqCst);
        return;
    };

    set_status(UpdateStatus::Downloading {
        progress: 0.0,
        downloaded: 0,
        total: info.size_bytes,
    });

    let failed_partial = Path::new(&info.asset_name).file_name().map(|name| {
        home.join("updates")
            .join(format!("{}.partial", name.to_string_lossy()))
    });
    let spawned = std::thread::Builder::new()
        .name("UpdateDownloader".into())
        .spawn(move || {
            let res = (|| -> Result<PathBuf, String> {
                let updates_dir = home.join("updates");
                std::fs::create_dir_all(&updates_dir).map_err(|e| e.to_string())?;
                let asset_name = Path::new(&info.asset_name)
                    .file_name()
                    .filter(|name| name.to_string_lossy().to_ascii_lowercase().ends_with(".apk"))
                    .ok_or("Release has an invalid APK asset name")?;
                let apk_path = updates_dir.join(asset_name);
                let partial_path = updates_dir.join(format!("{}.partial", asset_name.to_string_lossy()));

                let resp = ureq::get(&url)
                    .set("User-Agent", &format!("Horizon-Android/{CURRENT_VERSION}"))
                    .timeout(std::time::Duration::from_secs(60))
                    .call()
                    .map_err(|e| format!("Failed to download update: {e}"))?;

                let total_len = resp
                    .header("Content-Length")
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(info.size_bytes);

                let max_len = 512 * 1024 * 1024;
                if info.size_bytes > max_len {
                    return Err("Update APK exceeds the 512 MiB download limit.".into());
                }
                if total_len > max_len {
                    return Err("Update APK exceeds the 512 MiB download limit.".into());
                }
                let mut reader = resp.into_reader();
                let mut file = std::fs::File::create(&partial_path).map_err(|e| e.to_string())?;
                let mut buffer = [0u8; 65536];
                let mut downloaded = 0u64;

                loop {
                    use std::io::{Read, Write};
                    let n = reader
                        .read(&mut buffer)
                        .map_err(|e| format!("Download error: {e}"))?;
                    if n == 0 {
                        break;
                    }
                    if downloaded.saturating_add(n as u64) > max_len {
                        return Err("Update APK exceeded the 512 MiB download limit.".into());
                    }
                    file.write_all(&buffer[..n])
                        .map_err(|e| format!("Disk write error: {e}"))?;
                    downloaded += n as u64;

                    let progress = if total_len > 0 {
                        (downloaded as f32 / total_len as f32).clamp(0.0, 1.0)
                    } else {
                        0.5
                    };
                    set_status(UpdateStatus::Downloading {
                        progress,
                        downloaded,
                        total: total_len,
                    });
                }
                file.sync_all().map_err(|e| e.to_string())?;
                drop(file);
                if total_len > 0 && downloaded != total_len {
                    return Err(format!(
                        "Update download ended early ({downloaded} of {total_len} bytes)."
                    ));
                }
                if info.size_bytes > 0 && downloaded != info.size_bytes {
                    return Err(format!(
                        "Update size did not match release metadata (downloaded {downloaded}, expected {}).",
                        info.size_bytes
                    ));
                }
                std::fs::rename(&partial_path, &apk_path).map_err(|e| e.to_string())?;
                Ok(apk_path)
            })();

            match res {
                Ok(apk_path) => {
                    log::info!("Update downloaded to: {}", apk_path.display());
                    set_status(UpdateStatus::Downloaded {
                        path: apk_path.clone(),
                        version: info.version.clone(),
                    });
                }
                Err(err) => {
                    log::warn!("Download error: {err}");
                    if let Some(path) = failed_partial {
                        let _ = std::fs::remove_file(path);
                    }
                    set_status(UpdateStatus::Error(err));
                }
            }
            DOWNLOADING.store(false, Ordering::SeqCst);
        });
    if let Err(error) = spawned {
        DOWNLOADING.store(false, Ordering::SeqCst);
        set_status(UpdateStatus::Error(format!(
            "Could not start update download: {error}"
        )));
    }
}

/// Invokes Android package installer through JNI.
pub fn trigger_install(apk_path: &Path) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let result = crate::platform::with_context(|env, activity| {
            let path_str = apk_path.to_string_lossy().into_owned();
            let j_path = env.new_string(&path_str)?;
            let cls = env.get_object_class(activity)?;
            let response = env
                .call_static_method(
                    &cls,
                    "installApk",
                    "(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;",
                    &[
                        jni::objects::JValue::Object(activity),
                        jni::objects::JValue::Object(&j_path.into()),
                    ],
                )?
                .l()?;
            let response: String = env
                .get_string(&jni::objects::JString::from(response))?
                .into();
            Ok(response)
        });
        match result {
            Some(message) if message.is_empty() => Ok(()),
            Some(message) => Err(message),
            None => Err("Could not contact Android's package installer.".into()),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        log::info!("Triggering APK install on path: {}", apk_path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing_and_comparison() {
        assert_eq!(parse_version("1.0.0"), (1, 0, 0, 0));
        assert_eq!(parse_version("v1.0.1"), (1, 0, 1, 0));
        assert_eq!(parse_version("1.2.3-dev.4"), (1, 2, 3, 4));

        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(is_newer_version("v1.1.0", "1.0.0"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("0.9.9", "1.0.0"));
    }
}
