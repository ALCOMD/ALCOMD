//! Test-only Unity metadata on a new copy of the calling Windows test executable.
//!
//! This writes only a new file owned by the caller's fixture. It adds one resource
//! section to an unsigned PE instead of requiring an SDK resource compiler or FFI.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Creates an executable fixture with actual Win32 ProductName/ProductVersion metadata.
pub fn write_editor_fixture(
    executable: &Path,
    product_name: &str,
    product_version: &str,
) -> io::Result<()> {
    let source = std::env::current_exe()?;
    if fs::metadata(&source)?.len() > 512 * 1024 * 1024 {
        return Err(invalid("calling test executable is too large"));
    }
    let mut image = fs::read(source)?;
    let version = version_resource(product_name, product_version)?;
    add_resource_section(&mut image, &version)?;
    // Never replace a previously existing file, including another test's fixture.
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(executable)?;
    destination.write_all(&image)?;
    destination.sync_all()
}

fn add_resource_section(image: &mut Vec<u8>, version: &[u8]) -> io::Result<()> {
    if image.get(..2) != Some(b"MZ") {
        return Err(invalid("test executable has no DOS header"));
    }
    let pe = usize::try_from(read_u32(image, 0x3c)?).map_err(|_| invalid("PE offset"))?;
    if image.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err(invalid("test executable has no PE signature"));
    }
    let coff = pe.checked_add(4).ok_or_else(|| invalid("COFF offset"))?;
    let sections = usize::from(read_u16(image, coff + 2)?);
    if sections == 0 || sections >= 96 {
        return Err(invalid("unexpected test executable section count"));
    }
    let optional = coff + 20;
    let optional_size = usize::from(read_u16(image, coff + 16)?);
    let directories = match read_u16(image, optional)? {
        0x10b => optional + 96,
        0x20b => optional + 112,
        _ => return Err(invalid("test executable is not PE32/PE32+")),
    };
    let table = optional
        .checked_add(optional_size)
        .ok_or_else(|| invalid("section table offset"))?;
    if directories + 8 * 5 > table {
        return Err(invalid("test executable has incomplete data directories"));
    }
    // The caller is a local, unsigned Rust test executable. Never invalidate a signature.
    if read_u32(image, directories + 8 * 4)? != 0 || read_u32(image, directories + 8 * 4 + 4)? != 0
    {
        return Err(invalid(
            "test executable unexpectedly contains a certificate",
        ));
    }
    let file_alignment = read_u32(image, optional + 36)?;
    let section_alignment = read_u32(image, optional + 32)?;
    let headers_size = read_u32(image, optional + 60)? as usize;
    let new_header = table
        .checked_add(sections * 40)
        .ok_or_else(|| invalid("new section header offset"))?;
    let header_end = new_header
        .checked_add(40)
        .ok_or_else(|| invalid("new section header end"))?;
    let mut virtual_end = 0_u32;
    let mut first_raw = image.len();
    for index in 0..sections {
        let entry = table + index * 40;
        let length = read_u32(image, entry + 8)?.max(read_u32(image, entry + 16)?);
        let end = read_u32(image, entry + 12)?
            .checked_add(length)
            .ok_or_else(|| invalid("section virtual end"))?;
        virtual_end = virtual_end.max(end);
        let raw = read_u32(image, entry + 20)? as usize;
        if raw > 0 {
            first_raw = first_raw.min(raw);
        }
    }
    if header_end > headers_size || header_end > first_raw || header_end > image.len() {
        return Err(invalid("test executable has no spare section header space"));
    }
    if image[new_header..header_end].iter().any(|byte| *byte != 0) {
        return Err(invalid("spare section header is not empty"));
    }
    let virtual_address = align(virtual_end, section_alignment)?;
    let raw_address = align(
        u32::try_from(image.len()).map_err(|_| invalid("test executable size"))?,
        file_alignment,
    )?;
    let resource = resource_tree(virtual_address, version)?;
    let virtual_size = u32::try_from(resource.len()).map_err(|_| invalid("resource size"))?;
    let raw_size = align(virtual_size, file_alignment)?;
    let raw_end = raw_address
        .checked_add(raw_size)
        .ok_or_else(|| invalid("resource raw end"))?;
    let image_size = align(
        virtual_address
            .checked_add(virtual_size)
            .ok_or_else(|| invalid("resource virtual end"))?,
        section_alignment,
    )?;
    let initialized_size = read_u32(image, optional + 8)?
        .checked_add(raw_size)
        .ok_or_else(|| invalid("initialized data size"))?;
    image.resize(raw_end as usize, 0);
    image[raw_address as usize..raw_address as usize + resource.len()].copy_from_slice(&resource);
    image[new_header..new_header + 8].copy_from_slice(b".rsrc\0\0\0");
    write_u32(image, new_header + 8, virtual_size)?;
    write_u32(image, new_header + 12, virtual_address)?;
    write_u32(image, new_header + 16, raw_size)?;
    write_u32(image, new_header + 20, raw_address)?;
    write_u32(image, new_header + 36, 0x4000_0040)?; // Read-only initialized data.
    write_u16(image, coff + 2, (sections + 1) as u16)?;
    write_u32(image, optional + 8, initialized_size)?;
    write_u32(image, optional + 56, image_size)?;
    write_u32(image, optional + 64, 0)?; // Checksum is optional for ordinary test executables.
    write_u32(image, directories + 8 * 2, virtual_address)?;
    write_u32(image, directories + 8 * 2 + 4, virtual_size)
}

