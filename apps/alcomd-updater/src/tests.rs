use super::*;
use ed25519_dalek::{Signer, SigningKey};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("alcomd-signed-payload-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("isolated payload directory");
        Self(fs::canonicalize(path).expect("canonical isolated path"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("test payload cleanup: {error}");
        }
    }
}

fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sign_candidate(root: &Path, key: &SigningKey, channel: Channel, target: &str) {
    let product: Product = toml::from_str(PRODUCT).expect("user product metadata");
    let files = RUNTIME_BINARY_ROLES
        .iter()
        .map(|role| {
            let binary = product.binaries.get(*role).expect("binary name");
            let name = format!("{binary}{}", std::env::consts::EXE_SUFFIX);
            let data = name.as_bytes();
            fs::write(root.join(&name), data).expect("fixture binary");
            serde_json::json!({"path": name, "size": data.len(), "sha256": hexadecimal(&Sha256::digest(data))})
        })
        .collect::<Vec<_>>();
    let manifest = serde_json::to_vec(&serde_json::json!({
        "schema": 1, "productId": product.identity.bundle_identifier, "version": "4.0.0",
        "channel": channel, "target": target, "files": files,
    }))
    .expect("fixture manifest");
    fs::write(root.join(MANIFEST), &manifest).expect("manifest");
    fs::write(
        root.join(SIGNATURE),
        hexadecimal(&key.sign(&manifest).to_bytes()),
    )
    .expect("signature");
}

#[test]
fn candidate_verification_rejects_signature_digest_channel_and_extra_files() {
    let stage = Fixture::new();
    let key = SigningKey::from_bytes(&[19; 32]);
    let wrong = SigningKey::from_bytes(&[3; 32]).verifying_key();
    let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    sign_candidate(&stage.0, &key, Channel::Stable, &target);
    let verified = verify_payload(&stage.0, Channel::Stable, &key.verifying_key())
        .expect("signed staged candidate");
    assert_eq!(verified.version().major, 4);
    let daemon = verified.binary("daemon").expect("verified core");
    let name = daemon
        .file_name()
        .and_then(|name| name.to_str())
        .expect("binary name");
    assert_eq!(
        verified.file_sha256(name),
        Some(Sha256::digest(name.as_bytes()).into())
    );
    assert_eq!(verified.file_sha256(MANIFEST), None);

    assert!(verify_payload(&stage.0, Channel::Stable, &wrong).is_err());
    assert!(verify_payload(&stage.0, Channel::Beta, &key.verifying_key()).is_err());

    let extra = stage.0.join("unlisted.dll");
    fs::write(&extra, b"unlisted").expect("unexpected payload file");
    assert!(verify_payload(&stage.0, Channel::Stable, &key.verifying_key()).is_err());
    fs::remove_file(extra).expect("remove isolated test extra");
    fs::write(verified.binary("daemon").expect("binary"), b"tampered").expect("corrupt binary");
    assert!(verify_payload(&stage.0, Channel::Stable, &key.verifying_key()).is_err());
}

#[test]
fn platform_health_signature_cannot_be_reused_from_another_file_or_candidate() {
    let key = SigningKey::from_bytes(&[5; 32]);
    let report = br#"{"candidate":"isolated fixture","gui":true}"#;
    let mut message = b"ALCOMD bridge health v1\0".to_vec();
    message.extend_from_slice(report);
    let raw_signature = hexadecimal(&key.sign(report).to_bytes());
    let report_signature = hexadecimal(&key.sign(&message).to_bytes());
    assert!(
        verify_health_report_with_key(report, raw_signature.as_bytes(), &key.verifying_key())
            .is_err()
    );
    assert!(
        verify_health_report_with_key(report, report_signature.as_bytes(), &key.verifying_key())
            .is_ok()
    );
    assert!(
        verify_health_report_with_key(
            b"other report",
            report_signature.as_bytes(),
            &key.verifying_key()
        )
        .is_err()
    );
}
