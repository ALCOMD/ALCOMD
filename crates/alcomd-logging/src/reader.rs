use crate::{
    HistoryPage, LogError, LogQuery, LogRecord, RECORD_BYTES, ReadIssue, ReadPage, SCAN_BYTES,
    Segment, Shared, lock, writer,
};
use serde::{Deserialize, Serialize};

// Match the actual values users see, not JSON's escaped representation of
// backslashes, newlines and Unicode. The parsed record is already size/depth bounded.
fn contains_text(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.to_lowercase().contains(needle),
        serde_json::Value::Object(fields) => fields.iter().any(|(key, value)| {
            key.to_lowercase().contains(needle) || contains_text(value, needle)
        }),
        serde_json::Value::Array(values) => values.iter().any(|value| contains_text(value, needle)),
        value => value.to_string().to_lowercase().contains(needle),
    }
}

fn add_issue(issues: &mut Vec<ReadIssue>, issue: ReadIssue) {
    if issues.len() < 32 {
        issues.push(issue);
    } else if let Some(last) = issues.last_mut() {
        last.kind = "additional_issues".to_owned();
        last.segment_id = issue.segment_id;
        last.offset = issue.offset;
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    s: Option<String>,
    o: u64,
    #[serde(default)]
    d: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    v: u8,
    source: String,
    mode: String,
    p: Position,
    hs: String,
    hr: u64,
    q: String,
}

pub(crate) fn validate(limit: u32, query: &LogQuery) -> Result<(), LogError> {
    if !(1..=200).contains(&limit)
        || query.search.as_ref().is_some_and(|s| s.len() > 4096)
        || query.target.as_ref().is_some_and(|s| s.len() > 128)
        || query
            .diagnostic_id
            .as_ref()
            .is_some_and(|s| uuid::Uuid::parse_str(s).is_err())
        || query.levels.as_ref().is_some_and(|levels| {
            levels.len() > 5
                || levels
                    .iter()
                    .any(|s| !matches!(s.as_str(), "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR"))
        })
    {
        return Err(LogError::InvalidInput);
    }
    Ok(())
}

fn cursor(shared: &Shared, mode: &str, p: Position, query: &LogQuery) -> Cursor {
    let h = lock(&shared.health);
    Cursor {
        v: 1,
        source: shared.source_id.clone(),
        mode: mode.to_owned(),
        p,
        hs: h.session_id.clone(),
        hr: h.revision,
        q: query_key(query),
    }
}

fn query_key(query: &LogQuery) -> String {
    use sha2::{Digest, Sha256};
    let encoded = serde_json::to_vec(query).unwrap_or_default();
    let hash = Sha256::digest(encoded);
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn encode(value: &Cursor) -> Result<String, LogError> {
    serde_json::to_string(value).map_err(|_| LogError::Unavailable)
}

fn decode(
    shared: &Shared,
    text: &str,
    mode: Option<&str>,
    query: Option<&LogQuery>,
) -> Result<Cursor, LogError> {
    if text.len() > 1024 {
        return Err(LogError::InvalidInput);
    }
    let value: Cursor = serde_json::from_str(text).map_err(|_| LogError::InvalidInput)?;
    if value.v != 1
        || mode.is_some_and(|mode| value.mode != mode)
        || query.is_some_and(|q| value.q != query_key(q))
        || !matches!(value.mode.as_str(), "read" | "history")
    {
        return Err(LogError::InvalidInput);
    }
    if value.source != shared.source_id {
        return Err(LogError::CursorExpired);
    }
    if value
        .p
        .s
        .as_ref()
        .is_some_and(|s| uuid::Uuid::parse_str(s).is_err())
        || (value.p.s.is_none() && value.p.o != 0)
    {
        return Err(LogError::InvalidInput);
    }
    Ok(value)
}

fn tail(catalog: &[Segment]) -> Position {
    catalog.last().map_or(
        Position {
            s: None,
            o: 0,
            d: false,
        },
        |s| Position {
            s: Some(s.id.clone()),
            o: s.readable,
            d: false,
        },
    )
}

fn locate(catalog: &[Segment], p: &Position) -> Result<(usize, u64), LogError> {
    match &p.s {
        None => Ok((0, 0)),
        Some(id) => {
            let index = catalog
                .iter()
                .position(|s| &s.id == id)
                .ok_or(LogError::CursorExpired)?;
            if p.o > catalog[index].readable {
                return Err(LogError::CursorExpired);
            }
            Ok((index, p.o))
        }
    }
}

pub(crate) fn health_page(
    shared: &Shared,
    text: &str,
    query: &LogQuery,
) -> Result<Option<ReadPage>, LogError> {
    let mut old = decode(shared, text, Some("read"), Some(query))?;
    let health = lock(&shared.health).clone();
    if old.hs == health.session_id && old.hr == health.revision {
        return Ok(None);
    }
    let catalog = lock(&shared.catalog);
    locate(&catalog, &old.p)?;
    let has_more = old.p != tail(&catalog);
    drop(catalog);
    old.hs.clone_from(&health.session_id);
    old.hr = health.revision;
    Ok(Some(ReadPage {
        items: Vec::new(),
        next_cursor: encode(&old)?,
        baseline: false,
        health,
        issues: Vec::new(),
        has_more,
        scanned_bytes: 0,
    }))
}

fn parse(
    shared: &Shared,
    segment: &Segment,
    offset: u64,
    bytes: &[u8],
    complete: bool,
    query: &LogQuery,
    issues: &mut Vec<ReadIssue>,
) -> Option<LogRecord> {
    let issue = |kind: &str| ReadIssue {
        kind: kind.to_owned(),
        segment_id: Some(segment.id.clone()),
        offset: Some(offset),
    };
    if !complete {
        add_issue(issues, issue("incomplete_record"));
        return None;
    }
    if bytes.len().saturating_add(1) > RECORD_BYTES {
        add_issue(issues, issue("oversize_record"));
        return None;
    }
    let record: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => {
            add_issue(issues, issue("damaged_record"));
            return None;
        }
    };
    if !record
        .get("timestamp")
        .is_some_and(serde_json::Value::is_string)
        || !record
            .get("target")
            .is_some_and(serde_json::Value::is_string)
        || !record
            .get("fields")
            .is_some_and(serde_json::Value::is_object)
        || !record
            .get("level")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|l| matches!(l, "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR"))
    {
        add_issue(issues, issue("invalid_record"));
        return None;
    }
    if query.levels.as_ref().is_some_and(|levels| {
        !levels
            .iter()
            .any(|level| Some(level.as_str()) == record["level"].as_str())
    }) {
        return None;
    }
    if query.target.as_ref().is_some_and(|target| {
        !record["target"]
            .as_str()
            .unwrap_or_default()
            .starts_with(target)
    }) {
        return None;
    }
    if query
        .search
        .as_ref()
        .is_some_and(|search| !contains_text(&record, &search.to_lowercase()))
    {
        return None;
    }
    if query.diagnostic_id.as_ref().is_some_and(|id| {
        record["fields"]["diagnostic_id"].as_str() != Some(id.as_str())
            && !record["spans"].as_array().is_some_and(|spans| {
                spans
                    .iter()
                    .any(|span| span["diagnostic_id"].as_str() == Some(id.as_str()))
            })
    }) {
        return None;
    }
    Some(LogRecord {
        source_id: shared.source_id.clone(),
        session_id: segment.session.clone(),
        segment_id: segment.id.clone(),
        offset,
        record,
    })
}

