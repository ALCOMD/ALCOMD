use std::io;

fn desktop_entry(name: &str, image: &str) -> io::Result<String> {
    if name.chars().any(char::is_control)
        || !image.starts_with('/')
        || image.contains('=')
        || image.chars().any(char::is_control)
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // Desktop strings are unescaped before Exec arguments are parsed.
    let image = image
        .replace('\\', r"\\\\")
        .replace('"', r#"\\""#)
        .replace('$', r"\\$")
        .replace('`', r"\\`")
        .replace('%', "%%");
    let name = name.replace('\\', r"\\");
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec=\"{image}\" %u\nNoDisplay=true\nTerminal=false\nMimeType=x-scheme-handler/vcc;\n"
    ))
}

#[cfg(target_os = "linux")]
pub(super) fn set_registration(
    applications: &std::path::Path,
    identifier: &str,
    name: &str,
    image: &std::path::Path,
    enabled: bool,
) -> io::Result<()> {
    use std::{fs, io::Write};
    if identifier.is_empty()
        || !identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let path = applications.join(format!("{identifier}.vcc.desktop"));
    if !enabled {
        match fs::remove_file(&path) {
            Ok(()) => return update_cache(applications),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
    }
    let image = image.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    let contents = desktop_entry(name, image)?;
    fs::create_dir_all(applications)?;
    if !fs::read_to_string(&path).is_ok_and(|existing| existing == contents) {
        let temporary = applications.join(format!(".{identifier}.{}.tmp", uuid::Uuid::new_v4()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let written = file.write_all(contents.as_bytes());
        drop(file);
        if let Err(error) = written.and_then(|()| fs::rename(&temporary, path)) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    }
    update_cache(applications)
}

#[cfg(target_os = "linux")]
fn update_cache(applications: &std::path::Path) -> io::Result<()> {
    let status = std::process::Command::new("update-desktop-database")
        .arg(applications)
        .status()?;
    if !status.success() {
        return Err(io::Error::other("desktop MIME cache update failed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_binds_only_the_vcc_mime_type_and_quotes_the_image() {
        let contents = desktop_entry("ALCOMD", r#"/home/user/App Files/A% $`"\B.AppImage"#)
            .expect("valid AppImage path");
        assert!(contents.contains(r#"Exec="/home/user/App Files/A%% \\$\\`\\"\\\\B.AppImage" %u"#));
        assert!(contents.contains("MimeType=x-scheme-handler/vcc;\n"));
        assert!(desktop_entry("ALCOMD", "relative.AppImage").is_err());
        assert!(desktop_entry("ALCOMD", "/tmp/a\nb.AppImage").is_err());
    }
}
