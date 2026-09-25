//! Thin `DepotDownloader` wrapper for `cohlib backfill`: builds the CLI
//! invocation and the regex `-filelist` DepotDownloader reads, and runs it as
//! a subprocess. Argument/filelist construction is kept pure (no process
//! spawning) so it can be unit tested without Steam access — this whole area
//! is genuinely untested against real historical manifests (see the crate's
//! `backfill` module doc comment), so keeping the untestable part (the actual
//! network fetch) as thin as possible matters.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DepotDownloaderConfig {
    /// Path to the `DepotDownloader` binary, or a bare name resolved via `PATH`.
    pub binary: PathBuf,
    pub app: u32,
    pub username: Option<String>,
}

/// Builds the argument list for one `DepotDownloader` invocation: fetch
/// `depot`'s manifest (or the branch's current manifest, if `manifest` is
/// `None`) into `dir`, restricted to the paths matched by `filelist`.
pub fn build_args(
    config: &DepotDownloaderConfig,
    depot: u32,
    manifest: Option<u64>,
    dir: &Path,
    filelist: &Path,
) -> Vec<String> {
    let mut args = vec![
        "-app".to_string(),
        config.app.to_string(),
        "-depot".to_string(),
        depot.to_string(),
    ];
    if let Some(m) = manifest {
        args.push("-manifest".to_string());
        args.push(m.to_string());
    }
    args.push("-dir".to_string());
    args.push(dir.display().to_string());
    args.push("-filelist".to_string());
    args.push(filelist.display().to_string());
    if let Some(username) = &config.username {
        args.push("-username".to_string());
        args.push(username.clone());
        args.push("-remember-password".to_string());
    }
    args
}

/// One `regex:` pattern per line, in DepotDownloader's `-filelist` format.
/// Case-insensitive (`(?i)`): `RelicGame.module`'s `syncChecked` sections use
/// Windows casing (e.g. `Reflect`) that doesn't always match the depot's
/// actual (often lowercased) path casing — `checksums::resolve_sga` hits the
/// same mismatch reading a local depot and works around it the same way.
///
/// `sync_sections` is `RelicGame.module`'s parsed `category -> [(root, name)]`
/// map (see [`crate::checksums::parse_module_text`]) — every archive it lists
/// as `syncChecked` feeds the dataChecksum computation, so all of them must be
/// present for a build's checksum to be reproducible. `extra` is every other
/// file cohlib's own extraction pipeline reads directly, which isn't
/// necessarily the same archive set (e.g. `ReferenceAttributes.sga`, the file
/// `attrib::extract_game_data` actually parses, is a different archive from
/// the `syncChecked` `attrib` category's `Attrib.sga`).
pub fn build_filelist(
    sync_sections: &HashMap<String, Vec<(String, String)>>,
    extra: &[&str],
) -> Vec<String> {
    let mut patterns = Vec::new();
    let mut seen = HashSet::new();

    let mut push = |path: String, patterns: &mut Vec<String>| {
        let pattern = format!("regex:(?i)^{}$", regex_escape(&path));
        if seen.insert(pattern.clone()) {
            patterns.push(pattern);
        }
    };

    for archives in sync_sections.values() {
        for (root, name) in archives {
            push(
                format!("{}/{name}.sga", root.replace('\\', "/")),
                &mut patterns,
            );
        }
    }
    for path in extra {
        push((*path).to_string(), &mut patterns);
    }

    patterns
}

/// Writes `patterns` one per line, the format DepotDownloader's `-filelist`
/// expects.
pub fn write_filelist(path: &Path, patterns: &[String]) -> std::io::Result<()> {
    std::fs::write(path, patterns.join("\n"))
}

/// Escapes regex metacharacters in a literal path so it matches only itself
/// (aside from the deliberate `(?i)` case-insensitivity `build_filelist` adds).
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if ".\\+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Runs `DepotDownloader` for one depot, blocking until it exits.
pub fn fetch(
    config: &DepotDownloaderConfig,
    depot: u32,
    manifest: Option<u64>,
    dir: &Path,
    filelist: &Path,
) -> Result<(), String> {
    let args = build_args(config, depot, manifest, dir, filelist);
    eprintln!("Running: {} {}", config.binary.display(), args.join(" "));
    let status = Command::new(&config.binary)
        .args(&args)
        .status()
        .map_err(|e| format!("failed to run {}: {e}", config.binary.display()))?;
    if !status.success() {
        return Err(format!(
            "{} exited with {status} (depot {depot})",
            config.binary.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> DepotDownloaderConfig {
        DepotDownloaderConfig {
            binary: PathBuf::from("DepotDownloader"),
            app: 1677280,
            username: None,
        }
    }

    #[test]
    fn build_args_includes_manifest_when_given() {
        let args = build_args(
            &config(),
            1677281,
            Some(123456789),
            Path::new("/tmp/work"),
            Path::new("/tmp/filelist.txt"),
        );
        assert_eq!(
            args,
            vec![
                "-app",
                "1677280",
                "-depot",
                "1677281",
                "-manifest",
                "123456789",
                "-dir",
                "/tmp/work",
                "-filelist",
                "/tmp/filelist.txt",
            ]
        );
    }

    #[test]
    fn build_args_omits_manifest_flag_when_none() {
        let args = build_args(
            &config(),
            1677281,
            None,
            Path::new("/tmp/work"),
            Path::new("/tmp/filelist.txt"),
        );
        assert!(!args.contains(&"-manifest".to_string()));
    }

    #[test]
    fn build_args_includes_username_and_remember_password() {
        let mut c = config();
        c.username = Some("ry_9".to_string());
        let args = build_args(&c, 1677281, None, Path::new("/tmp"), Path::new("/tmp/f"));
        assert!(args.windows(2).any(|w| w == ["-username", "ry_9"]));
        assert!(args.contains(&"-remember-password".to_string()));
    }

    #[test]
    fn build_filelist_escapes_dots_and_dedupes() {
        let mut sections = HashMap::new();
        sections.insert(
            "attrib".to_string(),
            vec![("anvil\\archives".to_string(), "Attrib".to_string())],
        );
        sections.insert(
            "data".to_string(),
            vec![
                ("engine\\archives".to_string(), "Data".to_string()),
                ("engine\\archives".to_string(), "UI".to_string()),
                // Duplicate section (RelicGame.module lists "data" 3x) must not
                // produce duplicate filelist lines.
                ("engine\\archives".to_string(), "Data".to_string()),
            ],
        );
        let patterns = build_filelist(
            &sections,
            &["anvil/archives/ReferenceAttributes.sga", "RelicCoH3.exe"],
        );

        assert!(patterns.contains(&"regex:(?i)^anvil/archives/Attrib\\.sga$".to_string()));
        assert!(patterns.contains(&"regex:(?i)^engine/archives/Data\\.sga$".to_string()));
        assert!(patterns.contains(&"regex:(?i)^engine/archives/UI\\.sga$".to_string()));
        assert!(
            patterns.contains(&"regex:(?i)^anvil/archives/ReferenceAttributes\\.sga$".to_string())
        );
        assert!(patterns.contains(&"regex:(?i)^RelicCoH3\\.exe$".to_string()));

        // Exactly 4 unique patterns: Attrib, Data, UI, ReferenceAttributes, RelicCoH3.exe = 5.
        assert_eq!(patterns.len(), 5);
    }

    #[test]
    fn regex_escape_neutralizes_metacharacters() {
        assert_eq!(regex_escape("a.b"), "a\\.b");
        assert_eq!(regex_escape("a+b(c)"), "a\\+b\\(c\\)");
    }
}
