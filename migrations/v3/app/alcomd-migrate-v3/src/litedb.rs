//! Read only the two collections used by the published v3 migration source.
//! Format reference: LiteDB v5 BasePage, HeaderPage, DataPage and DataBlock.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde_json::{Map, Number, Value};

const PAGE_SIZE: usize = 8192;
const PAGE_HEADER: usize = 32;
const BLOCK_HEADER: usize = 6;
const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_RECORDS: usize = 16_384;
const MAX_BLOCKS: usize = MAX_RECORDS * 64;
const MAX_DEPTH: usize = 16;
const COLLECTIONS_OFFSET: usize = 192;
const HEADER: &[u8] = b"** This is a LiteDB file **";

pub type Document = Map<String, Value>;

#[derive(Default)]
pub struct Collections {
    pub projects: Vec<Document>,
    pub unity: Vec<Document>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct Address {
    page: usize,
    slot: u8,
}

struct Segment<'file> {
    collection: usize,
    continuation: bool,
    next: Option<Address>,
    data: &'file [u8],
}

pub fn read(bytes: &[u8]) -> Result<Collections> {
    ensure!(
        bytes.len().is_multiple_of(PAGE_SIZE),
        "invalid database length"
    );
    let mut pages = bytes.chunks_exact(PAGE_SIZE);
    let header = pages.next().context("missing database header")?;
    ensure!(
        header[..5] == [0, 0, 0, 0, 1] && &header[32..59] == HEADER && header[59] == 8,
        "unsupported database format"
    );
    ensure!(header[191] == 0, "uncommitted database state");
    let collections = document(&header[COLLECTIONS_OFFSET..], 0)?.0;
    let projects = collection_id(&collections, "projects", bytes)?;
    let unity = collection_id(&collections, "unityVersions", bytes)?;
    let last = u32_at(header, 64)?;
    ensure!(
        (last as usize) < bytes.len() / PAGE_SIZE,
        "missing database pages"
    );
    let mut by_address = BTreeMap::new();
    for (index, page) in pages.take(last as usize).enumerate() {
        let id = index + 1;
        ensure!(
            u32_at(page, 0)? as usize == id,
            "unexpected database page address"
        );
        if page[4] != 4 || page[23] == 0 {
            continue;
        }
        let collection = u32_at(page, 19)? as usize;
        if Some(collection) != projects && Some(collection) != unity {
            continue;
        }
        collect_segments(id, page, collection, &mut by_address)?;
        ensure!(by_address.len() <= MAX_BLOCKS, "too many database blocks");
    }
    let mut output = Collections::default();
    let mut consumed = BTreeSet::new();
    for (&address, segment) in &by_address {
        if segment.continuation {
            continue;
        }
        let records = if Some(segment.collection) == projects {
            &mut output.projects
        } else {
            &mut output.unity
        };
        ensure!(records.len() < MAX_RECORDS, "too many database records");
        let mut data = Vec::new();
        let mut next = Some(address);
        while let Some(address) = next {
            ensure!(
                consumed.insert(address),
                "duplicate database block reference"
            );
            let block = by_address.get(&address).context("missing database block")?;
            ensure!(
                block.collection == segment.collection,
                "database block crosses a collection boundary"
            );
            ensure!(
                block.data.len() <= MAX_DOCUMENT_BYTES.saturating_sub(data.len()),
                "database document exceeds its byte limit"
            );
            data.extend(block.data);
            next = block.next;
            if let Some(address) = next {
                ensure!(
                    by_address
                        .get(&address)
                        .is_some_and(|segment| segment.continuation),
                    "invalid continuation block"
                );
            }
        }
        let (entry, length) = document(&data, 0)?;
        ensure!(
            length == data.len(),
            "unexpected data after database document"
        );
        records.push(entry);
    }
    ensure!(
        by_address.len() == consumed.len(),
        "orphaned database block"
    );
    Ok(output)
}

fn collection_id(collections: &Document, name: &str, bytes: &[u8]) -> Result<Option<usize>> {
    let Some(value) = collections.get(name) else {
        return Ok(None);
    };
    let id = value
        .as_u64()
        .and_then(|id| usize::try_from(id).ok())
        .context("invalid collection reference")?;
    let offset = id
        .checked_mul(PAGE_SIZE)
        .context("invalid collection page")?;
    let page = bytes.get(offset..).context("missing collection page")?;
    ensure!(
        page.get(4) == Some(&2) && u32_at(page, 0)? as usize == id,
        "invalid collection page"
    );
    Ok(Some(id))
}

