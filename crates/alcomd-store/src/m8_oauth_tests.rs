use super::*;
use std::path::PathBuf;

fn entropy(n: u64) -> OAuthEntropy {
    OAuthEntropy {
        id: McpClientId::new().to_string(),
        secret: format!("{n:064x}"),
        extra: format!("{:064x}", n + 10000),
    }
}
fn path() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-oauth-tests")
        .join(McpClientId::new().to_string());
    std::fs::create_dir_all(&dir).expect("disposable test directory");
    dir.join("state.db")
}
async fn seed(store: &StateStoreHandle, name: &str) -> McpClient {
    store
        .mcp_write(
            PrincipalId::local_owner(),
            format!("create-{name}"),
            McpIdentityWrite::Create { name: name.into() },
            100,
        )
        .await
        .expect("client")
        .client
}
async fn register(
    service: &OAuthApplication<StateStoreHandle>,
    id: McpClientId,
    confidential: bool,
) -> OAuthRegistrationResult {
    service
        .set_registration(
            &AccessContext::local_owner(),
            OAuthRegistrationSetParams {
                client_id: id.to_string(),
                issuer: oauth_issuer(5443),
                resource: format!("{}/mcp", oauth_issuer(5443)),
                redirect_uris: vec!["http://127.0.0.1:3456/callback".into()],
                confidential,
                allow_refresh: true,
                expected_revision: 0,
                idempotency_key: format!("register-{id}"),
            },
            entropy(1),
            101,
        )
        .await
        .expect("register")
}
#[tokio::test]
async fn registration_mutations_bind_idempotency_and_revocation_to_the_authenticated_actor() {
    let store = StateStoreHandle::open(path()).expect("store");
    let service = OAuthApplication::new(store.clone(), [4; 32]);
    let caller = seed(&store, "administrator").await;
    let hash = mcp_credential_hash("isolated administrator credential");
    let issued = store
        .mcp_write(
            PrincipalId::local_owner(),
            "issue-admin".into(),
            McpIdentityWrite::Issue {
                id: caller.client_id,
                expected: caller.revision,
                hash,
            },
            101,
        )
        .await
        .expect("credential");
    let target = seed(&store, "registration").await;
    let request = OAuthRegistrationSetParams {
        client_id: target.client_id.to_string(),
        issuer: oauth_issuer(5443),
        resource: format!("{}/mcp", oauth_issuer(5443)),
        redirect_uris: vec!["http://127.0.0.1:3456/callback".into()],
        confidential: false,
        allow_refresh: true,
        expected_revision: 0,
        idempotency_key: "registration".into(),
    };
    let access = mcp_access(&caller).expect("actor");
    let set = || {
        with_workflow_authentication(
            ToolAuthentication::Credential(hash),
            service.set_registration(&access, request.clone(), entropy(4), 102),
        )
    };
    let original = set().await.expect("MCP administrator");
    let replay = set().await.expect("same caller and key");
    assert_eq!(
        replay.registration.oauth_client_id,
        original.registration.oauth_client_id
    );
    assert!(matches!(
        service
            .set_registration(
                &AccessContext::local_owner(),
                request.clone(),
                entropy(5),
                103
            )
            .await,
        Err(OAuthError::RevisionConflict)
    ));
    store
        .mcp_write(
            PrincipalId::local_owner(),
            "revoke-admin".into(),
            McpIdentityWrite::Revoke {
                id: caller.client_id,
                expected: issued.client.revision,
            },
            104,
        )
        .await
        .expect("revoke");
    assert!(matches!(set().await, Err(OAuthError::Denied)));
}

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
async fn begin(
    service: &OAuthApplication<StateStoreHandle>,
    reg: &OAuthRegistration,
    n: u64,
) -> OAuthBeginResult {
    service
        .begin(
            OAuthBeginParams {
                oauth_client_id: reg.oauth_client_id.clone(),
                issuer: reg.issuer.clone(),
                resource: reg.resource.clone(),
                redirect_uri: reg.redirect_uris[0].clone(),
                code_challenge: oauth_s256(VERIFIER),
                code_challenge_method: "S256".into(),
                scopes: vec![OAUTH_SCOPE.into()],
                state: Some("opaque-state".into()),
            },
            entropy(n),
            102,
        )
        .await
        .expect("begin")
}
async fn approve(
    service: &OAuthApplication<StateStoreHandle>,
    started: &OAuthBeginResult,
) -> String {
    let reviewed = service
        .get(
            &AccessContext::local_owner(),
            OAuthRequestGetParams {
                request_id: started.request_id.clone(),
            },
            103,
        )
        .await
        .expect("review");
    service
        .decide(
            &AccessContext::local_owner(),
            OAuthRequestDecideParams {
                request_id: started.request_id.clone(),
                approve: true,
                expected_fingerprint: reviewed.approval_fingerprint,
                idempotency_key: "explicit-consent".into(),
            },
            104,
        )
        .await
        .expect("approve");
    let finish = service
        .finish(
            OAuthFinishParams {
                request_id: started.request_id.clone(),
                browser_secret: started.browser_secret.clone(),
            },
            entropy(3),
            105,
        )
        .await
        .expect("finish");
    let callback = finish.redirect_url.as_deref().expect("callback");
    assert!(callback.starts_with("http://127.0.0.1:3456/callback?"));
    let pairs: Vec<_> = callback
        .split_once('?')
        .expect("query")
        .1
        .split('&')
        .collect();
    assert!(pairs.contains(&"iss=https%3A%2F%2Flocalhost%3A5443"));
    assert!(pairs.contains(&"state=opaque-state"));
    pairs
        .iter()
        .find_map(|p| p.strip_prefix("code="))
        .expect("code")
        .to_string()
}
fn token_input(reg: &OAuthRegistration, code: String, secret: Option<String>) -> OAuthTokenParams {
    OAuthTokenParams {
        grant_type: "authorization_code".into(),
        oauth_client_id: reg.oauth_client_id.clone(),
        resource: reg.resource.clone(),
        code: Some(code),
        redirect_uri: Some(reg.redirect_uris[0].clone()),
        code_verifier: Some(VERIFIER.into()),
        refresh_token: None,
        scopes: None,
        client_secret: secret,
    }
}
fn refresh_input(reg: &OAuthRegistration, token: String) -> OAuthTokenParams {
    OAuthTokenParams {
        grant_type: "refresh_token".into(),
        oauth_client_id: reg.oauth_client_id.clone(),
        resource: reg.resource.clone(),
        code: None,
        redirect_uri: None,
        code_verifier: None,
        refresh_token: Some(token),
        scopes: None,
        client_secret: None,
    }
}
fn guard(reg: &OAuthRegistration, token: &str) -> OAuthAccessGuard {
    OAuthAccessGuard::new(mcp_credential_hash(token), reg.resource.clone())
}

