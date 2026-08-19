use std::cmp::Ordering;
use std::fmt;

use semver::Version;

const MAX_RANGE_BYTES: usize = 1_024;
const MAX_OR_ARMS: usize = 64;
const MAX_PREDICATES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VpmRange {
    arms: Vec<Conjunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Conjunction {
    predicates: Vec<Predicate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Predicate {
    Exact(Version),
    Greater(Version),
    GreaterOrEqual(Version),
    Less(Version),
    LessOrEqual(Version),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RangeError {
    Invalid,
    TooLarge,
}

impl fmt::Display for RangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => formatter.write_str("invalid VPM version range"),
            Self::TooLarge => formatter.write_str("VPM version range exceeds its limit"),
        }
    }
}

impl std::error::Error for RangeError {}

impl VpmRange {
    pub(crate) fn parse(input: &str) -> Result<Self, RangeError> {
        if input.len() > MAX_RANGE_BYTES {
            return Err(RangeError::TooLarge);
        }
        if input.chars().any(char::is_control) {
            return Err(RangeError::Invalid);
        }

        let raw_arms = input.split("||").collect::<Vec<_>>();
        if raw_arms.len() > MAX_OR_ARMS {
            return Err(RangeError::TooLarge);
        }
        let mut arms = Vec::with_capacity(raw_arms.len());
        for raw_arm in raw_arms {
            arms.push(parse_conjunction(raw_arm.trim())?);
        }
        if arms.iter().any(|arm| arm.predicates.is_empty()) {
            arms.clear();
            arms.push(Conjunction {
                predicates: Vec::new(),
            });
        } else {
            arms.sort_by_key(Conjunction::canonical);
            arms.dedup();
        }
        Ok(Self { arms })
    }

    pub(crate) fn matches(&self, version: &Version, include_prerelease: bool) -> bool {
        self.arms
            .iter()
            .any(|arm| arm.matches(version, include_prerelease))
    }

    pub(crate) fn canonical(&self) -> String {
        if self.arms.iter().any(|arm| arm.predicates.is_empty()) {
            return "*".to_owned();
        }
        self.arms
            .iter()
            .map(Conjunction::canonical)
            .collect::<Vec<_>>()
            .join(" || ")
    }
}

impl Conjunction {
    fn matches(&self, version: &Version, include_prerelease: bool) -> bool {
        if !include_prerelease
            && !version.pre.is_empty()
            && !self.predicates.iter().any(|predicate| {
                let reference = predicate.version();
                !reference.pre.is_empty()
                    && reference.major == version.major
                    && reference.minor == version.minor
                    && reference.patch == version.patch
            })
        {
            return false;
        }
        self.predicates
            .iter()
            .all(|predicate| predicate.matches(version))
    }

    fn canonical(&self) -> String {
        if self.predicates.is_empty() {
            return "*".to_owned();
        }
        let mut predicates = self
            .predicates
            .iter()
            .map(Predicate::canonical)
            .collect::<Vec<_>>();
        predicates.sort();
        predicates.dedup();
        predicates.join(" ")
    }
}

impl Predicate {
    fn version(&self) -> &Version {
        match self {
            Self::Exact(version)
            | Self::Greater(version)
            | Self::GreaterOrEqual(version)
            | Self::Less(version)
            | Self::LessOrEqual(version) => version,
        }
    }

    fn matches(&self, candidate: &Version) -> bool {
        let ordering = compare_precedence(candidate, self.version());
        match self {
            Self::Exact(_) => ordering == Ordering::Equal,
            Self::Greater(_) => ordering == Ordering::Greater,
            Self::GreaterOrEqual(_) => ordering != Ordering::Less,
            Self::Less(_) => ordering == Ordering::Less,
            Self::LessOrEqual(_) => ordering != Ordering::Greater,
        }
    }

    fn canonical(&self) -> String {
        let operator = match self {
            Self::Exact(_) => "=",
            Self::Greater(_) => ">",
            Self::GreaterOrEqual(_) => ">=",
            Self::Less(_) => "<",
            Self::LessOrEqual(_) => "<=",
        };
        format!("{operator}{}", self.version())
    }
}

pub(crate) fn compare_precedence(left: &Version, right: &Version) -> Ordering {
    left.cmp_precedence(right)
}

