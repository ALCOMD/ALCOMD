use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;
pub(crate) const ID: &str = "dev.alcomd.tools-fixture";
pub(crate) fn declarations() -> Value {
    fn item(name: &str, uses: &[&str], properties: Value, required: &[&str]) -> Value {
        json!({"name":name,"title":name,"description":format!("Disposable test {name}"),"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"outputSchema":{"type":"object"},"uses":uses})
    }
    let pid = json!({"type":"string","format":"uuid"});
    let mut items = vec![
        item(
            "echo",
            &[],
            json!({"text":{"type":"string","maxLength":128}}),
            &["text"],
        ),
        item("projects", &["projects.list"], json!({}), &[]),
        item(
            "project",
            &["projects.get"],
            json!({"project_id":pid}),
            &["project_id"],
        ),
        item(
            "unauthorized_read",
            &[],
            json!({"project_id":pid}),
            &["project_id"],
        ),
        item(
            "operation",
            &["operations.get"],
            json!({"operation_id":pid}),
            &["operation_id"],
        ),
        item("cancel", &["operations.cancel"], json!({}), &[]),
    ];
    for name in ["plan_remove", "plan_twice"] {
        items.push(item(name,&["packages.planUninstall"],json!({"project_id":pid,"expected_revision":{"type":"integer","minimum":1},"package_id":{"type":"string","minLength":1,"maxLength":128}}),&["project_id","expected_revision","package_id"]));
    }
    for name in [
        "apply",
        "bad_after_apply",
        "trap_after_apply",
        "loop_after_apply",
        "apply_twice",
    ] {
        items.push(item(name, &["packages.applyPlan"], json!({}), &[]));
    }
    json!({"version":1,"tools":items})
}
fn hex(value: &[u8]) -> String {
    value.iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn package_with_install_requirements(path: &Path, consent: bool) {
    let signing = SigningKey::from_bytes(&[23; 32]);
    let key = signing.verifying_key().to_bytes();
    let publisher = format!("ed25519-sha256:{}", hex(&Sha256::digest(key)));
    let mut manifest = format!(
        "schema=1\nid=\"{ID}\"\nname=\"Disposable tool provider\"\nversion=\"1.0.0\"\napi=1\npublisher_name=\"ALCOMD test\"\npublisher_key_fingerprint=\"{publisher}\"\nlicense=\"AGPL-3.0-only\"\n[entrypoints]\ncomponent=\"component/extension.wasm\"\n[cli]\ncommands=\"component/cli-commands.json\"\n[mcp]\ntools=\"component/mcp-tools.json\"\n[interfaces]\nrequired=[\"alcomd:extension/host-operations@1.0.0\"]\noptional=[]\n[permissions]\nrequired=[]\noptional=[\"operations.cancel\",\"operations.read\",\"packages.manage\",\"packages.read\",\"projects.read\",\"repositories.read\"]\n"
    );
    if consent {
        manifest = manifest
            .replace(
                "[permissions]\nrequired=[]",
                "[permissions]\nrequired=[\"projects.read\"]",
            )
            .replace(",\"projects.read\"", "");
        manifest.push_str("[installation_terms]\ntitle=\"Fixture terms\"\ntext=\"Use only disposable test data.\"\n");
    }
    let catalogue = serde_json::to_vec(&declarations()).expect("valid isolated Provider fixture");
    let component = include_bytes!("mcp-tools-provider-v1.wasm");
    let commands = serde_json::to_vec(&json!({"version":1,"commands":declarations()["tools"]}))
        .expect("signed CLI commands");
    let mut entries = vec![
        ("alcomd-extension.toml", manifest.as_bytes()),
        ("component/extension.wasm", component.as_slice()),
        ("component/mcp-tools.json", catalogue.as_slice()),
        ("component/cli-commands.json", commands.as_slice()),
    ];
    entries.sort_by_key(|(name, _)| *name);
    let mut digest = Sha256::new();
    digest.update(b"ALCOMD-EXT-CONTENT-SHA256-V1\0");
    for (name, bytes) in &entries {
        digest.update(
            u32::try_from(name.len())
                .expect("valid isolated Provider fixture")
                .to_le_bytes(),
        );
        digest.update(name.as_bytes());
        digest.update(
            u64::try_from(bytes.len())
                .expect("valid isolated Provider fixture")
                .to_le_bytes(),
        );
        digest.update(bytes);
    }
    let digest = digest.finalize();
    let mut message = b"ALCOMD-EXT-SIGNATURE-V1\0".to_vec();
    message.extend_from_slice(&digest);
    let signature = json!({"formatVersion":1,"algorithm":"ed25519","packageDigest":hex(&digest),"publicKey":hex(&key),"publisherFingerprint":publisher,"signature":hex(&signing.sign(&message).to_bytes())});
    let mut zip =
        zip::ZipWriter::new(std::fs::File::create(path).expect("valid isolated Provider fixture"));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in entries {
        zip.start_file(name, options)
            .expect("valid isolated Provider fixture");
        zip.write_all(bytes)
            .expect("valid isolated Provider fixture");
    }
    zip.start_file("META-INF/alcomd-signature-v1.json", options)
        .expect("valid isolated Provider fixture");
    zip.write_all(signature.to_string().as_bytes())
        .expect("valid isolated Provider fixture");
    zip.finish().expect("valid isolated Provider fixture");
}
