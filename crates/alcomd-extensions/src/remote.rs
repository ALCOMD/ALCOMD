//! A single extension's version index. No Unity/VPM manifest parser is used.
use alcomd_application::{
    ExtensionRemoteSourceEvidence, ExtensionSourceFetcher, ExtensionSourcePreview,
    ExtensionSourceVersion, M6Error, M6ErrorCode,
};
use serde::{
    Deserialize, Serialize,
    de::{Error, MapAccess, Visitor},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
struct UniqueMap<T>(BTreeMap<String, T>);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for UniqueMap<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for MapVisitor<T> {
            type Value = UniqueMap<T>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a map with unique keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, T>()? {
                    if values.insert(key, value).is_some() {
                        return Err(A::Error::custom("duplicate source key"));
                    }
                }
                Ok(UniqueMap(values))
            }
        }
        deserializer.deserialize_map(MapVisitor(std::marker::PhantomData))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    format: String,
    packages: UniqueMap<Package>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Package {
    display_name: String,
    publisher_fingerprint: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    description: Option<String>,
    versions: UniqueMap<Version>,
}
fn optional_text<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Version {
    pub api_major: u32,
    pub url: String,
    #[serde(rename = "zipSHA256")]
    pub zip_sha256: String,
}
pub(crate) struct Snapshot {
    pub preview: ExtensionSourcePreview,
    package: Package,
}
impl Snapshot {
    pub fn selected(&self, version: &str) -> Result<&Version, M6Error> {
        self.package.versions.0.get(version).ok_or_else(invalid)
    }
}
pub(crate) async fn read(
    fetcher: &dyn ExtensionSourceFetcher,
    url: &str,
) -> Result<Snapshot, M6Error> {
    let response = fetcher.index(url).await?;
    parse(fetcher, &response.url, &response.bytes)
}
fn parse(
    fetcher: &dyn ExtensionSourceFetcher,
    url: &str,
    bytes: &[u8],
) -> Result<Snapshot, M6Error> {
    if bytes.is_empty() || bytes.len() > 1_048_576 {
        return Err(invalid());
    }
    // Do not retain parser errors containing attacker-controlled body/URL text.
    let mut document: Document = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if document.format != "alcomd-extension-source-v1" || document.packages.0.len() != 1 {
        return Err(invalid());
    }
    let (id, package) = document.packages.0.iter_mut().next().ok_or_else(invalid)?;
    if !crate::package::valid_extension_id(id)
        || package.display_name.trim().is_empty()
        || package.display_name.chars().count() > 120
        || package.display_name.chars().any(char::is_control)
        || !crate::package::valid_fingerprint(&package.publisher_fingerprint)
        || package
            .description
            .as_ref()
            .is_some_and(|v| v.len() > 2048 || v.contains('\0'))
        || package.versions.0.is_empty()
        || package.versions.0.len() > 256
    {
        return Err(invalid());
    }
    let mut versions = Vec::new();
    for (version, item) in &mut package.versions.0 {
        let parsed = semver::Version::parse(version).map_err(|_| invalid())?;
        if version.len() > 128 || item.api_major == 0 {
            return Err(invalid());
        }
        decode_digest(&item.zip_sha256)?;
        item.url = fetcher.normalize_url(&item.url)?;
        versions.push((
            parsed,
            ExtensionSourceVersion {
                version: version.clone(),
                api_major: item.api_major,
            },
        ));
    }
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    let mut hash = Sha256::new();
    hash.update(b"ALCOMD-EXT-SOURCE-V1\0");
    hash.update(url.as_bytes());
    hash.update([0]);
    hash.update(serde_json::to_vec(&document).map_err(|_| invalid())?);
    let fingerprint = hex(&hash.finalize());
    let (id, package) = document.packages.0.into_iter().next().ok_or_else(invalid)?;
    Ok(Snapshot {
        preview: ExtensionSourcePreview {
            url: url.into(),
            extension_id: id,
            display_name: package.display_name.clone(),
            publisher_fingerprint: package.publisher_fingerprint.clone(),
            description: package.description.clone(),
            versions: versions.into_iter().map(|(_, v)| v).collect(),
            source_fingerprint: fingerprint,
        },
        package,
    })
}
pub(crate) fn decode_digest(text: &str) -> Result<[u8; 32], M6Error> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid());
    }
    let mut result = [0; 32];
    for (slot, pair) in result.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *slot = u8::from_str_radix(std::str::from_utf8(pair).map_err(|_| invalid())?, 16)
            .map_err(|_| invalid())?;
    }
    Ok(result)
}
pub(crate) fn plan_fingerprint(
    digest: &[u8; 32],
    source: &ExtensionRemoteSourceEvidence,
) -> Result<[u8; 32], M6Error> {
    let mut hash = Sha256::new();
    hash.update(b"ALCOMD-EXT-REMOTE-PLAN-V1\0");
    hash.update(digest);
    hash.update(serde_json::to_vec(source).map_err(|_| invalid())?);
    Ok(hash.finalize().into())
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn invalid() -> M6Error {
    M6Error::new(M6ErrorCode::SourceInvalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{ExtensionSourceBytes, ExtensionSourceFuture};
    struct Fetcher;
    impl ExtensionSourceFetcher for Fetcher {
        fn normalize_url(&self, url: &str) -> Result<String, M6Error> {
            if !url.starts_with("https://") || url.contains('#') {
                return Err(invalid());
            }
            Ok(url.to_owned())
        }
        fn index<'a>(&'a self, _: &'a str) -> ExtensionSourceFuture<'a, ExtensionSourceBytes> {
            Box::pin(async { Err(invalid()) })
        }
        fn archive<'a>(
            &'a self,
            _: &'a str,
            _: [u8; 32],
        ) -> ExtensionSourceFuture<'a, std::path::PathBuf> {
            Box::pin(async { Err(invalid()) })
        }
    }
    fn doc() -> serde_json::Value {
        serde_json::json!({"format":"alcomd-extension-source-v1", "packages":{"dev.example.extension":{
            "displayName":"Example", "publisherFingerprint":format!("ed25519-sha256:{}", "a".repeat(64)),
            "versions":{"1.2.0":{"apiMajor":1,"url":"https://example.invalid/file?b=2&a=1&a=3", "zipSHA256":"a".repeat(64)}}
        }}})
    }
    fn read_doc(value: &serde_json::Value) -> Result<Snapshot, M6Error> {
        parse(
            &Fetcher,
            "https://example.invalid/index?z=3&z=2",
            &serde_json::to_vec(value).expect("JSON"),
        )
    }
    #[test]
    fn source_preserves_query_and_canonicalizes_json_not_addresses() {
        let value = doc();
        let snapshot = read_doc(&value).expect("valid source");
        assert_eq!(
            snapshot.selected("1.2.0").expect("version").url,
            "https://example.invalid/file?b=2&a=1&a=3"
        );
        assert_eq!(
            snapshot.preview.url,
            "https://example.invalid/index?z=3&z=2"
        );
        let pretty = serde_json::to_vec_pretty(&value).expect("pretty JSON");
        assert_eq!(
            snapshot.preview.source_fingerprint,
            parse(&Fetcher, &snapshot.preview.url, &pretty)
                .expect("pretty source")
                .preview
                .source_fingerprint
        );
        assert_ne!(
            snapshot.preview.source_fingerprint,
            parse(&Fetcher, "https://example.invalid/index?z=2&z=3", &pretty)
                .expect("other source")
                .preview
                .source_fingerprint
        );
        let mut changed = value;
        changed["packages"]["dev.example.extension"]["displayName"] = "Changed".into();
        assert_ne!(
            snapshot.preview.source_fingerprint,
            read_doc(&changed)
                .expect("changed source")
                .preview
                .source_fingerprint
        );
    }
    #[test]
    fn source_rejects_duplicate_keys_multiple_packages_unknown_fields_and_bad_identity() {
        let raw = serde_json::to_string(&doc()).expect("JSON");
        for duplicate in [raw.replacen("\"format\":", "\"format\":\"alcomd-extension-source-v1\",\"format\":",1),raw.replace("\"apiMajor\":1", "\"apiMajor\":1,\"apiMajor\":1"),raw.replace("\"versions\":{", "\"versions\":{\"1.2.0\":{\"apiMajor\":1,\"url\":\"https://example.invalid/f\",\"zipSHA256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"},")] {
            assert!(parse(&Fetcher,"https://example.invalid/index",duplicate.as_bytes()).is_err());
        }
        for key in ["BAD ID", "dev.example.other"] {
            let mut value = doc();
            let package = value["packages"]["dev.example.extension"].clone();
            value["packages"][key] = package;
            assert!(read_doc(&value).is_err());
        }
        let mut null_description = doc();
        null_description["packages"]["dev.example.extension"]["description"] =
            serde_json::Value::Null;
        assert!(read_doc(&null_description).is_err());
        let mut value = doc();
        value["packages"]["dev.example.extension"]["unexpected"] = true.into();
        assert!(read_doc(&value).is_err());
        let mut value = doc();
        value["packages"]["dev.example.extension"]["publisherFingerprint"] = "invalid".into();
        assert!(read_doc(&value).is_err());
    }
    #[test]
    fn source_enforces_bytes_semver_hash_and_version_count() {
        assert!(
            parse(
                &Fetcher,
                "https://example.invalid/index",
                &vec![b' '; 1_048_577]
            )
            .is_err()
        );
        for bad in ["version 1", "v1.0.0", "01.0.0"] {
            let mut v = doc();
            let entry = v["packages"]["dev.example.extension"]["versions"]["1.2.0"].take();
            v["packages"]["dev.example.extension"]["versions"] = serde_json::json!({bad:entry});
            assert!(read_doc(&v).is_err());
        }
        let mut v = doc();
        v["packages"]["dev.example.extension"]["versions"]["1.2.0"]["zipSHA256"] =
            "A".repeat(64).into();
        assert!(read_doc(&v).is_err());
        let mut v = doc();
        let entry = v["packages"]["dev.example.extension"]["versions"]["1.2.0"].clone();
        let versions = v["packages"]["dev.example.extension"]["versions"]
            .as_object_mut()
            .expect("versions");
        versions.clear();
        for n in 0..257 {
            versions.insert(format!("1.0.{n}"), entry.clone());
        }
        assert!(read_doc(&v).is_err());
        let mut v = doc();
        v["packages"]["dev.example.extension"]["description"] = "界".repeat(683).into();
        assert!(read_doc(&v).is_err());
    }
    #[test]
    fn source_versions_are_semver_descending_and_future_api_is_visible() {
        let mut v = doc();
        let mut entry = v["packages"]["dev.example.extension"]["versions"]["1.2.0"].clone();
        for version in ["1.10.0", "2.0.0-beta.1", "2.0.0"] {
            v["packages"]["dev.example.extension"]["versions"][version] = entry.clone();
        }
        entry["apiMajor"] = 2.into();
        v["packages"]["dev.example.extension"]["versions"]["3.0.0"] = entry;
        let snapshot = read_doc(&v).expect("version index");
        assert_eq!(
            snapshot
                .preview
                .versions
                .iter()
                .map(|v| v.version.as_str())
                .collect::<Vec<_>>(),
            ["3.0.0", "2.0.0", "2.0.0-beta.1", "1.10.0", "1.2.0"]
        );
        assert_eq!(snapshot.preview.versions[0].api_major, 2);
    }
    #[test]
    fn plan_fingerprint_binds_archive_and_exact_source_evidence() {
        let evidence = ExtensionRemoteSourceEvidence {
            url: "https://example.invalid/i?a=1&b=2".into(),
            version: "1.0.0".into(),
            source_fingerprint: "a".repeat(64),
            artifact_url: "https://example.invalid/f".into(),
            artifact_sha256: "b".repeat(64),
        };
        let first = plan_fingerprint(&[1; 32], &evidence).expect("fingerprint");
        let mut changed = evidence;
        changed.url = "https://example.invalid/i?b=2&a=1".into();
        assert_ne!(
            first,
            plan_fingerprint(&[1; 32], &changed).expect("fingerprint")
        );
        assert_ne!(
            first,
            plan_fingerprint(&[2; 32], &changed).expect("fingerprint")
        );
    }
}
