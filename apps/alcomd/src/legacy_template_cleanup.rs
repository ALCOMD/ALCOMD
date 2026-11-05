use std::fs;
use std::io;
use std::path::Path;

pub(super) fn retire(data_root: &Path) -> io::Result<()> {
    let (root, identity) = alcomd_platform::resolve_directory_identity(data_root)?;
    for (name, validate) in [
        (
            "template-store",
            validate_store as fn(&Path) -> io::Result<()>,
        ),
        ("template-builtin-staging", validate_builtins),
    ] {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => continue,
            other => {
                if !plain_directory(&other?) {
                    return Err(unverified(&path));
                }
            }
        }
        let (resolved, original_identity) = alcomd_platform::resolve_directory_identity(&path)?;
        if resolved != path {
            return Err(unverified(&path));
        }
        validate(&path)?;
        if alcomd_platform::file_identity_key(&root)? != identity
            || alcomd_platform::file_identity_key(&path)? != original_identity
        {
            return Err(unverified(&path));
        }
        alcomd_platform::project_delete_cleanup(&path)
            .map_err(|cause| io::Error::other(format!("{}: {cause}", path.display())))?;
    }
    Ok(())
}

fn validate_store(path: &Path) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        if !plain_directory(&fs::symlink_metadata(&child)?) {
            return Err(unverified(&child));
        }
        match entry.file_name().to_str() {
            Some("objects") => validate_entries(&child, |name, metadata| {
                plain_file(metadata)
                    && (name.strip_suffix(".alcomdtemplate").is_some_and(sha256)
                        || name
                            .strip_prefix('.')
                            .and_then(|name| name.split_once(".partial."))
                            .is_some_and(|(digest, suffix)| {
                                sha256(digest) && counters(suffix, '.')
                            }))
            })?,
            Some("staging") => validate_entries(&child, |name, metadata| {
                (plain_directory(metadata) && name.strip_prefix("create-plan-").is_some_and(uuid))
                    || (plain_file(metadata)
                        && name
                            .strip_prefix("derive-")
                            .and_then(|name| name.strip_suffix(".partial"))
                            .and_then(|name| name.split_at_checked(36))
                            .is_some_and(|(id, suffix)| {
                                uuid(id)
                                    && suffix
                                        .strip_prefix('-')
                                        .is_some_and(|tail| counters(tail, '-'))
                            }))
            })?,
            _ => return Err(unverified(&child)),
        }
    }
    Ok(())
}

fn validate_builtins(path: &Path) -> io::Result<()> {
    validate_entries(path, |name, metadata| {
        plain_file(metadata) && name.strip_suffix(".alcomdtemplate").is_some_and(uuid)
    })
}

fn validate_entries(path: &Path, valid: impl Fn(&str, &fs::Metadata) -> bool) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let metadata = fs::symlink_metadata(&child)?;
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| valid(name, &metadata))
        {
            return Err(unverified(&child));
        }
    }
    Ok(())
}

fn plain_directory(metadata: &fs::Metadata) -> bool {
    metadata.is_dir() && !link_or_reparse(metadata)
}

fn plain_file(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !link_or_reparse(metadata)
}

#[cfg(windows)]
fn link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn uuid(value: &str) -> bool {
    alcomd_application::OperationId::parse(value).is_ok()
}

fn counters(value: &str, separator: char) -> bool {
    value.split_once(separator).is_some_and(|(first, second)| {
        first.parse::<u32>().is_ok() && second.parse::<u64>().is_ok()
    })
}

fn unverified(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("Unverified template artifact: {}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retire_only_removes_verified_managed_template_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "alcomd-retire-template-test-{}",
            alcomd_application::OperationId::new(),
        ));
        fs::create_dir(&root).expect("isolated data root");
        let store = root.join("template-store");
        let objects = store.join("objects");
        let builtin = root.join("template-builtin-staging");
        fs::create_dir_all(&objects).expect("old object store");
        fs::create_dir(&builtin).expect("old builtin stage");
        fs::write(
            objects.join(format!("{}.alcomdtemplate", "1".repeat(64))),
            b"old",
        )
        .expect("old object");
        fs::write(store.join("personal.txt"), b"keep").expect("unverified file");
        let cache = root.join("package-cache");
        fs::create_dir(&cache).expect("shared cache");
        fs::write(cache.join("package.zip"), b"keep").expect("shared package");
        let exported = root.join("exported.alcomdtemplate");
        fs::write(&exported, b"keep").expect("user export");
        assert!(retire(&root).is_err());
        assert!(objects.exists());
        fs::remove_file(store.join("personal.txt")).expect("remove synthetic unverified file");
        retire(&root).expect("retire old managed data");
        assert!(!store.exists());
        assert!(!builtin.exists());
        assert_eq!(
            fs::read(cache.join("package.zip")).expect("shared cache kept"),
            b"keep"
        );
        assert_eq!(fs::read(exported).expect("user export kept"), b"keep");
        fs::remove_dir_all(root).expect("remove isolated fixture");
    }
}
