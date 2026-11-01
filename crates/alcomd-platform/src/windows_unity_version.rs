//! Bounded, read-only Windows version-resource inspection for Unity Editor.
//!
//! Safe Rust has no executable version-resource API. This module contains only
//! the three existing version.dll bindings; it never loads or executes a file.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::mem::{align_of, size_of};
use std::ops::Range;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};

const MAX_VERSION_INFO_BYTES: usize = 1_048_576;
const MAX_PATH_UNITS: usize = 32_767;
const MAX_TRANSLATIONS: usize = 64;

/// Reads the Editor's declared ProductVersion without executing the Editor.
/// Filesystem identity and canonical Unity-version validation belong to callers.
pub(super) fn read_unity_product_version(path: &Path) -> io::Result<String> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unity executable path must be absolute",
        ));
    }
    let mut wide_path = path
        .as_os_str()
        .encode_wide()
        .take(MAX_PATH_UNITS + 1)
        .collect::<Vec<_>>();
    if wide_path.is_empty() || wide_path.len() > MAX_PATH_UNITS || wide_path.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unity executable path is invalid",
        ));
    }
    wide_path.push(0);

    // SAFETY: wide_path is an owned, nul-terminated UTF-16 absolute filename
    // that remains live throughout this call. The optional output is null.
    let byte_len = unsafe { GetFileVersionInfoSizeW(wide_path.as_ptr(), null_mut()) };
    if byte_len == 0 {
        return Err(io::Error::last_os_error());
    }
    let byte_len = byte_len as usize;
    if byte_len > MAX_VERSION_INFO_BYTES {
        return Err(invalid_data(
            "Unity version resource exceeds its size limit",
        ));
    }
    // u32 storage supplies the DWORD alignment required by version resources.
    // Zero initialization also keeps the final rounded word fully initialized.
    let mut resource = VersionResource {
        words: vec![0_u32; byte_len.div_ceil(size_of::<u32>())],
        byte_len,
    };
    // SAFETY: the filename remains valid and nul-terminated; words is a live,
    // writable DWORD-aligned allocation of at least byte_len bytes. The length
    // is exactly the successful, bounded GetFileVersionInfoSizeW result.
    let read = unsafe {
        GetFileVersionInfoW(
            wide_path.as_ptr(),
            0,
            byte_len as u32,
            resource.words.as_mut_ptr().cast::<c_void>(),
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }

    let translations = resource.query_bytes("\\VarFileInfo\\Translation", ValueUnits::Bytes)?;
    if translations.is_empty()
        || !translations.len().is_multiple_of(4)
        || translations.len() / 4 > MAX_TRANSLATIONS
    {
        return Err(invalid_data("Unity version translations are invalid"));
    }
    let mut version = None;
    for translation in translations.chunks_exact(4) {
        let language = u16::from_le_bytes([translation[0], translation[1]]);
        let code_page = u16::from_le_bytes([translation[2], translation[3]]);
        let prefix = format!("\\StringFileInfo\\{language:04x}{code_page:04x}");
        let product_name = resource.query_string(&format!("{prefix}\\ProductName"))?;
        require_unity_product(&product_name)?;
        let observed = resource.query_string(&format!("{prefix}\\ProductVersion"))?;
        if version.as_ref().is_some_and(|version| version != &observed) {
            return Err(invalid_data(
                "Unity ProductVersion differs between declared translations",
            ));
        }
        version = Some(observed);
    }
    version.ok_or_else(|| invalid_data("Unity ProductVersion is missing"))
}

struct VersionResource {
    words: Vec<u32>,
    byte_len: usize,
}

#[derive(Clone, Copy)]
enum ValueUnits {
    Bytes,
    Utf16,
}

