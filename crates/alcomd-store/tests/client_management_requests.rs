use alcomd_application::*;
use alcomd_store::StateStoreHandle;

async fn register(store: &StateStoreHandle, name: &str) -> (McpClient, ToolAuthentication) {
    let owner = PrincipalId::local_owner();
    let client = store
        .mcp_write(
            owner.clone(),
            format!("create-{name}"),
            McpIdentityWrite::Create {
                name: name.to_owned(),
            },
            1,
        )
        .await
        .expect("register client")
        .client;
    let hash = mcp_credential_hash(name);
    let client = store
        .mcp_write(
            owner,
            format!("issue-{name}"),
            McpIdentityWrite::Issue {
                id: client.client_id,
                expected: client.revision,
                hash,
            },
            2,
        )
        .await
        .expect("issue initial credential")
        .client;
    (client, ToolAuthentication::Credential(hash))
}

fn decision(r: &ClientManagementReview, key: &str) -> ClientRequestDecisionInput {
    ClientRequestDecisionInput {
        request_id: r.request_id,
        approve: true,
        expected_fingerprint: r.fingerprint.clone(),
        idempotency_key: key.to_owned(),
    }
}

#[tokio::test]
async fn independent_owner_approves_original_action_once_and_delivers_secret_once() {
    let path =
        std::env::temp_dir().join(format!("alcomd-management-{}.sqlite3", McpClientId::new()));
    let store = StateStoreHandle::open(path.clone()).expect("new store");
    let (first, auth) = register(&store, "first").await;
    let (other, other_auth) = register(&store, "other").await;
    let access = mcp_access(&first).expect("active");
    let other_access = mcp_access(&other).expect("active");
    let app = ClientRequestApplication(&store);
    let action = ClientManagementAction::IssueCredential {
        client_id: other.client_id,
        expected_revision: other.revision,
    };
    let create = || app.create(&access, action.clone(), "apply".into(), 3);
    let pending = with_workflow_authentication(auth.clone(), create())
        .await
        .expect("application");
    assert_eq!(pending.status, ClientRequestStatus::Pending);
    assert_eq!(
        with_workflow_authentication(auth.clone(), create())
            .await
            .expect("same request")
            .request_id,
        pending.request_id
    );
    assert!(matches!(
        with_workflow_authentication(
            other_auth.clone(),
            app.get(&other_access, pending.request_id, 4)
        )
        .await,
        Err(OAuthError::Denied)
    ));
    assert!(
        with_workflow_authentication(
            other_auth.clone(),
            app.list(&other_access, OAuthPageParams::default(), 4)
        )
        .await
        .expect("scoped list")
        .items
        .is_empty()
    );
    assert_eq!(
        app.list(&AccessContext::local_owner(), OAuthPageParams::default(), 4)
            .await
            .expect("owner list")
            .items
            .len(),
        1
    );
    assert!(matches!(
        with_workflow_authentication(
            auth.clone(),
            app.decide(&access, decision(&pending, "vote"), "unused".into(), 4)
        )
        .await,
        Err(OAuthError::Denied)
    ));
    let owner = AccessContext::local_owner();
    let mut altered = decision(&pending, "owner");
    altered.expected_fingerprint = "different".into();
    assert!(matches!(
        app.decide(&owner, altered, "unused".into(), 5).await,
        Err(OAuthError::RevisionConflict)
    ));
    let result = app
        .decide(&owner, decision(&pending, "owner"), "new-secret".into(), 5)
        .await
        .expect("owner approves");
    assert_eq!(result.request.status, ClientRequestStatus::Approved);
    assert_eq!(result.credential.as_deref(), Some("new-secret"));
    drop(store);
    let store = StateStoreHandle::open(path).expect("reopen current state");
    let app = ClientRequestApplication(&store);
    let replay = app
        .decide(&owner, decision(&pending, "owner"), "unused".into(), 6)
        .await
        .expect("same decision receipt");
    assert!(replay.credential.is_none() && replay.client_secret.is_none());
    assert!(matches!(
        app.decide(&owner, decision(&pending, "second"), "unused".into(), 6)
            .await,
        Err(OAuthError::RevisionConflict)
    ));
    let issued = store
        .mcp_authenticate(mcp_credential_hash("new-secret"))
        .await
        .expect("new credential");
    assert_eq!(issued.revision, other.revision + 1);
    assert!(
        store
            .mcp_authenticate(mcp_credential_hash("other"))
            .await
            .is_err()
    );
    let review =
        with_workflow_authentication(auth.clone(), app.get(&access, pending.request_id, 6))
            .await
            .expect("requester status");
    assert!(
        !serde_json::to_string(&review)
            .expect("serialize public request review")
            .contains("new-secret")
    );
    let new_request = with_workflow_authentication(
        auth.clone(),
        app.create(&access, action, "another".into(), 7),
    )
    .await
    .expect("new application");
    assert!(matches!(
        app.decide(&owner, decision(&new_request, "stale"), "unused".into(), 8)
            .await,
        Err(OAuthError::RevisionConflict)
    ));
    assert!(matches!(
        app.decide(
            &owner,
            decision(&new_request, "expired"),
            "unused".into(),
            new_request.expires_at_ms
        )
        .await,
        Err(OAuthError::Expired)
    ));
    store
        .mcp_write(
            owner.principal().clone(),
            "revoke".into(),
            McpIdentityWrite::Revoke {
                id: first.client_id,
                expected: first.revision,
            },
            9,
        )
        .await
        .expect("revoke requester");
    assert!(matches!(
        app.decide(
            &owner,
            decision(&new_request, "blocked"),
            "unused".into(),
            10
        )
        .await,
        Err(OAuthError::Denied)
    ));
    assert!(matches!(
        with_workflow_authentication(auth, app.get(&access, new_request.request_id, 10)).await,
        Err(OAuthError::Denied)
    ));
}