pub(crate) fn read(
    shared: &Shared,
    text: Option<&str>,
    limit: u32,
    query: &LogQuery,
) -> Result<ReadPage, LogError> {
    let catalog = lock(&shared.catalog).clone();
    if text.is_none() {
        return Ok(ReadPage {
            items: Vec::new(),
            next_cursor: encode(&cursor(shared, "read", tail(&catalog), query))?,
            baseline: true,
            health: lock(&shared.health).clone(),
            issues: Vec::new(),
            has_more: false,
            scanned_bytes: 0,
        });
    }
    let mut position = decode(
        shared,
        text.ok_or(LogError::InvalidInput)?,
        Some("read"),
        Some(query),
    )?
    .p;
    let (mut index, mut offset) = locate(&catalog, &position)?;
    let mut budget = SCAN_BYTES;
    let mut scanned = 0;
    let mut items = Vec::new();
    let mut issues = Vec::new();
    while index < catalog.len() && budget > 0 && items.len() < limit as usize {
        let segment = &catalog[index];
        let count = usize::try_from(segment.readable.saturating_sub(offset))
            .map_err(|_| LogError::Unavailable)?
            .min(budget);
        if count == 0 {
            position = Position {
                s: Some(segment.id.clone()),
                o: offset,
                d: false,
            };
            index += 1;
            offset = 0;
            continue;
        }
        let buffer = writer::read_segment(segment, offset, count)?;
        let mut bytes = buffer.as_slice();
        if position.d {
            let skip = bytes
                .iter()
                .position(|b| *b == b'\n')
                .map_or(bytes.len(), |i| i + 1);
            let found = bytes[..skip].last() == Some(&b'\n');
            offset += skip as u64;
            budget -= skip;
            scanned += skip as u64;
            position = Position {
                s: Some(segment.id.clone()),
                o: offset,
                d: !found,
            };
            bytes = &bytes[skip..];
            if !found {
                continue;
            }
            if bytes.is_empty() {
                continue;
            }
        }
        let mut consumed = 0;
        let mut discard = false;
        for (start, end, complete) in line_ranges(bytes) {
            if !complete && offset + (bytes.len() as u64) < segment.readable {
                break;
            }
            if !complete && !segment.sealed {
                break;
            }
            if let Some(item) = parse(
                shared,
                segment,
                offset + start as u64,
                &bytes[start..end],
                complete,
                query,
                &mut issues,
            ) {
                items.push(item);
            }
            consumed = end + usize::from(complete);
            if items.len() >= limit as usize {
                break;
            }
        }
        if consumed == 0 {
            if bytes.len() >= RECORD_BYTES {
                add_issue(
                    &mut issues,
                    ReadIssue {
                        kind: "oversize_record".to_owned(),
                        segment_id: Some(segment.id.clone()),
                        offset: Some(offset),
                    },
                );
                consumed = bytes.len();
                discard = true;
            } else {
                break;
            }
        }
        offset += consumed as u64;
        budget -= consumed;
        scanned += consumed as u64;
        position = Position {
            s: Some(segment.id.clone()),
            o: offset,
            d: discard,
        };
        if offset >= segment.readable {
            index += 1;
            offset = 0;
        }
    }
    let has_more = position != tail(&catalog);
    shared.observe_read_issues(&issues);
    Ok(ReadPage {
        items,
        next_cursor: encode(&cursor(shared, "read", position, query))?,
        baseline: false,
        health: lock(&shared.health).clone(),
        issues,
        has_more,
        scanned_bytes: scanned,
    })
}

