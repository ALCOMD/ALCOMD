//! Numeric Unity package minimum versions, independent of exact launch matching.
//!
//! `unity` and `unityRelease` follow the Unity 2022.3 package manifest manual.
//! Release ordering follows Unity's assembly version-expression manual:
//! alpha < beta < final < patch. Experimental and regional/custom versions are
//! deliberately unknown here; registration/launch retain their own exact rules.

/// A fully interpreted Unity version used only for package minimum comparisons.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnityVersion {
    major: u64,
    minor: u64,
    update: u64,
    channel: u8,
    revision: u64,
}

impl UnityVersion {
    /// The beginning of a Unity major/minor line, including its prereleases.
    #[must_use]
    pub const fn major_minor(major: u64, minor: u64) -> Self {
        Self {
            major,
            minor,
            update: 0,
            channel: 0,
            revision: 0,
        }
    }

    /// Parses a complete canonical Editor version, never a partial version.
    #[must_use]
    pub fn parse_editor(value: &str) -> Option<Self> {
        alcomd_application::canonical_unity_version(value).ok()?;
        let (major, rest) = value.split_once('.')?;
        let (minor, release) = rest.split_once('.')?;
        let (update, channel, revision) = parse_release(release)?;
        Some(Self {
            major: decimal(major)?,
            minor: decimal(minor)?,
            update,
            channel,
            revision,
        })
    }

    /// Interprets `unityRelease` only when the caller has a `unity` field.
    #[must_use]
    pub fn minimum(unity: &str, release: Option<&str>) -> Option<Self> {
        let (major, minor) = unity.split_once('.')?;
        let mut value = Self::major_minor(decimal(major)?, decimal(minor)?);
        if let Some(release) = release {
            (value.update, value.channel, value.revision) = parse_release(release)?;
        }
        Some(value)
    }
}

fn decimal(value: &str) -> Option<u64> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.len() > 1 && value.starts_with('0')
    {
        return None;
    }
    value.parse().ok()
}

fn parse_release(value: &str) -> Option<(u64, u8, u64)> {
    let channel_index = value.bytes().position(|byte| !byte.is_ascii_digit())?;
    let channel = match value.as_bytes().get(channel_index)? {
        b'a' => 1,
        b'b' => 2,
        b'f' => 3,
        b'p' => 4,
        _ => return None,
    };
    let update = decimal(value.get(..channel_index)?)?;
    let revision = decimal(value.get(channel_index + 1..)?)?;
    (revision > 0).then_some((update, channel, revision))
}

pub(crate) fn package_minimum(
    unity: Option<&str>,
    release: Option<&str>,
) -> Result<Option<UnityVersion>, ()> {
    unity
        .map(|unity| UnityVersion::minimum(unity, release).ok_or(()))
        .transpose()
}

pub(crate) fn meets_minimum(actual: Option<UnityVersion>, minimum: Option<UnityVersion>) -> bool {
    minimum.is_none_or(|minimum| actual.is_some_and(|actual| actual >= minimum))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_release_is_numeric_and_uses_documented_channel_order() {
        let minimum = package_minimum(Some("2022.3"), Some("22f1")).expect("minimum");
        for (actual, compatible) in [
            ("2022.3.21f9", false),
            ("2022.3.22b10", false),
            ("2022.3.22f1", true),
            ("2022.3.22f10", true),
            ("2022.3.22p1", true),
            ("2022.3.100f1", true),
            ("2023.1.0a1", true),
        ] {
            assert_eq!(
                meets_minimum(UnityVersion::parse_editor(actual), minimum),
                compatible,
                "{actual}"
            );
        }
        let ordered = ["0a2", "0a10", "0b1", "0f1", "0p1", "1a1"]
            .map(|release| UnityVersion::minimum("2022.3", Some(release)).expect("release"));
        assert!(ordered.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn absent_unity_adds_no_minimum_and_uninterpretable_values_stay_unknown() {
        assert_eq!(package_minimum(None, Some("garbage")), Ok(None));
        assert!(meets_minimum(None, None));
        assert!(!meets_minimum(
            None,
            Some(UnityVersion::major_minor(2022, 3))
        ));
        for release in [
            "",
            "22",
            "22f0",
            "22z1",
            "22x",
            "22x1",
            "022f1",
            "22f01",
            "22f1c1",
            "22f1-custom",
        ] {
            assert!(
                package_minimum(Some("2022.3"), Some(release)).is_err(),
                "{release}"
            );
        }
        for editor in [
            "2022.3",
            "2022.3.22",
            "2022.3.22f1c1",
            "2022.3.22f1-custom",
            "2022.3.22x",
            "2022.3.22x1",
        ] {
            assert!(UnityVersion::parse_editor(editor).is_none(), "{editor}");
        }
    }
}
