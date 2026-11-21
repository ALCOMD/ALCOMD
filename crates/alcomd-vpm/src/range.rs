use std::cmp::Ordering;

use alcomd_domain::vpm_range::{RangeError, VpmRange};
use semver::Version;

/// A project's direct requirement has a different single-version meaning from a
/// package dependency or caller-supplied ordinary range.
pub(crate) struct ProjectDependencyRange(VpmRange);

impl ProjectDependencyRange {
    pub(crate) fn parse(input: &str) -> Result<Self, RangeError> {
        let ordinary = VpmRange::parse(input)?;
        let range = if let Ok(version) = Version::parse(input.trim()) {
            VpmRange::at_least(&version)?
        } else {
            ordinary
        };
        Ok(Self(range))
    }

    pub(crate) fn canonical(&self) -> String {
        self.0.canonical()
    }

    pub(crate) fn matches(&self, version: &Version) -> bool {
        self.0.matches(version, true)
    }
}

pub(crate) fn compare_precedence(left: &Version, right: &Version) -> Ordering {
    left.cmp_precedence(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_domain::vpm_range::MAX_RANGE_BYTES;
    use serde_json::Value;

    const VECTORS: &str =
        include_str!("../../alcomd-testing/fixtures/m4/version-range-vectors.json");

    #[test]
    fn frozen_differential_vectors_all_match() {
        let fixture: Value = serde_json::from_str(VECTORS).expect("range fixture");
        for vector in fixture["projectDirectDependencies"]
            .as_array()
            .expect("project vectors")
        {
            let range = ProjectDependencyRange::parse(vector["range"].as_str().expect("range"))
                .expect("direct range");
            let version =
                Version::parse(vector["version"].as_str().expect("version")).expect("version");
            assert_eq!(
                range.0.matches(&version, false),
                vector["matches"].as_bool().expect("match")
            );
        }
        for vector in fixture["vectors"].as_array().expect("vectors") {
            let id = vector["id"].as_str().expect("id");
            let range = VpmRange::parse(vector["range"].as_str().expect("range"))
                .unwrap_or_else(|error| panic!("{id}: {error}"));
            let version = Version::parse(vector["version"].as_str().expect("version"))
                .expect("fixture version");
            assert_eq!(
                range.matches(
                    &version,
                    vector["includePrerelease"]
                        .as_bool()
                        .expect("include prerelease"),
                ),
                vector["matches"].as_bool().expect("matches"),
                "{id}"
            );
        }

        for vector in fixture["intersections"].as_array().expect("intersections") {
            let id = vector["id"].as_str().expect("id");
            let version = Version::parse(vector["version"].as_str().expect("version"))
                .expect("fixture version");
            let actual = vector["ranges"]
                .as_array()
                .expect("ranges")
                .iter()
                .all(|range| {
                    VpmRange::parse(range.as_str().expect("range"))
                        .expect("valid range")
                        .matches(
                            &version,
                            vector["includePrerelease"]
                                .as_bool()
                                .expect("include prerelease"),
                        )
                });
            assert_eq!(
                actual,
                vector["matches"].as_bool().expect("matches"),
                "{id}"
            );
        }

        for range in fixture["loose"].as_array().expect("loose") {
            VpmRange::parse(range.as_str().expect("loose range")).expect("accepted loose range");
        }
        for range in fixture["invalid"].as_array().expect("invalid") {
            assert!(VpmRange::parse(range.as_str().expect("invalid range")).is_err());
        }
    }

    #[test]
    fn canonical_round_trip_preserves_semantics_and_input_order_is_irrelevant() {
        let left = VpmRange::parse(">=1.0.0 <2.0.0 || ^3.1.0").expect("range");
        let reordered = VpmRange::parse("^3.1.0 || <2.0.0 >=1.0.0").expect("range");
        assert_eq!(left.canonical(), reordered.canonical());
        let round_trip = VpmRange::parse(&left.canonical()).expect("canonical range");
        for version in ["0.9.9", "1.5.0", "2.0.0", "3.2.0", "4.0.0-beta.1"] {
            let version = Version::parse(version).expect("version");
            assert_eq!(
                left.matches(&version, false),
                round_trip.matches(&version, false)
            );
        }
    }

    #[test]
    fn build_metadata_never_changes_range_precedence() {
        let left = Version::parse("1.2.3+left").expect("version");
        let right = Version::parse("1.2.3+right").expect("version");
        assert_eq!(compare_precedence(&left, &right), Ordering::Equal);
        assert!(
            VpmRange::parse("=1.2.3+expected")
                .expect("range")
                .matches(&left, false)
        );
    }

    #[test]
    fn prerelease_eligibility_is_explicit_and_core_scoped() {
        let prerelease = Version::parse("2.0.0-beta.2").expect("version");
        let ordinary = VpmRange::parse(">=1.0.0").expect("range");
        assert!(!ordinary.matches(&prerelease, false));
        assert!(ordinary.matches(&prerelease, true));
        assert!(
            VpmRange::parse(">=2.0.0-beta.1 <2.0.0")
                .expect("range")
                .matches(&prerelease, false)
        );
        assert!(
            !VpmRange::parse(">=1.9.0-beta.1 <3.0.0")
                .expect("range")
                .matches(&prerelease, false)
        );
    }

    #[test]
    fn malformed_and_oversized_inputs_fail_without_panicking() {
        for input in [">=", "not-a-version", "1..2", "1.2.x.4", "1.2.3 - nope"] {
            assert!(VpmRange::parse(input).is_err(), "{input}");
        }
        assert_eq!(
            VpmRange::parse(&"1".repeat(MAX_RANGE_BYTES + 1)),
            Err(RangeError::TooLarge)
        );
        assert!(VpmRange::parse("1.2.3\n>=2.0.0").is_err());
    }

    #[test]
    fn repeated_matching_is_deterministic() {
        let range = VpmRange::parse("^1.2.3 || >=3.0.0 <4.0.0").expect("range");
        let version = Version::parse("3.5.0").expect("version");
        let expected = range.matches(&version, false);
        for _ in 0..1_000 {
            assert_eq!(range.matches(&version, false), expected);
        }
    }

    #[test]
    fn every_frozen_range_canonical_roundtrip_preserves_both_prerelease_modes() {
        let fixture: Value = serde_json::from_str(VECTORS).expect("range fixture");
        for vector in fixture["vectors"].as_array().expect("ordinary vectors") {
            let id = vector["id"].as_str().expect("id");
            let range = VpmRange::parse(vector["range"].as_str().expect("range"))
                .expect("valid ordinary range");
            let version = Version::parse(vector["version"].as_str().expect("version"))
                .expect("valid version");
            let canonical = range.canonical();
            let restored = VpmRange::parse(&canonical).expect("canonical ordinary range");
            for include_prerelease in [false, true] {
                assert_eq!(
                    restored.matches(&version, include_prerelease),
                    range.matches(&version, include_prerelease),
                    "{id}: {canonical}, includePrerelease={include_prerelease}",
                );
            }
        }
        for vector in fixture["projectDirectDependencies"]
            .as_array()
            .expect("project vectors")
        {
            let range = ProjectDependencyRange::parse(vector["range"].as_str().expect("range"))
                .expect("project direct dependency");
            let restored = VpmRange::parse(&range.canonical()).expect("canonical direct range");
            for text in [
                "1.2.0-alpha",
                "1.2.3-beta.1",
                "1.2.3",
                "1.4.0",
                "2.0.0-beta.1",
            ] {
                let version = Version::parse(text).expect("valid version");
                for include_prerelease in [false, true] {
                    assert_eq!(
                        restored.matches(&version, include_prerelease),
                        range.0.matches(&version, include_prerelease)
                    );
                }
            }
        }
    }
}