#[test]
fn s256_matches_rfc7636_and_targets_do_not_accept_aliases_or_cleartext() {
    assert_eq!(
        oauth_s256(VERIFIER),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert!(oauth_validate_target("https://localhost:443", "https://localhost:443/mcp").is_ok());
    for issuer in [
        "http://localhost:5443",
        "https://127.0.0.1:5443",
        "https://localhost:5443/",
        "https://localhost:0",
    ] {
        assert_eq!(
            oauth_validate_target(issuer, &format!("{issuer}/mcp")),
            Err(OAuthError::InvalidTarget)
        );
    }
}

#[tokio::test]
async fn connection_requires_owner_consent_and_rejects_an_unknown_browser() {
    let store = StateStoreHandle::open(path()).expect("store");
    let service = OAuthApplication::new(store.clone(), [17; 32]);
    let core = seed(&store, "zero").await;
    let reg = register(&service, core.client_id, false).await.registration;
    let request = begin(&service, &reg, 2).await;
    let owner = AccessContext::local_owner();
    let unknown = AccessContext::new(
        PrincipalId::parse("external:untrusted").expect("principal"),
        [Permission::McpClientsManage, Permission::McpClientsRead],
    );
    assert!(matches!(
        service
            .get(
                &unknown,
                OAuthRequestGetParams {
                    request_id: request.request_id.clone(),
                },
                103
            )
            .await,
        Err(OAuthError::Denied)
    ));
    assert!(matches!(
        service
            .finish(
                OAuthFinishParams {
                    request_id: request.request_id.clone(),
                    browser_secret: "wrong".into()
                },
                entropy(3),
                105
            )
            .await,
        Err(OAuthError::Denied)
    ));
    let review = service
        .get(
            &owner,
            OAuthRequestGetParams {
                request_id: request.request_id.clone(),
            },
            103,
        )
        .await
        .expect("review");
    service
        .decide(
            &owner,
            OAuthRequestDecideParams {
                request_id: request.request_id.clone(),
                approve: false,
                expected_fingerprint: review.approval_fingerprint,
                idempotency_key: "explicit-deny".into(),
            },
            104,
        )
        .await
        .expect("deny");
    let denied = service
        .finish(
            OAuthFinishParams {
                request_id: request.request_id.clone(),
                browser_secret: request.browser_secret,
            },
            entropy(3),
            105,
        )
        .await
        .expect("denied callback");
    assert_eq!(denied.status, "denied");
    assert!(
        denied
            .redirect_url
            .expect("registered redirect")
            .contains("error=access_denied")
    );
    assert!(matches!(
        service
            .token(token_input(&reg, "0".repeat(64), None), entropy(4), 106)
            .await,
        Err(OAuthError::InvalidGrant)
    ));
    let access = mcp_access(&core).expect("zero identity");
    assert!(access.require(Permission::StateCheck).is_ok());
    assert_eq!(access.principal(), &core.principal_id);
}

#[tokio::test]
async fn pkce_client_redirect_audience_and_code_response_loss_do_not_mint_twice() {
    let store = StateStoreHandle::open(path()).expect("store");
    let service = OAuthApplication::new(store.clone(), [17; 32]);
    let core = seed(&store, "one").await;
    let other = seed(&store, "two").await;
    let reg = register(&service, core.client_id, false).await.registration;
    let reg2 = register(&service, other.client_id, false)
        .await
        .registration;
    let request = begin(&service, &reg, 2).await;
    let code = approve(&service, &request).await;
    let input = token_input(&reg, code, None);
    let mut bad = input.clone();
    bad.code_verifier = Some("a".repeat(43));
    assert!(matches!(
        service.token(bad, entropy(4), 106).await,
        Err(OAuthError::InvalidGrant)
    ));
    let mut bad = input.clone();
    bad.oauth_client_id = reg2.oauth_client_id.clone();
    assert!(matches!(
        service.token(bad, entropy(4), 106).await,
        Err(OAuthError::InvalidGrant)
    ));
    let mut bad = input.clone();
    bad.redirect_uri = Some("http://127.0.0.1:3456/evil".into());
    assert!(matches!(
        service.token(bad, entropy(4), 106).await,
        Err(OAuthError::InvalidGrant)
    ));
    let issued = service
        .token(input.clone(), entropy(4), 106)
        .await
        .expect("one accepted issuance");
    assert!(matches!(
        service.token(input, entropy(5), 107).await,
        Err(OAuthError::InvalidGrant)
    ));
    let access = service
        .authenticate(guard(&reg, &issued.access_token), 107)
        .await
        .expect("access");
    assert!(access.require(Permission::StateCheck).is_ok());
    assert!(access.require(Permission::OperationsRead).is_ok());
    assert!(access.require(Permission::McpClientsManage).is_ok());
    let mut wrong = guard(&reg, &issued.access_token);
    assert!(matches!(
        service
            .authenticate(guard(&reg, &issued.access_token), 106 + OAUTH_ACCESS_MS)
            .await,
        Err(OAuthError::InvalidGrant)
    ));
    wrong.resource = "https://localhost:9999/mcp".into();
    assert!(matches!(
        service.authenticate(wrong, 107).await,
        Err(OAuthError::InvalidGrant)
    ));
    assert!(matches!(
        service
            .authenticate(
                guard(&reg, issued.refresh_token.as_deref().expect("refresh")),
                107
            )
            .await,
        Err(OAuthError::InvalidGrant)
    ));
    let count = store
        .request_worker(
            |db| {
                db.query_row(
                    "SELECT count(DISTINCT family_id) FROM mcp_oauth_tokens",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .map_err(unavailable)
            },
            || OAuthError::Unavailable,
        )
        .await
        .expect("count");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn refresh_rotation_reuse_and_restart_preserve_revocation_without_reissuing_secrets() {
    let file = path();
    let store = StateStoreHandle::open(file.clone()).expect("store");
    let service = OAuthApplication::new(store.clone(), [17; 32]);
    let core = seed(&store, "refresh").await;
    let reg = register(&service, core.client_id, false).await.registration;
    let request = begin(&service, &reg, 2).await;
    let code = approve(&service, &request).await;
    let original = service
        .token(token_input(&reg, code, None), entropy(4), 106)
        .await
        .expect("token");
    let first_refresh = refresh_input(&reg, original.refresh_token.expect("refresh"));
    let rotated = service
        .token(first_refresh.clone(), entropy(5), 107)
        .await
        .expect("rotate");
    drop(service);
    drop(store);
    let store = StateStoreHandle::open(file).expect("reopen");
    let service = OAuthApplication::new(store.clone(), [17; 32]);
    assert!(
        matches!(
            service
                .finish(
                    OAuthFinishParams {
                        request_id: request.request_id,
                        browser_secret: request.browser_secret
                    },
                    entropy(99),
                    108
                )
                .await,
            Err(OAuthError::Expired)
        ),
        "volatile authorization flows do not survive restart"
    );
    assert!(
        service
            .authenticate(guard(&reg, &rotated.access_token), 108)
            .await
            .is_ok()
    );
    assert!(matches!(
        service.token(first_refresh, entropy(6), 108).await,
        Err(OAuthError::InvalidGrant)
    ));
    assert!(matches!(
        service
            .authenticate(guard(&reg, &rotated.access_token), 109)
            .await,
        Err(OAuthError::InvalidGrant)
    ));
    assert!(matches!(
        service
            .token(
                refresh_input(&reg, rotated.refresh_token.expect("new refresh")),
                entropy(7),
                109
            )
            .await,
        Err(OAuthError::InvalidGrant)
    ));
}

#[tokio::test]
async fn oauth_connection_has_business_access_until_rotation() {
    let store = StateStoreHandle::open(path()).expect("store");
    let service = OAuthApplication::new(store.clone(), [17; 32]);
    let core = seed(&store, "scope").await;
    let reg = register(&service, core.client_id, false).await.registration;
    let request = begin(&service, &reg, 2).await;
    let code = approve(&service, &request).await;
    let issued = service
        .token(token_input(&reg, code, None), entropy(4), 106)
        .await
        .expect("token");
    let access = service
        .authenticate(guard(&reg, &issued.access_token), 110)
        .await
        .expect("authenticated connection");
    assert!(access.require(Permission::StateCheck).is_ok());
    assert!(access.require(Permission::OperationsRead).is_ok());
    assert!(access.require(Permission::McpClientsManage).is_ok());
    store
        .mcp_write(
            PrincipalId::local_owner(),
            "rotate-generic".into(),
            McpIdentityWrite::Issue {
                id: core.client_id,
                expected: core.revision,
                hash: mcp_credential_hash("generic test secret"),
            },
            111,
        )
        .await
        .expect("rotate");
    assert!(matches!(
        service
            .authenticate(guard(&reg, &issued.access_token), 112)
            .await,
        Err(OAuthError::InvalidGrant)
    ));
}

#[tokio::test]
async fn confidential_registration_response_is_one_shot_and_pending_consent_cannot_drift() {
    let store = StateStoreHandle::open(path()).expect("store");
    let service = OAuthApplication::new(store.clone(), [17; 32]);
    let core = seed(&store, "confidential").await;
    let created = register(&service, core.client_id, true).await;
    let secret = created.client_secret.clone().expect("secret");
    let reg = created.registration;
    let replay = OAuthRegistrationSetParams {
        client_id: core.client_id.to_string(),
        issuer: reg.issuer.clone(),
        resource: reg.resource.clone(),
        redirect_uris: reg.redirect_uris.clone(),
        confidential: true,
        allow_refresh: true,
        expected_revision: 0,
        idempotency_key: format!("register-{}", core.client_id),
    };
    assert!(matches!(
        service
            .set_registration(&AccessContext::local_owner(), replay, entropy(99), 102)
            .await,
        Err(OAuthError::ResponseUnavailable)
    ));
    let request = begin(&service, &reg, 2).await;
    let code = approve(&service, &request).await;
    assert!(matches!(
        service
            .token(token_input(&reg, code.clone(), None), entropy(4), 106)
            .await,
        Err(OAuthError::InvalidClient)
    ));
    assert!(
        service
            .token(
                token_input(&reg, code, Some(secret.clone())),
                entropy(4),
                106
            )
            .await
            .is_ok()
    );
    let content=store.request_worker(|db|{
        let mut q=db.prepare("SELECT response_json FROM idempotency_records WHERE method LIKE 'mcp.oauth.%'").map_err(unavailable)?;
        q.query_map([],|r|r.get::<_,String>(0)).map_err(unavailable)?.collect::<Result<Vec<_>,_>>().map_err(unavailable)
    },||OAuthError::Unavailable).await.expect("idempotency").join("\n");
    assert!(!content.contains(&secret));
    assert!(!content.contains("secretHash"));
    let pending = begin(&service, &reg, 8).await;
    let review = service
        .get(
            &AccessContext::local_owner(),
            OAuthRequestGetParams {
                request_id: pending.request_id.clone(),
            },
            110,
        )
        .await
        .expect("review");
    store
        .mcp_write(
            PrincipalId::local_owner(),
            "changed".into(),
            McpIdentityWrite::Issue {
                id: core.client_id,
                expected: core.revision,
                hash: mcp_credential_hash("rotate pending review"),
            },
            111,
        )
        .await
        .expect("rotate credential during review");
    assert!(matches!(
        service
            .decide(
                &AccessContext::local_owner(),
                OAuthRequestDecideParams {
                    request_id: pending.request_id,
                    approve: true,
                    expected_fingerprint: review.approval_fingerprint,
                    idempotency_key: "stale-consent".into()
                },
                112
            )
            .await,
        Err(OAuthError::RevisionConflict)
    ));
}

#[tokio::test]
async fn concurrent_refresh_has_one_consumer_and_reuse_revokes_the_family() {
    let store = StateStoreHandle::open(path()).expect("store");
    let service = OAuthApplication::new(store.clone(), [17; 32]);
    let core = seed(&store, "concurrent").await;
    let reg = register(&service, core.client_id, false).await.registration;
    let request = begin(&service, &reg, 2).await;
    let code = approve(&service, &request).await;
    let issued = service
        .token(token_input(&reg, code, None), entropy(4), 106)
        .await
        .expect("token");
    let input = refresh_input(&reg, issued.refresh_token.expect("refresh"));
    // Independent use-case instances intentionally do not share their volatile lock.
    let first = OAuthApplication::new(store.clone(), [17; 32]);
    let second = OAuthApplication::new(store.clone(), [17; 32]);
    let (a, b) = tokio::join!(
        first.token(input.clone(), entropy(5), 107),
        second.token(input, entropy(6), 107)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let token = match (a, b) {
        (Ok(value), Err(OAuthError::InvalidGrant)) | (Err(OAuthError::InvalidGrant), Ok(value)) => {
            value
        }
        _ => panic!("exactly one refresh wins; replay is rejected"),
    };
    assert!(matches!(
        service
            .authenticate(guard(&reg, &token.access_token), 108)
            .await,
        Err(OAuthError::InvalidGrant)
    ));
    let count = store
        .request_worker(
            |db| {
                db.query_row("SELECT count(*) FROM mcp_oauth_tokens", [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(unavailable)
            },
            || OAuthError::Unavailable,
        )
        .await
        .expect("one child pair");
    assert_eq!(count, 4);
}

#[tokio::test]
async fn consumed_refresh_replay_still_revokes_a_live_family_after_its_idle_deadline() {
    let store = StateStoreHandle::open(path()).expect("store");
    let core = seed(&store, "late-replay").await;
    let service = OAuthApplication::new(store, [17; 32]);
    let reg = register(&service, core.client_id, false).await.registration;
    let request = begin(&service, &reg, 2).await;
    let code = approve(&service, &request).await;
    let issued = service
        .token(token_input(&reg, code, None), entropy(4), 106)
        .await
        .expect("token");
    let old = refresh_input(&reg, issued.refresh_token.expect("first refresh"));
    let child = service
        .token(old.clone(), entropy(5), OAUTH_REFRESH_IDLE_MS)
        .await
        .expect("rotate before idle deadline");
    let later = OAUTH_REFRESH_IDLE_MS + 107;
    assert!(matches!(
        service.token(old, entropy(6), later).await,
        Err(OAuthError::InvalidGrant)
    ));
    assert!(
        matches!(
            service
                .authenticate(guard(&reg, &child.access_token), later)
                .await,
            Err(OAuthError::InvalidGrant)
        ),
        "a consumed refresh remains reuse evidence until family expiry"
    );
    assert!(matches!(
        service
            .token(
                refresh_input(&reg, child.refresh_token.expect("child refresh")),
                entropy(7),
                later
            )
            .await,
        Err(OAuthError::InvalidGrant)
    ));
}

#[tokio::test]
async fn registration_history_is_preserved_with_snapshot_and_byte_bounded_pages() {
    let store = StateStoreHandle::open(path()).expect("store");
    let service = OAuthApplication::new(store.clone(), [17; 32]);
    let owner = AccessContext::local_owner();
    let core = seed(&store, "history").await;
    let mut active = None;
    for n in 0..65u64 {
        let uris = if n < 4 {
            (0..16)
                .map(|i| format!("https://callback.invalid/{i}/{}", "a".repeat(1850)))
                .collect()
        } else {
            vec!["http://localhost:3456/callback".into()]
        };
        let input = OAuthRegistrationSetParams {
            client_id: core.client_id.to_string(),
            issuer: oauth_issuer(5443),
            resource: format!("{}/mcp", oauth_issuer(5443)),
            redirect_uris: uris,
            confidential: false,
            allow_refresh: true,
            expected_revision: 0,
            idempotency_key: format!("history-{n}"),
        };
        let record = service
            .set_registration(&owner, input, entropy(100 + n), 1000 + n * 2)
            .await
            .expect("new registration preserves old revoked rows")
            .registration;
        if n < 64 {
            service
                .revoke_registration(
                    &owner,
                    OAuthRegistrationRevokeParams {
                        oauth_client_id: record.oauth_client_id,
                        expected_revision: record.revision,
                        idempotency_key: format!("history-revoke-{n}"),
                    },
                    1001 + n * 2,
                )
                .await
                .expect("revoke history");
        } else {
            active = Some(record)
        }
    }
    let first = service
        .list(
            &owner,
            OAuthPageParams {
                cursor: None,
                limit: Some(128),
            },
        )
        .await
        .expect("byte-limited first page");
    assert!(
        first.next_cursor.is_some(),
        "65 records fit count budget but not byte budget"
    );
    let original_cursor = first.next_cursor.clone();
    let mut page = first;
    let mut ids = std::collections::HashSet::new();
    loop {
        assert!(serde_json::to_vec(&page).expect("bounded page").len() < 65536);
        for item in page.items {
            assert!(ids.insert(item.oauth_client_id));
        }
        let Some(cursor) = page.next_cursor else {
            break;
        };
        page = service
            .list(
                &owner,
                OAuthPageParams {
                    cursor: Some(cursor),
                    limit: Some(128),
                },
            )
            .await
            .expect("complete historical pagination");
    }
    assert_eq!(ids.len(), 65);
    let active = active.expect("one active registration");
    let mut input = OAuthRegistrationSetParams {
        client_id: core.client_id.to_string(),
        issuer: active.issuer,
        resource: active.resource,
        redirect_uris: vec!["http://localhost:3456/callback?code=ambiguous".into()],
        confidential: false,
        allow_refresh: false,
        expected_revision: active.revision,
        idempotency_key: "ambiguous-response".into(),
    };
    assert!(matches!(
        service
            .set_registration(&owner, input.clone(), entropy(200), 2000)
            .await,
        Err(OAuthError::InvalidInput)
    ));
    input.redirect_uris = (0..16)
        .map(|i| {
            let prefix = format!("https://callback.invalid/{i}/");
            format!("{prefix}{}", "a".repeat(2048 - prefix.len()))
        })
        .collect();
    input.idempotency_key = "oversized-uri-json".into();
    assert!(matches!(
        service
            .set_registration(&owner, input.clone(), entropy(201), 2000)
            .await,
        Err(OAuthError::LimitExceeded)
    ));
    assert!(
        service
            .list(
                &owner,
                OAuthPageParams {
                    cursor: original_cursor.clone(),
                    limit: Some(128)
                }
            )
            .await
            .is_ok(),
        "rejected input must not change snapshot"
    );
    input.redirect_uris = vec!["http://localhost:3456/callback".into()];
    input.idempotency_key = "change-snapshot".into();
    service
        .set_registration(&owner, input, entropy(202), 2000)
        .await
        .expect("explicit update");
    assert!(matches!(
        service
            .list(
                &owner,
                OAuthPageParams {
                    cursor: original_cursor,
                    limit: Some(128)
                }
            )
            .await,
        Err(OAuthError::RevisionConflict)
    ));
}