fn parse_conjunction(input: &str) -> Result<Conjunction, RangeError> {
    if input.is_empty() || matches!(input, "*" | "x" | "X") {
        return Ok(Conjunction {
            predicates: Vec::new(),
        });
    }

    let tokens = input.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.len() > MAX_PREDICATES {
        return Err(RangeError::TooLarge);
    }
    if tokens.len() == 3 && tokens[1] == "-" {
        let lower = parse_version(tokens[0])?;
        let upper = parse_version(tokens[2])?;
        return Ok(Conjunction {
            predicates: vec![
                Predicate::GreaterOrEqual(lower),
                Predicate::LessOrEqual(upper),
            ],
        });
    }

    let mut predicates = Vec::new();
    for token in tokens {
        if token == "-" && !predicates.is_empty() {
            continue;
        }
        predicates.extend(parse_primitive(token)?);
        if predicates.len() > MAX_PREDICATES {
            return Err(RangeError::TooLarge);
        }
    }
    if predicates.is_empty() {
        return Err(RangeError::Invalid);
    }
    Ok(Conjunction { predicates })
}

fn parse_primitive(input: &str) -> Result<Vec<Predicate>, RangeError> {
    if let Some(version) = input.strip_prefix(">=") {
        return Ok(vec![Predicate::GreaterOrEqual(parse_version(version)?)]);
    }
    if let Some(version) = input.strip_prefix("<=") {
        return Ok(vec![Predicate::LessOrEqual(parse_version(version)?)]);
    }
    if let Some(version) = input.strip_prefix('>') {
        return Ok(vec![Predicate::Greater(parse_version(version)?)]);
    }
    if let Some(version) = input.strip_prefix('<') {
        return Ok(vec![Predicate::Less(parse_version(version)?)]);
    }
    if let Some(version) = input.strip_prefix('=') {
        return Ok(vec![Predicate::Exact(parse_version(version)?)]);
    }
    if let Some(version) = input.strip_prefix('^') {
        return caret(version);
    }
    if let Some(version) = input.strip_prefix('~') {
        return tilde(version);
    }
    if input.contains('*') || input.contains('x') || input.contains('X') {
        return wildcard(input);
    }

    Ok(vec![Predicate::GreaterOrEqual(parse_version(input)?)])
}

fn parse_version(input: &str) -> Result<Version, RangeError> {
    if input.is_empty() {
        return Err(RangeError::Invalid);
    }
    Version::parse(input).map_err(|_| RangeError::Invalid)
}

fn caret(input: &str) -> Result<Vec<Predicate>, RangeError> {
    let lower = parse_version(input)?;
    let upper = if lower.major > 0 {
        Version::new(checked_next(lower.major)?, 0, 0)
    } else if lower.minor > 0 {
        Version::new(0, checked_next(lower.minor)?, 0)
    } else {
        Version::new(0, 0, checked_next(lower.patch)?)
    };
    Ok(vec![
        Predicate::GreaterOrEqual(lower),
        Predicate::Less(upper),
    ])
}

fn tilde(input: &str) -> Result<Vec<Predicate>, RangeError> {
    let lower = parse_version(input)?;
    let upper = Version::new(lower.major, checked_next(lower.minor)?, 0);
    Ok(vec![
        Predicate::GreaterOrEqual(lower),
        Predicate::Less(upper),
    ])
}

fn wildcard(input: &str) -> Result<Vec<Predicate>, RangeError> {
    let parts = input.split('.').collect::<Vec<_>>();
    if !(1..=3).contains(&parts.len()) {
        return Err(RangeError::Invalid);
    }
    let wildcard_at = parts
        .iter()
        .position(|part| matches!(*part, "*" | "x" | "X"))
        .ok_or(RangeError::Invalid)?;
    if parts[wildcard_at..]
        .iter()
        .any(|part| !matches!(*part, "*" | "x" | "X"))
    {
        return Err(RangeError::Invalid);
    }
    let numeric = parts[..wildcard_at]
        .iter()
        .map(|part| part.parse::<u64>().map_err(|_| RangeError::Invalid))
        .collect::<Result<Vec<_>, _>>()?;
    match numeric.as_slice() {
        [] => Ok(Vec::new()),
        [major] => Ok(vec![
            Predicate::GreaterOrEqual(Version::new(*major, 0, 0)),
            Predicate::Less(Version::new(checked_next(*major)?, 0, 0)),
        ]),
        [major, minor] => Ok(vec![
            Predicate::GreaterOrEqual(Version::new(*major, *minor, 0)),
            Predicate::Less(Version::new(*major, checked_next(*minor)?, 0)),
        ]),
        _ => Err(RangeError::Invalid),
    }
}

fn checked_next(value: u64) -> Result<u64, RangeError> {
    value.checked_add(1).ok_or(RangeError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const VECTORS: &str =
        include_str!("../../alcomd-testing/fixtures/m4/version-range-vectors.json");

    #[test]
    fn frozen_differential_vectors_all_match() {
        let fixture: Value = serde_json::from_str(VECTORS).expect("range fixture");
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
}
