use std::cmp::Ordering;
use std::collections::BTreeSet;
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
    source: String,
    prerelease_cores: BTreeSet<(u64, u64, u64)>,
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
        arms.sort_by_key(Conjunction::canonical);
        arms.dedup();
        Ok(Self { arms })
    }

    pub(crate) fn matches(&self, version: &Version, include_prerelease: bool) -> bool {
        self.arms
            .iter()
            .any(|arm| arm.matches(version, include_prerelease))
    }

    pub(crate) fn canonical(&self) -> String {
        self.arms
            .iter()
            .map(Conjunction::canonical)
            .collect::<Vec<_>>()
            .join(" || ")
    }
}

/// A project's direct requirement has a different single-version meaning from a
/// package dependency or caller-supplied ordinary range.
pub(crate) struct ProjectDependencyRange(VpmRange);

impl ProjectDependencyRange {
    pub(crate) fn parse(input: &str) -> Result<Self, RangeError> {
        let ordinary = VpmRange::parse(input)?;
        let range = if let Ok(version) = Version::parse(input.trim()) {
            VpmRange {
                arms: vec![parse_conjunction(&format!(">={version}"))?],
            }
        } else {
            ordinary
        };
        Ok(Self(range))
    }

    pub(crate) fn canonical(&self) -> String {
        self.0.canonical()
    }
}

impl Conjunction {
    fn matches(&self, version: &Version, include_prerelease: bool) -> bool {
        if !include_prerelease
            && !version.pre.is_empty()
            && !self
                .prerelease_cores
                .contains(&(version.major, version.minor, version.patch))
        {
            return false;
        }
        self.predicates
            .iter()
            .all(|predicate| predicate.matches(version))
    }

    fn canonical(&self) -> String {
        self.source.clone()
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
}

pub(crate) fn compare_precedence(left: &Version, right: &Version) -> Ordering {
    left.cmp_precedence(right)
}

#[derive(Clone)]
struct PartialVersion {
    lower: Version,
    precision: usize,
}

impl PartialVersion {
    fn parse(input: &str) -> Result<Self, RangeError> {
        if let Ok(lower) = Version::parse(input) {
            return Ok(Self {
                lower,
                precision: 3,
            });
        }
        let parts = input.split('.').collect::<Vec<_>>();
        if !(1..=3).contains(&parts.len()) {
            return Err(RangeError::Invalid);
        }
        let mut numbers = [0; 3];
        let mut precision = 0;
        let mut wildcard = false;
        for (index, part) in parts.iter().enumerate() {
            if matches!(*part, "*" | "x" | "X") {
                wildcard = true;
                continue;
            }
            if wildcard
                || part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.len() > 1 && part.starts_with('0')
            {
                return Err(RangeError::Invalid);
            }
            numbers[index] = part.parse().map_err(|_| RangeError::Invalid)?;
            precision += 1;
        }
        Ok(Self {
            lower: Version::new(numbers[0], numbers[1], numbers[2]),
            precision,
        })
    }