fn collect_segments<'file>(
    page_id: usize,
    page: &'file [u8],
    collection: usize,
    output: &mut BTreeMap<Address, Segment<'file>>,
) -> Result<()> {
    let highest = page[30];
    ensure!(highest != u8::MAX, "missing database slot table");
    let footer = PAGE_SIZE - 4 * (usize::from(highest) + 1);
    let mut occupied = Vec::with_capacity(usize::from(page[23]));
    for slot in 0..=highest {
        let footer_offset = PAGE_SIZE - 4 * (usize::from(slot) + 1);
        let len = usize::from(u16_at(page, footer_offset)?);
        let start = usize::from(u16_at(page, footer_offset + 2)?);
        if start == 0 {
            ensure!(len == 0, "unused database slot has a length");
            continue;
        }
        let end = start + len;
        ensure!(
            start >= PAGE_HEADER && end <= footer && len > BLOCK_HEADER,
            "invalid database slot bounds"
        );
        occupied.push((start, end));
        let block = &page[start..end];
        ensure!(block[0] <= 1, "invalid database block type");
        let next_page = u32_at(block, 1)?;
        let next_slot = block[5];
        let next = match (next_page, next_slot) {
            (u32::MAX, u8::MAX) => None,
            (u32::MAX, _) | (_, u8::MAX) => anyhow::bail!("invalid database block address"),
            (page, slot) => Some(Address {
                page: page as usize,
                slot,
            }),
        };
        output.insert(
            Address {
                page: page_id,
                slot,
            },
            Segment {
                collection,
                continuation: block[0] == 1,
                next,
                data: &block[BLOCK_HEADER..],
            },
        );
    }
    occupied.sort_unstable();
    ensure!(
        occupied.len() == usize::from(page[23])
            && occupied.windows(2).all(|pair| pair[0].1 <= pair[1].0),
        "inconsistent database slot table"
    );
    Ok(())
}

fn document(bytes: &[u8], depth: usize) -> Result<(Document, usize)> {
    ensure!(
        depth < MAX_DEPTH,
        "database document exceeds its nesting limit"
    );
    let len = u32_at(bytes, 0)? as usize;
    ensure!(
        (5..=MAX_DOCUMENT_BYTES).contains(&len),
        "invalid database document length"
    );
    let (terminator, elements) = bytes
        .get(4..len)
        .and_then(<[u8]>::split_last)
        .context("truncated database document")?;
    ensure!(*terminator == 0, "missing database document terminator");
    let mut cursor = elements;
    let mut data = Map::new();
    while let Some((&kind, rest)) = cursor.split_first() {
        cursor = rest;
        let name = cstring(&mut cursor)?;
        let item = element(kind, &mut cursor, depth)?;
        ensure!(
            data.insert(name.to_owned(), item).is_none(),
            "duplicate database field"
        );
    }
    Ok((data, len))
}

fn element(kind: u8, bytes: &mut &[u8], depth: usize) -> Result<Value> {
    Ok(match kind {
        0x01 => Number::from_f64(f64::from_le_bytes(take(bytes, 8)?.try_into()?))
            .map_or(Value::Null, Value::Number),
        0x02 => {
            let len = u32::from_le_bytes(take(bytes, 4)?.try_into()?) as usize;
            let (terminator, contents) = take(bytes, len)?
                .split_last()
                .context("empty BSON string")?;
            ensure!(*terminator == 0, "invalid BSON string terminator");
            Value::String(std::str::from_utf8(contents)?.to_owned())
        }
        0x03 | 0x04 => {
            let (items, consumed) = document(bytes, depth + 1)?;
            let _ = take(bytes, consumed)?;
            if kind == 0x03 {
                Value::Object(items)
            } else {
                Value::Array(
                    (0..items.len())
                        .map(|index| {
                            items
                                .get(&index.to_string())
                                .cloned()
                                .context("invalid BSON array order")
                        })
                        .collect::<Result<Vec<_>>>()?,
                )
            }
        }
        0x05 => {
            let len = u32::from_le_bytes(take(bytes, 4)?.try_into()?) as usize;
            let _ = take(
                bytes,
                len.checked_add(1).context("invalid binary field length")?,
            )?;
            Value::Null
        }
        0x07 => {
            let _ = take(bytes, 12)?;
            Value::Null
        }
        0x08 => match take(bytes, 1)?[0] {
            0 => Value::Bool(false),
            1 => Value::Bool(true),
            _ => anyhow::bail!("invalid BSON boolean"),
        },
        0x09 | 0x12 => Value::from(i64::from_le_bytes(take(bytes, 8)?.try_into()?)),
        0x0A | 0x7F | 0xFF => Value::Null,
        0x10 => Value::from(i32::from_le_bytes(take(bytes, 4)?.try_into()?)),
        0x13 => {
            let _ = take(bytes, 16)?;
            Value::Null
        }
        _ => anyhow::bail!("unrecognized BSON field type"),
    })
}

fn cstring<'a>(bytes: &mut &'a [u8]) -> Result<&'a str> {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .context("missing BSON field terminator")?;
    let name = std::str::from_utf8(take(bytes, end + 1)?.split_last().context("BSON field")?.1)?;
    Ok(name)
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8]> {
    let (value, rest) = bytes
        .split_at_checked(len)
        .context("truncated BSON field")?;
    *bytes = rest;
    Ok(value)
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("truncated database header")?
            .try_into()?,
    ))
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .context("truncated database header")?
            .try_into()?,
    ))
}
