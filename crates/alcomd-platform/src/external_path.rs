use std::{borrow::Cow, path::Path};

/// Adapts an existing path for an external program or system UI.
/// Keep the original path for filesystem operations, identity, and storage.
#[must_use]
pub fn external_filesystem_path(path: &Path) -> Cow<'_, Path> {
    #[cfg(windows)]
    if let Some(ordinary) = path.to_str().and_then(ordinary_windows_spelling) {
        let ordinary = std::path::PathBuf::from(ordinary);
        if let Ok(resolved) = std::fs::canonicalize(&ordinary)
            && (resolved == path
                || std::fs::canonicalize(path).is_ok_and(|original| original == resolved))
        {
            return Cow::Owned(ordinary);
        }
    }
    Cow::Borrowed(path)
}

#[cfg(windows)]
fn ordinary_windows_spelling(path: &str) -> Option<String> {
    let suffix = path.strip_prefix(r"\\?\")?;
    let ordinary = match suffix.as_bytes() {
        [drive, b':', b'\\', ..] if drive.is_ascii_alphabetic() => {
            if !suffix[3..].is_empty() && !ordinary_components(&suffix[3..]) {
                return None;
            }
            suffix.to_owned()
        }
        _ => {
            let (prefix, unc) = suffix.split_at_checked(4)?;
            if !prefix.eq_ignore_ascii_case("UNC\\") {
                return None;
            }
            let mut components = unc.split('\\');
            if components.next()?.is_empty()
                || components.next()?.is_empty()
                || !ordinary_components(unc)
            {
                return None;
            }
            format!(r"\\{unc}")
        }
    };
    // MAX_PATH includes the terminating NUL. The receiving program may not be long-path aware.
    (ordinary.encode_utf16().count() < 260).then_some(ordinary)
}

#[cfg(windows)]
fn ordinary_components(path: &str) -> bool {
    path.strip_suffix('\\')
        .unwrap_or(path)
        .split('\\')
        .all(|name| {
            if name.is_empty()
                || name.ends_with(['.', ' '])
                || name.contains(['<', '>', ':', '"', '/', '|', '?', '*'])
                || name.bytes().any(|byte| byte < 32)
            {
                return false;
            }
            let base = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
            let base = base.to_ascii_uppercase();
            !matches!(
                base.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            ) && !base
                .strip_prefix("COM")
                .or_else(|| base.strip_prefix("LPT"))
                .is_some_and(|number| {
                    matches!(
                        number,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
        })
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_ordinary_drive_and_unc_spellings() {
        for (extended, ordinary) in [
            (r"\\?\C:\", r"C:\"),
            (r"\\?\C:\中文 文件\Unity.exe", r"C:\中文 文件\Unity.exe"),
            (r"\\?\UNC\nas\Share\项目", r"\\nas\Share\项目"),
            (r"\\?\unc\nas\Share", r"\\nas\Share"),
        ] {
            assert_eq!(
                ordinary_windows_spelling(extended).as_deref(),
                Some(ordinary)
            );
        }
        for path in [
            r"\\?\Volume{00000000-0000-0000-0000-000000000000}\",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1",
            r"\\.\pipe\alcomd",
            r"\\?\UNC\nas",
            r"\\?\UNC\nas\\share",
            r"\\?\C:relative",
            r"\\?\C:\folder. \Unity.exe",
            r"\\?\C:\folder.\Unity.exe",
            r"\\?\C:\folder\..\Unity.exe",
            r"\\?\C:\folder//Unity.exe",
            r"\\?\C:\folder\\Unity.exe",
            r"\\?\C:\folder\NUL.txt",
            r"\\?\C:\folder\COM¹.txt",
            r"\\?\C:\folder\name:stream",
        ] {
            assert_eq!(ordinary_windows_spelling(path), None, "{path}");
            assert!(matches!(
                external_filesystem_path(Path::new(path)),
                Cow::Borrowed(_)
            ));
        }
    }

    #[test]
    fn leaves_long_and_unresolved_paths_extended() {
        let short = format!(r"\\?\C:\{}\x", "😀".repeat(127));
        let long = format!(r"{short}x");
        assert_eq!(
            ordinary_windows_spelling(&short)
                .expect("259 UTF-16 units")
                .encode_utf16()
                .count(),
            259
        );
        assert_eq!(ordinary_windows_spelling(&long), None);
        let missing = std::fs::canonicalize(std::env::temp_dir())
            .expect("canonical temp directory")
            .join(format!(
                "alcomd-missing-external-path-{}",
                std::process::id()
            ));
        assert!(!missing.exists());
        assert_eq!(external_filesystem_path(&missing).as_ref(), missing);
    }
}
