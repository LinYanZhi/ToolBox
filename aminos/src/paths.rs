use std::path::{Path,  PathBuf};

/// ── Source directory lookup ────────────────────────────
///
/// Priority (first match wins):
/// 1. as.exe sibling `source/`           — portable USB mode
/// 2. `%LOCALAPPDATA%\aminos\source\`    — standard install (auto-downloaded)
/// 3. `%AMINOS_SOURCE%` env var          — manual override
/// 4. Walk up from exe dir (dev mode)    — development fallback

pub fn source_dir() -> PathBuf {
    // 1. Portable: as.exe sibling `source/`
    if let Some(dir) = portable_source() {
        return dir;
    }

    // 2. User data dir: %LOCALAPPDATA%\aminos\source\
    let appdata = appdata_source();
    if appdata.is_dir() {
        return appdata;
    }

    // 3. Env var: %AMINOS_SOURCE%
    if let Ok(env) = std::env::var("AMINOS_SOURCE") {
        let p = PathBuf::from(env);
        if p.is_dir() {
            return p;
        }
    }

    // 4. Walk up from exe (dev mode)
    if let Some(dir) = walk_up_source(std::env::current_exe().ok().as_deref()) {
        return dir;
    }

    // Fallback: appdata path (will be created by `as source update`)
    appdata_source()
}

/// `as.exe` 同级目录下的 `source/`
fn portable_source() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join("source");
    if dir.is_dir() {
        return Some(dir);
    }
    None
}

/// `%LOCALAPPDATA%\aminos\source\`
fn appdata_source() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    local.join("aminos").join("source")
}

/// Walk up from the given path looking for `source/`
fn walk_up_source(from: Option<&Path>) -> Option<PathBuf> {
    let mut dir: &Path = from?;
    for _ in 0..8 {
        if dir.join("source").is_dir() {
            return Some(dir.join("source"));
        }
        dir = dir.parent()?;
    }
    None
}

/// ── Data directories ───────────────────────────────────

pub fn apps_dir() -> PathBuf {
    appdata_source()
        .parent()
        .map(|p| p.join("apps"))
        .unwrap_or_else(|| PathBuf::from("apps"))
}

pub fn builds_dir() -> PathBuf {
    appdata_source()
        .parent()
        .map(|p| p.join("downloads"))
        .unwrap_or_else(|| PathBuf::from("downloads"))
}

pub fn downloads_dir() -> PathBuf {
    builds_dir()
}

pub fn installed_json() -> PathBuf {
    apps_dir().join("installed.json")
}