impl VersionResource {
    fn query_bytes(&self, key: &str, units: ValueUnits) -> io::Result<&[u8]> {
        let wide_key = key.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mut value = null_mut();
        let mut length = 0;
        // SAFETY: words exclusively owns a successful GetFileVersionInfoW
        // result and remains live, aligned and unchanged during this read-only
        // query. wide_key is nul-terminated; both outputs are valid local slots.
        // The returned borrowed pointer is never dereferenced directly.
        let found = unsafe {
            VerQueryValueW(
                self.words.as_ptr().cast::<c_void>(),
                wide_key.as_ptr(),
                &mut value,
                &mut length,
            )
        };
        if found == 0 {
            return Err(invalid_data(
                "Unity version resource value is missing or invalid",
            ));
        }
        let byte_length = match units {
            ValueUnits::Bytes => length as usize,
            ValueUnits::Utf16 => (length as usize)
                .checked_mul(size_of::<u16>())
                .ok_or_else(|| invalid_data("Unity version string length overflows"))?,
        };
        let range = checked_value_range(
            self.words.as_ptr().addr(),
            self.byte_len,
            value.addr(),
            byte_length,
        )?;
        // SAFETY: every u32 bit pattern is valid and the storage was fully zero
        // initialized before the API wrote to it. byte_len is within this owned
        // allocation. The checked range below is inside those initialized bytes;
        // its returned borrow cannot outlive self and no mutable access occurs.
        let bytes =
            unsafe { std::slice::from_raw_parts(self.words.as_ptr().cast::<u8>(), self.byte_len) };
        Ok(&bytes[range])
    }

    fn query_string(&self, key: &str) -> io::Result<String> {
        decode_resource_string(self.query_bytes(key, ValueUnits::Utf16)?)
    }
}

fn checked_value_range(
    base: usize,
    available: usize,
    value: usize,
    length: usize,
) -> io::Result<Range<usize>> {
    if value == 0 || length == 0 || !value.is_multiple_of(align_of::<u16>()) {
        return Err(invalid_data("Unity version resource pointer is invalid"));
    }
    let start = value
        .checked_sub(base)
        .ok_or_else(|| invalid_data("Unity version resource pointer precedes its buffer"))?;
    let end = start
        .checked_add(length)
        .filter(|end| *end <= available)
        .ok_or_else(|| invalid_data("Unity version resource value exceeds its buffer"))?;
    Ok(start..end)
}

fn decode_resource_string(bytes: &[u8]) -> io::Result<String> {
    if bytes.len() < size_of::<u16>() || !bytes.len().is_multiple_of(size_of::<u16>()) {
        return Err(invalid_data(
            "Unity version resource UTF-16 length is invalid",
        ));
    }
    let mut words = bytes
        .chunks_exact(2)
        .map(|word| u16::from_le_bytes([word[0], word[1]]))
        .collect::<Vec<_>>();
    if words.pop() != Some(0) || words.is_empty() || words.contains(&0) {
        return Err(invalid_data(
            "Unity version resource string is not terminated correctly",
        ));
    }
    String::from_utf16(&words)
        .map_err(|_| invalid_data("Unity version resource string contains invalid UTF-16"))
}

fn require_unity_product(name: &str) -> io::Result<()> {
    if name == "Unity" {
        Ok(())
    } else {
        Err(invalid_data("Executable ProductName is not Unity Editor"))
    }
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::{checked_value_range, decode_resource_string, require_unity_product};

    #[test]
    fn version_value_range_accepts_only_aligned_owned_bytes() {
        assert_eq!(
            checked_value_range(100, 12, 102, 10).expect("owned bytes"),
            2..12
        );
        for (value, length) in [(0, 2), (98, 2), (101, 2), (102, 0), (112, 2), (110, 4)] {
            assert!(checked_value_range(100, 12, value, length).is_err());
        }
    }

    #[test]
    fn version_value_range_rejects_integer_overflow() {
        assert!(checked_value_range(2, 12, usize::MAX - 1, 4).is_err());
        assert!(checked_value_range(100, 12, 102, usize::MAX).is_err());
    }

    #[test]
    fn version_string_preserves_the_declared_product_version() {
        let expected = "2022.3.22f1_887be4894c44";
        let bytes = expected
            .encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            decode_resource_string(&bytes).expect("valid version"),
            expected
        );
    }

    #[test]
    fn version_string_rejects_invalid_utf16_and_terminators() {
        for bytes in [
            vec![],
            vec![0],
            vec![0, 0],
            vec![b'U', 0],
            vec![b'U', 0, 0, 0, b'X', 0, 0, 0],
            vec![0, 0xD8, 0, 0],
        ] {
            assert!(decode_resource_string(&bytes).is_err());
        }
    }

    #[test]
    fn product_identity_rejects_hub_and_unrelated_executables() {
        require_unity_product("Unity").expect("Editor product");
        for product in ["Unity Hub", "UnityInstaller", "cmd", "", " Unity", "Unity "] {
            assert!(require_unity_product(product).is_err());
        }
    }
}