    fn next(&self) -> Result<Option<Version>, RangeError> {
        Ok(match self.precision {
            0 => None,
            1 => Some(Version::new(checked_next(self.lower.major)?, 0, 0)),
            2 => Some(Version::new(
                self.lower.major,
                checked_next(self.lower.minor)?,
                0,
            )),
            _ => Some(Version::new(
                self.lower.major,
                self.lower.minor,
                checked_next(self.lower.patch)?,
            )),
        })
    }
}

fn before_release(mut version: Version) -> Version {
    version.pre = semver::Prerelease::new("0").expect("constant prerelease");
    version
}

fn parse_conjunction(input: &str) -> Result<Conjunction, RangeError> {
    if input.is_empty() {
        return Ok(Conjunction {
            predicates: Vec::new(),
            source: "*".to_owned(),
            prerelease_cores: BTreeSet::new(),
        });
    }
    let tokens = input.split_ascii_whitespace().collect::<Vec<_>>();
    if tokens.len() > MAX_PREDICATES {
        return Err(RangeError::TooLarge);
    }
    let mut predicates = Vec::new();
    let mut source_terms = Vec::new();
    let mut prerelease_cores = BTreeSet::new();
    for token in &tokens {
        let operand = token.trim_start_matches(['>', '<', '=', '^', '~']);
        if let Ok(version) = Version::parse(operand)
            && !version.pre.is_empty()
        {
            prerelease_cores.insert((version.major, version.minor, version.patch));
        }
    }
    let mut index = 0;
    while index < tokens.len() {
        if tokens.get(index + 1) == Some(&"-") {
            let lower = PartialVersion::parse(tokens[index])?;
            let upper = PartialVersion::parse(tokens.get(index + 2).ok_or(RangeError::Invalid)?)?;
            if lower.precision > 0 {
                predicates.push(Predicate::GreaterOrEqual(if lower.precision < 3 {
                    before_release(lower.lower)
                } else {
                    lower.lower
                }));
            }
            source_terms.push(format!("{} - {}", tokens[index], tokens[index + 2]));
            if upper.precision == 3 {
                predicates.push(Predicate::LessOrEqual(upper.lower));
            } else if let Some(next) = upper.next()? {
                predicates.push(Predicate::Less(before_release(next)));
            }
            index += 3;
        } else {
            let token = tokens[index];
            if matches!(token, ">=" | "<=" | ">" | "<" | "=" | "^" | "~") {
                let operand = tokens.get(index + 1).ok_or(RangeError::Invalid)?;
                source_terms.push(format!("{token}{operand}"));
                predicates.extend(parse_primitive(&format!("{token}{operand}"))?);
                index += 2;
            } else {
                source_terms.push(token.to_owned());
                predicates.extend(parse_primitive(token)?);
                index += 1;
            }
        }
        if predicates.len() > MAX_PREDICATES {
            return Err(RangeError::TooLarge);
        }
    }
    source_terms.sort();
    source_terms.dedup();
    Ok(Conjunction {
        predicates,
        source: source_terms.join(" "),
        prerelease_cores,
    })
}

fn parse_primitive(input: &str) -> Result<Vec<Predicate>, RangeError> {
    let (operator, operand) = [">=", "<=", ">", "<", "=", "^", "~"]
        .into_iter()
        .find_map(|operator| input.strip_prefix(operator).map(|value| (operator, value)))
        .unwrap_or(("", input));
    let partial = PartialVersion::parse(operand)?;
    if partial.precision == 0 {
        return Ok(match operator {
            ">" => vec![Predicate::Less(Version::new(0, 0, 0))],
            "<" => vec![Predicate::Less(before_release(Version::new(0, 0, 0)))],
            "<=" | "~" | "^" => vec![Predicate::GreaterOrEqual(Version::new(0, 0, 0))],
            _ => Vec::new(),
        });
    }
    let lower = partial.lower.clone();
    let partial_lower = || before_release(lower.clone());
    match operator {
        ">=" => Ok(vec![Predicate::GreaterOrEqual(if partial.precision < 3 {
            partial_lower()
        } else {
            lower
        })]),
        "<=" => Ok(vec![if partial.precision < 3 {
            Predicate::Less(partial.next()?.ok_or(RangeError::Invalid)?)
        } else {
            Predicate::LessOrEqual(lower)
        }]),
        ">" => Ok(vec![if partial.precision < 3 {
            Predicate::GreaterOrEqual(before_release(partial.next()?.ok_or(RangeError::Invalid)?))
        } else {
            Predicate::Greater(lower)
        }]),
        "<" => Ok(vec![Predicate::Less(if partial.precision < 3 {
            partial_lower()
        } else {
            lower
        })]),
        "^" | "~" => {
            let upper = if operator == "~" {
                if partial.precision == 1 {
                    Version::new(checked_next(lower.major)?, 0, 0)
                } else {
                    Version::new(lower.major, checked_next(lower.minor)?, 0)
                }
            } else if lower.major > 0 || partial.precision == 1 {
                Version::new(checked_next(lower.major)?, 0, 0)
            } else if lower.minor > 0 || partial.precision == 2 {
                Version::new(0, checked_next(lower.minor)?, 0)
            } else {
                Version::new(0, 0, checked_next(lower.patch)?)
            };
            Ok(vec![
                Predicate::GreaterOrEqual(lower),
                Predicate::Less(before_release(upper)),
            ])
        }
        _ if partial.precision == 3 => Ok(vec![Predicate::Exact(lower)]),
        _ => Ok(vec![
            Predicate::GreaterOrEqual(before_release(lower)),
            Predicate::Less(before_release(partial.next()?.ok_or(RangeError::Invalid)?)),
        ]),
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
