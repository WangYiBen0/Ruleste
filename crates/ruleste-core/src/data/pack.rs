//! Pack metadata (`metadata.json`).
//!
//! The converted resource tree is organized as `resources/<pack>/<namespace>/`.
//! A pack is a self-describing, installable unit: its `metadata.json` declares
//! identity and rendering facts plus the chapters the main menu offers. Schema
//! (documented in README §resources):
//!
//! ```json
//! {
//!   "pack":       "Celeste",
//!   "namespace":  "Celeste",
//!   "title":      "Celeste",
//!   "version":    "1.0.0",
//!   "author":     "",
//!   "homepage":   "",
//!   "description":"",
//!   "chapters": [
//!     { "id": "0-Intro", "name": "Prologue",
//!       "map": "maps/Celeste/0-Intro.bin",
//!       "song": "music_lvl0_dream" }
//!   ]
//! }
//! ```
//!
//! Every field except `pack`/`namespace` is optional; missing chapters fall
//! back to a scan of the pack's `maps/` directory at load time (`scan`).

use std::path::{Path, PathBuf};

use anyhow::Context;

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PackMeta {
    /// Unique pack id (also the first path component under `resources/`).
    #[serde(default)]
    pub pack: String,
    /// Namespace subdirectory under the pack (also usually the pack id).
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub chapters: Vec<ChapterMeta>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ChapterMeta {
    /// Stable chapter id (e.g. `0-Intro`). Used as the game's room/slot key.
    #[serde(default)]
    pub id: String,
    /// Human-readable chapter name (e.g. `Prologue`).
    #[serde(default)]
    pub name: String,
    /// Map file, either absolute or relative to the workspace root.
    #[serde(default)]
    pub map: PathBuf,
    /// Default music stream name from the audio manifest.
    #[serde(default)]
    pub song: String,
}

impl PackMeta {
    /// Path of the opened metadata file (needed by the authoring helpers).
    pub const FILE_NAME: &'static str = "metadata.json";

    /// Loads `<resources_root>/<pack>/<namespace>/metadata.json`.
    pub fn load(resources_root: &Path, pack: &str, namespace: &str) -> anyhow::Result<PackMeta> {
        let path = resources_root
            .join(pack)
            .join(namespace)
            .join(Self::FILE_NAME);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut meta: PackMeta =
            serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        if meta.pack.is_empty() {
            meta.pack = pack.to_string();
        }
        if meta.namespace.is_empty() {
            meta.namespace = namespace.to_string();
        }
        if meta.title.is_empty() {
            meta.title = pack.to_string();
        }
        if meta.chapters.is_empty() {
            meta.chapters = scan_maps(pack);
        }
        Ok(meta)
    }

    /// Scans every `resources/<pack>/<namespace>/metadata.json`.
    pub fn scan_all(resources_root: &Path) -> Vec<PackMeta> {
        let mut out = Vec::new();
        let Ok(packs) = std::fs::read_dir(resources_root) else {
            return out;
        };
        for pack in packs.flatten() {
            if !pack.path().is_dir() {
                continue;
            }
            let pack = pack.file_name().to_string_lossy().into_owned();
            let Ok(namespaces) = std::fs::read_dir(resources_root.join(&pack)) else {
                continue;
            };
            for ns in namespaces.flatten() {
                if !ns.path().is_dir() {
                    continue;
                }
                let ns = ns.file_name().to_string_lossy().into_owned();
                if let Ok(meta) = PackMeta::load(resources_root, &pack, &ns) {
                    out.push(meta);
                }
            }
        }
        out.sort_by(|a, b| a.title.cmp(&b.title));
        out
    }
}

/// Fills `chapters` from the pack's `maps/` directory (`maps/<pack>/*.bin`).
fn scan_maps(pack: &str) -> Vec<ChapterMeta> {
    let maps = PathBuf::from("maps").join(pack);
    let Ok(entries) = std::fs::read_dir(&maps) else {
        return Vec::new();
    };
    let mut chapters: Vec<ChapterMeta> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|e| e == "bin"))
        .map(|e| ChapterMeta {
            id: e
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string(),
            name: String::new(),
            map: e.path(),
            song: String::new(),
        })
        .collect();
    chapters.sort_by(|a, b| a.id.cmp(&b.id));
    chapters
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn parses_schema_with_defaults() {
        let dir = std::env::temp_dir().join(format!("ruleste_pack_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(
            &dir.join("MyPack/MyNS/metadata.json"),
            r#"{
                "pack": "MyPack",
                "namespace": "MyNS",
                "title": "My Pack",
                "version": "1.2.3",
                "chapters": [
                    { "id": "1-A", "name": "First", "map": "maps/MyPack/1-A.bin", "song": "music_lvl1" }
                ]
            }"#,
        );
        let m = PackMeta::load(&dir, "MyPack", "MyNS").unwrap();
        assert_eq!(m.pack, "MyPack");
        assert_eq!(m.namespace, "MyNS");
        assert_eq!(m.title, "My Pack");
        assert_eq!(m.version, "1.2.3");
        assert_eq!(m.chapters.len(), 1);
        assert_eq!(m.chapters[0].song, "music_lvl1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_fields_rejected() {
        let dir = std::env::temp_dir().join(format!("ruleste_pack_bad_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(
            &dir.join("P/N/metadata.json"),
            r#"{ "pack": "P", "namespace": "N", "wat": true }"#,
        );
        assert!(PackMeta::load(&dir, "P", "N").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_all_finds_packs() {
        let dir = std::env::temp_dir().join(format!("ruleste_scan_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(
            &dir.join("A/Aa/metadata.json"),
            r#"{ "pack":"A", "namespace":"Aa" }"#,
        );
        write(
            &dir.join("B/Bb/metadata.json"),
            r#"{ "pack":"B", "namespace":"Bb" }"#,
        );
        std::fs::write(dir.join("B/no-json.txt"), "").unwrap();
        let packs = PackMeta::scan_all(&dir);
        assert_eq!(packs.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