pub(crate) fn history(
    shared: &Shared,
    text: Option<&str>,
    before: Option<&str>,
    limit: u32,
    query: &LogQuery,
) -> Result<HistoryPage, LogError> {
    if text.is_some() && before.is_some() {
        return Err(LogError::InvalidInput);
    }
    let catalog = lock(&shared.catalog).clone();
    let mut position = match text {
        Some(v) => decode(shared, v, Some("history"), Some(query))?.p,
        None => match before {
            Some(v) => decode(shared, v, Some("read"), None)?.p,
            None => tail(&catalog),
        },
    };
    let mut items = Vec::new();
    let mut issues = Vec::new();
    let mut scanned = 0u64;
    let mut budget = SCAN_BYTES;
    let (mut index, mut end) = locate(&catalog, &position)?;
    if position.s.is_none() || catalog.is_empty() {
        return Ok(HistoryPage {
            items,
            next_cursor: None,
            health: lock(&shared.health).clone(),
            issues,
            has_more: false,
            scanned_bytes: 0,
        });
    }
    while budget > 0 && items.len() < limit as usize {
        let segment = &catalog[index];
        if end == 0 {
            if index == 0 {
                break;
            }
            index -= 1;
            end = catalog[index].readable;
            position = Position {
                s: Some(catalog[index].id.clone()),
                o: end,
                d: false,
            };
            continue;
        }
        let length = usize::try_from(end.min(budget as u64)).map_err(|_| LogError::Unavailable)?;
        let start_offset = end - length as u64;
        let buffer = writer::read_segment(segment, start_offset, length)?;
        let mut bytes = buffer.as_slice();
        if position.d {
            let boundary = bytes.iter().rposition(|b| *b == b'\n');
            match boundary {
                Some(last) => {
                    let keep = last + 1;
                    let skipped = bytes.len() - keep;
                    budget -= skipped;
                    scanned += skipped as u64;
                    end = start_offset + keep as u64;
                    position = Position {
                        s: Some(segment.id.clone()),
                        o: end,
                        d: false,
                    };
                    bytes = &bytes[..keep];
                }
                None => {
                    end = start_offset;
                    position = Position {
                        s: Some(segment.id.clone()),
                        o: end,
                        d: true,
                    };
                    budget -= bytes.len();
                    scanned += bytes.len() as u64;
                    continue;
                }
            }
        }
        let aligned = if start_offset == 0 {
            0
        } else {
            bytes
                .iter()
                .position(|b| *b == b'\n')
                .map_or(bytes.len(), |i| i + 1)
        };
        if aligned == bytes.len() {
            if bytes.len() < RECORD_BYTES && start_offset > 0 {
                break;
            }
            add_issue(
                &mut issues,
                ReadIssue {
                    kind: "oversize_record".to_owned(),
                    segment_id: Some(segment.id.clone()),
                    offset: Some(start_offset),
                },
            );
            position = Position {
                s: Some(segment.id.clone()),
                o: start_offset,
                d: true,
            };
            end = start_offset;
            budget -= bytes.len();
            scanned += bytes.len() as u64;
            continue;
        }
        let mut consumed_start = bytes.len();
        for (start, stop, complete) in line_ranges(&bytes[aligned..]).into_iter().rev() {
            let begin = aligned + start;
            let stop = aligned + stop;
            if let Some(item) = parse(
                shared,
                segment,
                start_offset + begin as u64,
                &bytes[begin..stop],
                complete,
                query,
                &mut issues,
            ) {
                items.push(item);
            }
            consumed_start = begin;
            if items.len() >= limit as usize {
                break;
            }
        }
        if consumed_start == bytes.len() {
            consumed_start = 0;
        }
        let consumed = bytes.len() - consumed_start;
        end = start_offset + consumed_start as u64;
        position = Position {
            s: Some(segment.id.clone()),
            o: end,
            d: false,
        };
        budget -= consumed;
        scanned += consumed as u64;
    }
    shared.observe_read_issues(&issues);
    let has_more = index > 0 || position.o > 0;
    let next_cursor = if has_more {
        Some(encode(&cursor(shared, "history", position, query))?)
    } else {
        None
    };
    Ok(HistoryPage {
        items,
        next_cursor,
        health: lock(&shared.health).clone(),
        issues,
        has_more,
        scanned_bytes: scanned,
    })
}

fn line_ranges(bytes: &[u8]) -> Vec<(usize, usize, bool)> {
    let mut result = Vec::new();
    let mut start = 0;
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            result.push((start, i, true));
            start = i + 1;
        }
    }
    if start < bytes.len() {
        result.push((start, bytes.len(), false));
    }
    result
}
