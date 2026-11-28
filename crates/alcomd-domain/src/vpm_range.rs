use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;

use semver::Version;

pub const MAX_RANGE_BYTES: usize = 1_024;
const MAX_OR_ARMS: usize = 64;
const MAX_PREDICATES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VpmRange {
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
pub enum RangeError {
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
    pub fn parse(input: &str) -> Result<Self, RangeError> {
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

    pub fn at_least(version: &Version) -> Result<Self, RangeError> {
        Ok(Self {
            arms: vec![parse_conjunction(&format!(">={version}"))?],
        })
    }

    pub fn matches(&self, version: &Version, include_prerelease: bool) -> bool {
        self.arms
            .iter()
            .any(|arm| arm.matches(version, include_prerelease))
    }

    pub fn canonical(&self) -> String {
        self.arms
            .iter()
            .map(Conjunction::canonical)
            .collect::<Vec<_>>()
            .join(" || ")
    }

    /// Conjoins the range arms and rejects a stored result that changes prerelease matching.
    pub fn intersect(&self, other: &Self) -> Result<Self, RangeError> {
        if self.arms.len() * other.arms.len() > MAX_OR_ARMS {
            return Err(RangeError::TooLarge);
        }
        let mut source = String::new();
        for left in &self.arms {
            for right in &other.arms {
                if !source.is_empty() {
                    source.push_str(" || ");
                }
                source.push_str(&left.source);
                source.push(' ');
                source.push_str(&right.source);
                if source.len() > MAX_RANGE_BYTES {
                    return Err(RangeError::TooLarge);
                }
            }
        }
        let combined = Self::parse(&source)?;
        let opted_in_cores: BTreeSet<_> = combined
            .arms
            .iter()
            .flat_map(|arm| arm.prerelease_cores.iter().copied())
            .collect();
        let mut representatives: BTreeSet<_> = opted_in_cores
            .iter()
            .map(|&(major, minor, patch)| before_release(Version::new(major, minor, patch)))
            .collect();
        for bound in combined
            .arms
            .iter()
            .flat_map(|arm| arm.predicates.iter().map(Predicate::version))
        {
            if bound.pre.is_empty()
                || !opted_in_cores.contains(&(bound.major, bound.minor, bound.patch))
            {
                continue;
            }
            let mut at_bound = bound.clone();
            at_bound.build = semver::BuildMetadata::EMPTY;
            representatives.insert(at_bound.clone());
            at_bound.pre = semver::Prerelease::new(&format!("{}.0", bound.pre))
                .map_err(|_| RangeError::Invalid)?;
            representatives.insert(at_bound);
        }

        // Conjoining comparators preserves release matches and matches with prereleases enabled.
        // In ordinary mode a combined arm can opt into a prerelease that one operand excludes.
        // At each affected core, the first prerelease, each comparator boundary, and its immediate
        // successor (append `.0`) cover every interval on which the predicates have fixed results.
        if representatives.iter().any(|version| {
            combined.matches(version, false)
                != (self.matches(version, false) && other.matches(version, false))
        }) {
            return Err(RangeError::Invalid);
        }
        Ok(combined)
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
        let ordering = candidate.cmp_precedence(self.version());
        match self {
            Self::Exact(_) => ordering == Ordering::Equal,
            Self::Greater(_) => ordering == Ordering::Greater,
            Self::GreaterOrEqual(_) => ordering != Ordering::Less,
            Self::Less(_) => ordering == Ordering::Less,
            Self::LessOrEqual(_) => ordering != Ordering::Greater,
        }
    }
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

    #[test]
    fn stored_intersections_preserve_releases_and_compatible_prerelease_permissions()
    -> Result<(), Box<dyn std::error::Error>> {
        for (first, second) in [
            ("^1.0.0", ">=1.1.0 <1.2.0"),
            (">=1.1.0-alpha.1 <2.0.0", ">=1.1.0-beta.1 <1.2.0"),
            (">=1.1.0 <2.0.0", ">=1.1.0-beta.1"),
            (
                ">=1.0.0 <2.0.0 || >=1.1.0-beta.1 <1.1.0",
                ">=1.1.0-beta.1 <1.1.0",
            ),
        ] {
            let left = VpmRange::parse(first)?;
            let right = VpmRange::parse(second)?;
            let intersection = left.intersect(&right)?;
            let stored = VpmRange::parse(&intersection.canonical())?;
            for version in [
                "1.0.0",
                "1.1.0-0",
                "1.1.0-alpha.1",
                "1.1.0-beta.1",
                "1.1.0-beta.1.0",
                "1.1.0",
                "1.2.0",
                "2.0.0",
            ] {
                let version = Version::parse(version)?;
                for include_prerelease in [false, true] {
                    assert_eq!(
                        stored.matches(&version, include_prerelease),
                        left.matches(&version, include_prerelease)
                            && right.matches(&version, include_prerelease),
                        "{first} intersect {second} at {version}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn intersection_rejects_prerelease_admission_from_only_one_operand() -> Result<(), RangeError> {
        let stable = VpmRange::parse(">=1.0.0 <2.0.0")?;
        for range in ["=1.1.0-beta.1", ">1.1.0-beta.1 <1.1.0"] {
            let explicit = VpmRange::parse(range)?;
            assert_eq!(stable.intersect(&explicit), Err(RangeError::Invalid));
        }
        Ok(())
    }
}
