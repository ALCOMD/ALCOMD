use alcomd_application::{AccessContext, Permission, PrincipalId};

const DAEMON_RUNTIME: &str = include_str!("../../../apps/alcomd/src/m6_runtime.rs");
const APPLICATION_M7: &str = include_str!("../../alcomd-application/src/m7.rs");
const RPC_M7: &str = include_str!("../../../apps/alcomd/src/m7_rpc.rs");

#[test]
fn client_scope_is_exact_and_does_not_substitute_for_extension_authority() {
    let access = AccessContext::new(
        PrincipalId::parse("client:m7-security").expect("principal"),
        [Permission::ProjectsRead, Permission::ExtensionsUiUse],
    )
    .with_project_read_scopes(["project-a".to_owned()])
    .with_extension_ui_scopes(["dev.example.a".to_owned()]);

    assert!(access.require_project_read_scope("project-a").is_ok());
    assert!(access.require_project_read_scope("project-b").is_err());
    assert!(access.require_extension_ui_scope("dev.example.a").is_ok());
    assert!(access.require_extension_ui_scope("dev.example.b").is_err());

    assert!(DAEMON_RUNTIME.contains("require_project_read_scope(project_id)"));
    assert!(DAEMON_RUNTIME.contains("authority.project_summary("));
    assert!(DAEMON_RUNTIME.contains("require_extension_ui_scope(&lease.extension_id)"));
    assert!(DAEMON_RUNTIME.contains("authority.data_get(lease"));
    assert!(DAEMON_RUNTIME.contains("authority.data_set(lease"));
    assert!(DAEMON_RUNTIME.contains("authority.data_delete(lease"));
}

#[test]
fn invocation_context_and_host_binding_fail_closed() {
    assert!(DAEMON_RUNTIME.contains("returned_context_id != context_id"));
    assert!(DAEMON_RUNTIME.contains("lease_id != process.lease.lease_id"));
    assert!(DAEMON_RUNTIME.contains("!bound(&message, &process.lease"));
    assert!(DAEMON_RUNTIME.contains("invocation_cancelled(&invocation)"));
    assert!(DAEMON_RUNTIME.contains("terminate(&mut process.child)"));
    assert!(APPLICATION_M7.contains("!authority.is_current()"));
}

#[test]
fn sensitive_ui_payload_has_no_production_logging_sink() {
    for source in [APPLICATION_M7, RPC_M7, DAEMON_RUNTIME] {
        for forbidden_sink in [
            "println!(",
            "eprintln!(",
            "tracing::trace!(",
            "tracing::debug!(",
            "tracing::info!(",
            "tracing::warn!(",
            "tracing::error!(",
            "log::",
        ] {
            assert!(
                !source.contains(forbidden_sink),
                "Portable UI production path contains raw-capable logging sink {forbidden_sink}"
            );
        }
    }
    assert!(APPLICATION_M7.contains("session.replay.clear()"));
    assert!(APPLICATION_M7.contains("session.current_document.clear()"));
    assert!(
        APPLICATION_M7
            .contains("write!(formatter, \"Portable UI request failed: {:?}\", self.code)")
    );
}
