//! `audio/` manifest loader.
//!
//! The audio tree produced by `tools/bank-to-ogg.sh` keeps two manifests:
//! a top-level `audio/manifest` listing every FMOD bank, and one
//! `audio/<bank>/manifest` per bank listing its decoded streams. Streams are
//! addressed by their FMOD/FSB5 stream name (e.g. `game_gen_diamond_touch_01`)
//! — the exact identifiers the original game triggers as `event:/...` paths.
//! The `event:/...` -> stream mapping is a follow-up research item (see
//! ROADMAP): until then the audio bus is addressed directly by stream name.
//!
//! Format (`\t`-separated, `#` comments, ASCII):
//! ```text,ignore
//! # audio/manifest
//! <bank>  (bank)  <bank_dir>/
//! ```
//! ```text,ignore
//! # audio/<bank_dir>/manifest
//! <stream_name>  <bank>  <bank_dir>/<stream_name>.ogg
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;

/// One playable stream resolved from the manifests.
#[derive(Clone, Debug)]
pub struct SampleEntry {
    /// FMOD bank the stream was exported from (e.g. `sfx`).
    pub bank: String,
    /// Path relative to the audio root (e.g. `sfx/char_bad_appear.ogg`).
    pub file: PathBuf,
}

/// Stream-name index over all decoded banks.
#[derive(Default, Debug)]
pub struct AudioManifest {
    /// Bank name -> bank directory name (as listed in the top manifest).
    pub banks: Vec<String>,
    /// Stream name -> entry. Names are globally unique across banks; Celeste
    /// builds banks so a sound name never repeats.
    samples: Vec<SampleEntry>,
    index: HashMap<String, usize>,
}

impl AudioManifest {
    /// Loads `<audio_root>/manifest` and every bank sub-manifest it lists.
    pub fn load(audio_root: &Path) -> anyhow::Result<AudioManifest> {
        let top = audio_root.join("manifest");
        let text =
            std::fs::read_to_string(&top).with_context(|| format!("reading {}", top.display()))?;
        let mut manifest = AudioManifest::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 3 {
                continue;
            }
            let (bank, dir) = (cols[0], cols[2].trim_end_matches('/'));
            manifest.banks.push(bank.to_string());
            manifest.load_bank(audio_root, bank, dir)?;
        }
        Ok(manifest)
    }

    fn load_bank(&mut self, audio_root: &Path, bank: &str, dir: &str) -> anyhow::Result<()> {
        let manifest_path = audio_root.join(dir).join("manifest");
        let text = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 3 {
                continue;
            }
            let (sample, file) = (cols[0], cols[2]);
            let entry = SampleEntry {
                bank: bank.to_string(),
                file: PathBuf::from(file),
            };
            // First occurrence wins on duplicate names.
            self.index.entry(sample.to_string()).or_insert_with(|| {
                self.samples.push(entry);
                self.samples.len() - 1
            });
        }
        Ok(())
    }

    /// Number of indexed streams.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the manifest has no streams.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Looks up a stream by name.
    pub fn get(&self, name: &str) -> Option<&SampleEntry> {
        self.index.get(name).map(|&i| &self.samples[i])
    }

    /// Iterates `(stream name, entry)` in insertion order (manifest order).
    pub fn iter_entries(&self) -> impl Iterator<Item = (&str, &SampleEntry)> {
        self.index
            .iter()
            .map(move |(name, &i)| (name.as_str(), &self.samples[i]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn manifest_loads_banks_and_streams() {
        let dir =
            std::env::temp_dir().join(format!("ruleste_audio_manifest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(
            &dir.join("manifest"),
            "# Ruleste audio manifest\nsfx\t(bank)\tsfx/\nmusic\t(bank)\tmusic/\n",
        );
        write(
            &dir.join("sfx").join("manifest"),
            "# bank=sfx\nchar_bad_appear\tsfx\tsfx/char_bad_appear.ogg\nfoo\tsfx\tsfx/foo.ogg\n",
        );
        write(
            &dir.join("music").join("manifest"),
            "# bank=music\nmusic_lvl0_dream\tmusic\tmusic/music_lvl0_dream.ogg\n",
        );

        let m = AudioManifest::load(&dir).unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(m.banks, vec!["sfx", "music"]);
        let s = m.get("char_bad_appear").unwrap();
        assert_eq!(s.bank, "sfx");
        assert_eq!(s.file, PathBuf::from("sfx/char_bad_appear.ogg"));
        assert!(m.get("nope").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn manifest_skips_comments_and_duplicates() {
        let dir =
            std::env::temp_dir().join(format!("ruleste_audio_manifest_dup_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(
            &dir.join("manifest"),
            "sfx\t(bank)\tsfx/\n\n# trailing comment\n",
        );
        write(
            &dir.join("sfx").join("manifest"),
            "a\tsfx\tsfx/a.ogg\na\tsfx\tsfx/a.ogg\n",
        );
        let m = AudioManifest::load(&dir).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("a").unwrap().file, PathBuf::from("sfx/a.ogg"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
