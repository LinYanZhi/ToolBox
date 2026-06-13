use std::path::PathBuf;

static RESOLVER: config::PathResolver = config::PathResolver::aminos();

pub fn source_dir() -> PathBuf {
    RESOLVER.source_dir()
}

pub fn builds_dir() -> PathBuf {
    RESOLVER.appdata_root().join("builds")
}

pub fn downloads_dir() -> PathBuf {
    RESOLVER.downloads_dir()
}

pub fn apps_dir() -> PathBuf {
    RESOLVER.apps_dir()
}

pub fn installed_json() -> PathBuf {
    RESOLVER.installed_json()
}