fn resource_tree(virtual_address: u32, version: &[u8]) -> io::Result<Vec<u8>> {
    // Three directory levels: RT_VERSION (16), resource ID 1, language en-US (0x409).
    let mut bytes = vec![0; 88];
    for offset in [0, 24, 48] {
        write_u16(&mut bytes, offset + 14, 1)?;
    }
    write_u32(&mut bytes, 16, 16)?;
    write_u32(&mut bytes, 20, 0x8000_0018)?;
    write_u32(&mut bytes, 40, 1)?;
    write_u32(&mut bytes, 44, 0x8000_0030)?;
    write_u32(&mut bytes, 64, 0x409)?;
    write_u32(&mut bytes, 68, 72)?;
    write_u32(
        &mut bytes,
        72,
        virtual_address
            .checked_add(88)
            .ok_or_else(|| invalid("version resource RVA"))?,
    )?;
    write_u32(
        &mut bytes,
        76,
        u32::try_from(version.len()).map_err(|_| invalid("version resource size"))?,
    )?;
    write_u32(&mut bytes, 80, 1200)?;
    bytes.extend_from_slice(version);
    Ok(bytes)
}

fn version_resource(product_name: &str, product_version: &str) -> io::Result<Vec<u8>> {
    let strings = block(
        "StringFileInfo",
        &[],
        0,
        1,
        &[block(
            "040904B0",
            &[],
            0,
            1,
            &[
                string_block("ProductName", product_name)?,
                string_block("ProductVersion", product_version)?,
                string_block("FileVersion", "0.0.0.0")?,
            ],
        )?],
    )?;
    let variables = block(
        "VarFileInfo",
        &[],
        0,
        1,
        &[block("Translation", &[0x09, 0x04, 0xb0, 0x04], 4, 0, &[])?],
    )?;
    let fixed = [
        0xfeef_04bd_u32,
        0x0001_0000,
        0,
        0,
        0,
        0,
        0x3f,
        0,
        0x0004_0004,
        1,
        0,
        0,
        0,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    block("VS_VERSION_INFO", &fixed, 52, 0, &[strings, variables])
}

fn string_block(key: &str, value: &str) -> io::Result<Vec<u8>> {
    if value.contains('\0') || value.encode_utf16().count() > 1024 {
        return Err(invalid("fixture metadata is too long or contains NUL"));
    }
    let wide = value
        .encode_utf16()
        .chain(Some(0))
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    block(key, &wide, (wide.len() / 2) as u16, 1, &[])
}

fn block(
    key: &str,
    value: &[u8],
    value_length: u16,
    kind: u16,
    children: &[Vec<u8>],
) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; 6];
    write_u16(&mut bytes, 2, value_length)?;
    write_u16(&mut bytes, 4, kind)?;
    bytes.extend(key.encode_utf16().chain(Some(0)).flat_map(u16::to_le_bytes));
    pad_block(&mut bytes);
    bytes.extend_from_slice(value);
    for child in children {
        pad_block(&mut bytes);
        bytes.extend_from_slice(child);
    }
    let length = u16::try_from(bytes.len()).map_err(|_| invalid("version block length"))?;
    write_u16(&mut bytes, 0, length)?;
    Ok(bytes)
}

fn pad_block(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

fn align(value: u32, alignment: u32) -> io::Result<u32> {
    if !alignment.is_power_of_two() || alignment > 65_536 {
        return Err(invalid("unexpected PE alignment"));
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(|| invalid("aligned PE size"))
}

fn read_u16(bytes: &[u8], offset: usize) -> io::Result<u16> {
    bytes
        .get(offset..offset + 2)
        .map(|value| u16::from_le_bytes([value[0], value[1]]))
        .ok_or_else(|| invalid("truncated PE u16"))
}

fn read_u32(bytes: &[u8], offset: usize) -> io::Result<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|value| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
        .ok_or_else(|| invalid("truncated PE u32"))
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) -> io::Result<()> {
    bytes
        .get_mut(offset..offset + 2)
        .ok_or_else(|| invalid("truncated PE u16 destination"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> io::Result<()> {
    bytes
        .get_mut(offset..offset + 4)
        .ok_or_else(|| invalid("truncated PE u32 destination"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
