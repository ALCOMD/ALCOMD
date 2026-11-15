//! OAuth authority belongs to Core; adapters carry only fixed typed requests.
use crate::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

pub const OAUTH_SCOPES: [&str; 8] = [
    "projects.read",
    "packages.read",
    "repositories.read",
    "packages.manage",
    "operations.read",
    "operations.cancel",
    "state.check",
    "events.read",
];
pub const OAUTH_ACCESS_MS: u64 = 600_000;
pub const OAUTH_REFRESH_IDLE_MS: u64 = 7 * 86_400_000;
pub const OAUTH_FAMILY_MS: u64 = 30 * 86_400_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthError {
    InvalidInput,
    InvalidClient,
    InvalidGrant,
    InvalidScope,
    InvalidTarget,
    Expired,
    Denied,
    RevisionConflict,
    IdempotencyConflict,
    ResponseUnavailable,
    LimitExceeded,
    Unavailable,
}

#[derive(Clone)]
pub struct OAuthAccessGuard {
    pub token_hash: [u8; 32],
    pub resource: String,
    cursor_key: Option<[u8; 32]>,
}
impl OAuthAccessGuard {
    pub fn new(token_hash: [u8; 32], resource: String) -> Self {
        Self {
            token_hash,
            resource,
            cursor_key: None,
        }
    }
}
pub struct OAuthAuthenticated {
    pub client: McpClient,
    pub scopes: Vec<String>,
}

/// Stateless, domain-separated HMAC-SHA256 under a private Core process key.
/// Access secrets and their digests are client-derivable and must never be the key.
pub fn oauth_project_cursor_binding(
    guard: &OAuthAccessGuard,
    grant_revision: u64,
    ceiling: &[McpGrant],
    cursor: &RegistryCursor<ProjectId>,
) -> Result<String, OAuthError> {
    let message = serde_json::to_vec(&(
        "alcomd.m8.oauth.project-cursor.v1",
        &guard.token_hash,
        &guard.resource,
        grant_revision,
        ceiling,
        cursor.registered_at_ms,
        cursor.id,
    ))
    .map_err(|_| OAuthError::Unavailable)?;
    let key = guard.cursor_key.as_ref().ok_or(OAuthError::Denied)?;
    Ok(oauth_hex(&cursor_hmac(key, &message)))
}

// RFC2104 construction over the existing SHA256 implementation. Production
// keys are exactly32 bytes; the short-key branch is checked by RFC4231 vectors.
fn cursor_hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0u8; 64];
    if key.len() > block.len() {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let inner_pad = block.map(|b| b ^ 0x36);
    let outer_pad = block.map(|b| b ^ 0x5c);
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
    outer.finalize().into()
}

#[cfg(test)]
mod cursor_binding_tests {
    use super::*;
    #[test]
    fn cursor_hmac_matches_rfc4231_short_and_long_key_vectors() {
        // https://www.rfc-editor.org/rfc/rfc4231.html sections4.2,4.3,4.7.
        for (key, message, expected) in [
            (
                vec![0x0b; 20],
                b"Hi There".as_slice(),
                "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
            ),
            (
                b"Jefe".to_vec(),
                b"what do ya want for nothing?".as_slice(),
                "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
            ),
            (
                vec![0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First".as_slice(),
                "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
            ),
        ] {
            assert_eq!(oauth_hex(&cursor_hmac(&key, message)), expected);
        }
    }
    #[test]
    fn a_client_known_token_digest_is_not_the_core_cursor_key() {
        let guard = OAuthAccessGuard::new([23; 32], "https://localhost:5443/mcp".into());
        let cursor = RegistryCursor {
            registered_at_ms: 1,
            id: ProjectId::new(),
            authorization_binding: None,
        };
        assert!(oauth_project_cursor_binding(&guard, 1, &[], &cursor).is_err());
        let mut signed = guard.clone();
        signed.cursor_key = Some([17; 32]);
        let mac = oauth_project_cursor_binding(&signed, 1, &[], &cursor).expect("private Core key");
        let mut forged = guard;
        forged.cursor_key = Some(forged.token_hash);
        assert_ne!(
            oauth_project_cursor_binding(&forged, 1, &[], &cursor).expect("client guess"),
            mac
        );
        signed.cursor_key = Some([18; 32]);
        assert_ne!(
            oauth_project_cursor_binding(&signed, 1, &[], &cursor).expect("new process key"),
            mac
        );
    }
}
impl std::fmt::Debug for OAuthAccessGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OAuthAccessGuard([REDACTED])")
    }
}
impl AccessContext {
    pub fn oauth_guard(&self) -> Option<&OAuthAccessGuard> {
        self.oauth_guard.as_ref()
    }
    pub(crate) fn with_oauth_guard(mut self, guard: OAuthAccessGuard, scopes: Vec<String>) -> Self {
        self.oauth_guard = Some(guard);
        self.oauth_scopes = scopes;
        self
    }
    pub fn oauth_scope_denied(&self, required: &[&str]) -> bool {
        self.oauth_guard.is_some()
            && required
                .iter()
                .any(|s| !self.oauth_scopes.iter().any(|known| known == s))
    }
}

#[derive(Clone)]
pub struct OAuthRegistrationState {
    pub registration: OAuthRegistration,
    pub secret_hash: Option<[u8; 32]>,
    pub client: McpClient,
    pub credential_generation: String,
    pub grant_revision: u64,
}
#[derive(Clone)]
pub struct OAuthTokenState {
    pub registration: OAuthRegistrationState,
    pub grants: Vec<McpGrant>,
    pub family_id: String,
    pub family_expires_at_ms: u64,
    pub consumed: bool,
}
#[derive(Clone)]
pub struct OAuthIssue {
    pub registration: OAuthRegistrationState,
    pub grants: Vec<McpGrant>,
    pub family_id: String,
    pub family_expires_at_ms: u64,
    pub parent_refresh: Option<[u8; 32]>,
    pub source_code_hash: Option<[u8; 32]>,
    pub access_hash: [u8; 32],
    pub refresh_hash: Option<[u8; 32]>,
}

pub trait OAuthStore: McpIdentityStore {
    fn oauth_registration(
        &self,
        id: String,
    ) -> impl Future<Output = Result<OAuthRegistrationState, OAuthError>> + Send;
    fn oauth_registrations(
        &self,
        input: OAuthPageParams,
    ) -> impl Future<Output = Result<OAuthRegistrationPage, OAuthError>> + Send;
    fn oauth_set_registration(
        &self,
        input: OAuthRegistrationSetParams,
        id: String,
        secret_hash: Option<[u8; 32]>,
        now: u64,
    ) -> impl Future<Output = Result<OAuthRegistration, OAuthError>> + Send;
    fn oauth_revoke_registration(
        &self,
        input: OAuthRegistrationRevokeParams,
        now: u64,
    ) -> impl Future<Output = Result<OAuthRegistration, OAuthError>> + Send;
    fn oauth_issue(
        &self,
        issue: OAuthIssue,
        now: u64,
    ) -> impl Future<Output = Result<(), OAuthError>> + Send;
    fn oauth_refresh(
        &self,
        hash: [u8; 32],
        id: String,
        now: u64,
    ) -> impl Future<Output = Result<OAuthTokenState, OAuthError>> + Send;
    fn oauth_revoke_token(
        &self,
        hash: [u8; 32],
        id: String,
        now: u64,
    ) -> impl Future<Output = Result<(), OAuthError>> + Send;
    fn oauth_authenticate(
        &self,
        guard: OAuthAccessGuard,
        now: u64,
    ) -> impl Future<Output = Result<OAuthAuthenticated, OAuthError>> + Send;
}

/// An entropy bundle is supplied by daemon's already-approved OS random source.
/// Values are never accepted as entropy from public RPC input.
pub struct OAuthEntropy {
    pub id: String,
    pub secret: String,
    pub extra: String,
}
impl OAuthEntropy {
    pub fn access(&self) -> String {
        format!("oa1_{}", self.secret)
    }
    pub fn refresh(&self) -> String {
        format!("or1_{}", self.extra)
    }
}

#[derive(Clone)]
pub struct OAuthApplication<S: OAuthStore> {
    store: S,
    pending: Arc<Mutex<VecDeque<Pending>>>,
    cursor_key: [u8; 32],
}
struct Pending {
    id: String,
    input: OAuthBeginParams,
    browser_hash: [u8; 32],
    expires: u64,
    approved: Option<Vec<McpGrant>>,
    denied: bool,
    delivered: bool,
    code_hash: Option<[u8; 32]>,
    code_expires: u64,
    consumed: bool,
    decision: Option<(String, [u8; 32])>,
    approval_fingerprint: Option<String>,
}

pub fn oauth_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn oauth_fingerprint<T: Serialize>(value: &T) -> Result<String, OAuthError> {
    Ok(oauth_hex(&Sha256::digest(
        serde_json::to_vec(value).map_err(|_| OAuthError::Unavailable)?,
    )))
}
/// Stream metadata only; neither historical grants nor secrets are collected for paging.
pub fn oauth_registration_fingerprint(
    records: impl Iterator<Item = Result<OAuthRegistration, OAuthError>>,
) -> Result<(String, usize), OAuthError> {
    let mut digest = Sha256::new();
    let mut count = 0usize;
    for record in records {
        let bytes = serde_json::to_vec(&record?).map_err(|_| OAuthError::Unavailable)?;
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
        count = count.checked_add(1).ok_or(OAuthError::Unavailable)?;
    }
    Ok((oauth_hex(&digest.finalize()), count))
}
pub fn oauth_page_window(
    cursor: &Option<String>,
    fp: &str,
    len: usize,
    limit: Option<u32>,
) -> Result<(usize, usize), OAuthError> {
    page(cursor, fp, len, limit)
}
fn hash(secret: &str) -> [u8; 32] {
    mcp_credential_hash(secret)
}
fn owner(access: &AccessContext, write: bool) -> Result<(), OAuthError> {
    if access.principal() != &PrincipalId::local_owner() {
        return Err(OAuthError::Denied);
    }
    access
        .require(if write {
            Permission::McpClientsManage
        } else {
            Permission::McpClientsRead
        })
        .map_err(|_| OAuthError::Denied)
}
fn key(value: &str) -> Result<(), OAuthError> {
    if value.is_empty() || value.len() > 128 || !value.is_ascii() {
        return Err(OAuthError::InvalidInput);
    }
    Ok(())
}
fn validate_id(value: &str) -> Result<(), OAuthError> {
    if value.len() != 36 || McpClientId::parse(value).is_err() {
        return Err(OAuthError::InvalidInput);
    }
    Ok(())
}
pub fn oauth_issuer(port: u16) -> String {
    format!("https://localhost:{port}")
}
pub fn oauth_validate_target(issuer: &str, resource: &str) -> Result<(), OAuthError> {
    if issuer.len() > 2048 || resource.len() > 2048 {
        return Err(OAuthError::InvalidTarget);
    }
    let u = Url::parse(issuer).map_err(|_| OAuthError::InvalidTarget)?;
    let port = u.port_or_known_default().ok_or(OAuthError::InvalidTarget)?;
    if port == 0 || issuer != oauth_issuer(port) || resource != format!("{issuer}/mcp") {
        return Err(OAuthError::InvalidTarget);
    }
    Ok(())
}
pub fn oauth_validate_registration(input: &OAuthRegistrationSetParams) -> Result<(), OAuthError> {
    key(&input.idempotency_key)?;
    validate_id(&input.client_id)?;
    oauth_validate_target(&input.issuer, &input.resource)?;
    if input.expected_revision > i64::MAX as u64
        || input.redirect_uris.is_empty()
        || input.redirect_uris.len() > 16
    {
        return Err(OAuthError::InvalidInput);
    }
    if serde_json::to_vec(&input.redirect_uris)
        .map_err(|_| OAuthError::InvalidInput)?
        .len()
        > 32768
    {
        return Err(OAuthError::LimitExceeded);
    }
    let mut seen = std::collections::HashSet::new();
    for value in &input.redirect_uris {
        let u = Url::parse(value).map_err(|_| OAuthError::InvalidInput)?;
        let loopback = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if value.len() > 2048
            || !value.is_ascii()
            || value.bytes().any(|b| b.is_ascii_control())
            || !u.username().is_empty()
            || u.password().is_some()
            || u.fragment().is_some()
            || u.query_pairs().any(|(key, _)| {
                [
                    "code",
                    "state",
                    "iss",
                    "error",
                    "error_description",
                    "error_uri",
                ]
                .contains(&key.as_ref())
            })
            || !(u.scheme() == "https" || (u.scheme() == "http" && loopback))
            || u.host_str().is_none()
            || !seen.insert(value)
        {
            return Err(OAuthError::InvalidInput);
        }
    }
    Ok(())
}
fn scopes(values: &[String]) -> Result<(), OAuthError> {
    if values.is_empty() || values.len() > 8 {
        return Err(OAuthError::InvalidScope);
    }
    let mut seen = std::collections::HashSet::new();
    for s in values {
        if !OAUTH_SCOPES.contains(&s.as_str()) || !seen.insert(s) {
            return Err(OAuthError::InvalidScope);
        }
    }
    Ok(())
}
fn active(state: &OAuthRegistrationState) -> Result<(), OAuthError> {
    if state.registration.revoked_at_ms.is_some() || state.client.revoked_at_ms.is_some() {
        return Err(OAuthError::InvalidClient);
    }
    Ok(())
}
fn client_auth(state: &OAuthRegistrationState, secret: Option<&str>) -> Result<(), OAuthError> {
    active(state)?;
    match (&state.secret_hash, secret) {
        (None, None) => Ok(()),
        (Some(expected), Some(s)) if s.len() <= 256 && &hash(s) == expected => Ok(()),
        _ => Err(OAuthError::InvalidClient),
    }
}

/// Fixed SHA-256 base64url encoding for PKCE, not a general codec or crypto primitive.
pub fn oauth_s256(verifier: &str) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let digest = Sha256::digest(verifier.as_bytes());
    let mut out = String::with_capacity(43);
    for chunk in digest.chunks(3) {
        let v = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        for i in 0..chunk.len() + 1 {
            out.push(ABC[((v >> (18 - 6 * i)) & 63) as usize] as char)
        }
    }
    out
}

fn review(
    p: &Pending,
    current: &OAuthRegistrationState,
) -> Result<(Vec<McpGrant>, String), OAuthError> {
    active(current)?;
    let grants: Vec<_> = current
        .client
        .grants
        .iter()
        .filter(|g| p.input.scopes.contains(&g.permission))
        .cloned()
        .collect();
    let fingerprint = oauth_fingerprint(&(
        &p.id,
        &p.input.oauth_client_id,
        &p.input.issuer,
        &p.input.resource,
        &p.input.redirect_uri,
        &p.input.scopes,
        current.registration.revision,
        current.client.revision,
        &current.credential_generation,
        &grants,
    ))?;
    Ok((grants, fingerprint))
}

impl<S: OAuthStore> OAuthApplication<S> {
    pub fn new(store: S, cursor_key: [u8; 32]) -> Self {
        Self {
            store,
            pending: Arc::new(Mutex::new(VecDeque::new())),
            cursor_key,
        }
    }
    pub async fn authenticate(
        &self,
        mut guard: OAuthAccessGuard,
        now: u64,
    ) -> Result<AccessContext, OAuthError> {
        let authenticated = self.store.oauth_authenticate(guard.clone(), now).await?;
        guard.cursor_key = Some(self.cursor_key);
        Ok(mcp_access(&authenticated.client)
            .map_err(|_| OAuthError::Denied)?
            .with_oauth_guard(guard, authenticated.scopes))
    }
    pub async fn set_registration(
        &self,
        access: &AccessContext,
        input: OAuthRegistrationSetParams,
        entropy: OAuthEntropy,
        now: u64,
    ) -> Result<OAuthRegistrationResult, OAuthError> {
        owner(access, true)?;
        oauth_validate_registration(&input)?;
        let secret = input.confidential.then_some(entropy.secret);
        let registration = self
            .store
            .oauth_set_registration(input, entropy.id, secret.as_deref().map(hash), now)
            .await?;
        Ok(OAuthRegistrationResult {
            registration,
            client_secret: secret,
        })
    }
    pub async fn revoke_registration(
        &self,
        access: &AccessContext,
        input: OAuthRegistrationRevokeParams,
        now: u64,
    ) -> Result<OAuthRegistration, OAuthError> {
        owner(access, true)?;
        key(&input.idempotency_key)?;
        validate_id(&input.oauth_client_id)?;
        self.store.oauth_revoke_registration(input, now).await
    }
    pub async fn list(
        &self,
        access: &AccessContext,
        input: OAuthPageParams,
    ) -> Result<OAuthRegistrationPage, OAuthError> {
        owner(access, false)?;
        self.store.oauth_registrations(input).await
    }
    pub async fn begin(
        &self,
        input: OAuthBeginParams,
        entropy: OAuthEntropy,
        now: u64,
    ) -> Result<OAuthBeginResult, OAuthError> {
        validate_id(&input.oauth_client_id)?;
        oauth_validate_target(&input.issuer, &input.resource)?;
        scopes(&input.scopes)?;
        if input.code_challenge_method != "S256"
            || input.code_challenge.len() != 43
            || !input
                .code_challenge
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            || input
                .state
                .as_ref()
                .is_some_and(|s| s.len() > 2048 || s.chars().any(char::is_control))
        {
            return Err(OAuthError::InvalidInput);
        }
        let current = self
            .store
            .oauth_registration(input.oauth_client_id.clone())
            .await?;
        active(&current)?;
        if current.registration.issuer != input.issuer
            || current.registration.resource != input.resource
        {
            return Err(OAuthError::InvalidTarget);
        }
        if !current
            .registration
            .redirect_uris
            .contains(&input.redirect_uri)
        {
            return Err(OAuthError::InvalidInput);
        }
        let mut pending = self.pending.lock().await;
        pending.retain(|p| {
            p.expires > now
                && !p.consumed
                && !(p.denied && p.delivered)
                && (p.code_hash.is_none() || p.code_expires > now)
        });
        if pending.len() >= 32
            || pending
                .iter()
                .filter(|p| p.input.oauth_client_id == input.oauth_client_id)
                .count()
                >= 4
        {
            return Err(OAuthError::LimitExceeded);
        }
        let id = entropy.id;
        let browser_secret = entropy.secret;
        pending.push_back(Pending {
            id: id.clone(),
            input,
            browser_hash: hash(&browser_secret),
            expires: now.saturating_add(600_000),
            approved: None,
            denied: false,
            delivered: false,
            code_hash: None,
            code_expires: 0,
            consumed: false,
            decision: None,
            approval_fingerprint: None,
        });
        Ok(OAuthBeginResult {
            request_id: id,
            browser_secret,
        })
    }
    pub async fn get(
        &self,
        access: &AccessContext,
        input: OAuthRequestGetParams,
        now: u64,
    ) -> Result<OAuthRequestReview, OAuthError> {
        owner(access, false)?;
        validate_id(&input.request_id)?;
        let pending = self.pending.lock().await;
        let p = pending
            .iter()
            .find(|p| p.id == input.request_id)
            .ok_or(OAuthError::Expired)?;
        if p.expires <= now {
            return Err(OAuthError::Expired);
        }
        let current = self
            .store
            .oauth_registration(p.input.oauth_client_id.clone())
            .await?;
        let (all, fp) = review(p, &current)?;
        let (start, limit) = page(&input.cursor, &fp, all.len(), input.limit)?;
        let grants = all
            .iter()
            .skip(start)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let end = start + grants.len();
        Ok(OAuthRequestReview {
            request_id: p.id.clone(),
            oauth_client_id: p.input.oauth_client_id.clone(),
            client_id: current.client.client_id.to_string(),
            display_name: current.client.display_name,
            issuer: p.input.issuer.clone(),
            resource: p.input.resource.clone(),
            redirect_uri: p.input.redirect_uri.clone(),
            scopes: p.input.scopes.clone(),
            grants,
            next_cursor: (end < all.len()).then(|| format!("{fp}:{end}")),
            approval_fingerprint: fp,
            status: status(p).into(),
            expires_at_ms: p.expires,
        })
    }
    pub async fn decide(
        &self,
        access: &AccessContext,
        input: OAuthRequestDecideParams,
        now: u64,
    ) -> Result<OAuthRequestDecision, OAuthError> {
        owner(access, true)?;
        key(&input.idempotency_key)?;
        validate_id(&input.request_id)?;
        if input.expected_fingerprint.len() != 64
            || !input
                .expected_fingerprint
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(OAuthError::InvalidInput);
        }
        validate_mcp_grants(&input.grants).map_err(|_| OAuthError::InvalidInput)?;
        let mut pending = self.pending.lock().await;
        let p = pending
            .iter_mut()
            .find(|p| p.id == input.request_id)
            .ok_or(OAuthError::Expired)?;
        if p.expires <= now {
            return Err(OAuthError::Expired);
        }
        let digest = hash(&serde_json::to_string(&input).map_err(|_| OAuthError::InvalidInput)?);
        if let Some((key, fp)) = &p.decision {
            if key == &input.idempotency_key && fp == &digest {
                return Ok(OAuthRequestDecision {
                    request_id: p.id.clone(),
                    status: if input.approve { "approved" } else { "denied" }.into(),
                });
            }
            return Err(OAuthError::IdempotencyConflict);
        }
        let current = self
            .store
            .oauth_registration(p.input.oauth_client_id.clone())
            .await?;
        let (allowed, fp) = review(p, &current)?;
        if fp != input.expected_fingerprint {
            return Err(OAuthError::RevisionConflict);
        }
        if input.grants.iter().any(|g| !allowed.contains(g))
            || (!input.approve && !input.grants.is_empty())
        {
            return Err(OAuthError::Denied);
        }
        if input.approve && input.grants.is_empty() {
            return Err(OAuthError::InvalidScope);
        }
        p.denied = !input.approve;
        p.approval_fingerprint = Some(fp);
        p.approved = input.approve.then_some(input.grants);
        p.decision = Some((input.idempotency_key, digest));
        Ok(OAuthRequestDecision {
            request_id: p.id.clone(),
            status: status(p).into(),
        })
    }
    pub async fn finish(
        &self,
        input: OAuthFinishParams,
        entropy: OAuthEntropy,
        now: u64,
    ) -> Result<OAuthFinishResult, OAuthError> {
        validate_id(&input.request_id)?;
        if input.browser_secret.len() != 64 || !input.browser_secret.is_ascii() {
            return Err(OAuthError::Denied);
        }
        let mut pending = self.pending.lock().await;
        let p = pending
            .iter_mut()
            .find(|p| p.id == input.request_id)
            .ok_or(OAuthError::Expired)?;
        if hash(&input.browser_secret) != p.browser_hash {
            return Err(OAuthError::Denied);
        }
        if p.expires <= now {
            return Err(OAuthError::Expired);
        }
        let current = self
            .store
            .oauth_registration(p.input.oauth_client_id.clone())
            .await?;
        active(&current)?;
        if current.registration.issuer != p.input.issuer
            || current.registration.resource != p.input.resource
            || !current
                .registration
                .redirect_uris
                .contains(&p.input.redirect_uri)
        {
            return Err(OAuthError::InvalidGrant);
        }
        if p.delivered {
            return Err(OAuthError::ResponseUnavailable);
        }
        let Some(grants) = p.approved.as_ref() else {
            if !p.denied {
                return Ok(OAuthFinishResult {
                    status: "pending".into(),
                    redirect_url: None,
                });
            }
            let mut u = Url::parse(&p.input.redirect_uri).map_err(|_| OAuthError::InvalidInput)?;
            u.query_pairs_mut()
                .append_pair("error", "access_denied")
                .append_pair("iss", &p.input.issuer);
            if let Some(state) = &p.input.state {
                u.query_pairs_mut().append_pair("state", state);
            }
            p.delivered = true;
            return Ok(OAuthFinishResult {
                status: "denied".into(),
                redirect_url: Some(u.into()),
            });
        };
        if grants.iter().any(|g| !current.client.grants.contains(g))
            || p.approval_fingerprint.as_ref() != Some(&review(p, &current)?.1)
        {
            return Err(OAuthError::RevisionConflict);
        }
        let code = entropy.secret;
        p.code_hash = Some(hash(&code));
        p.code_expires = now.saturating_add(60_000).min(p.expires);
        p.delivered = true;
        let mut u = Url::parse(&p.input.redirect_uri).map_err(|_| OAuthError::InvalidInput)?;
        u.query_pairs_mut()
            .append_pair("code", &code)
            .append_pair("iss", &p.input.issuer);
        if let Some(state) = &p.input.state {
            u.query_pairs_mut().append_pair("state", state);
        }
        Ok(OAuthFinishResult {
            status: "approved".into(),
            redirect_url: Some(u.into()),
        })
    }
    pub async fn token(
        &self,
        input: OAuthTokenParams,
        entropy: OAuthEntropy,
        now: u64,
    ) -> Result<OAuthTokenResult, OAuthError> {
        validate_id(&input.oauth_client_id)?;
        let current = self
            .store
            .oauth_registration(input.oauth_client_id.clone())
            .await?;
        client_auth(&current, input.client_secret.as_deref())?;
        if current.registration.resource != input.resource {
            return Err(OAuthError::InvalidTarget);
        }
        let mut pending = self.pending.lock().await;
        let (grants, family_id, family_expires_at_ms, parent_refresh, source_code_hash, index) =
            match input.grant_type.as_str() {
                "authorization_code" => {
                    if input.refresh_token.is_some() || input.scopes.is_some() {
                        return Err(OAuthError::InvalidInput);
                    }
                    let code = input
                        .code
                        .as_ref()
                        .filter(|s| s.len() == 64)
                        .ok_or(OAuthError::InvalidGrant)?;
                    let digest = hash(code);
                    let i = pending
                        .iter()
                        .position(|p| p.code_hash == Some(digest))
                        .ok_or(OAuthError::InvalidGrant)?;
                    let p = &pending[i];
                    let verifier = input
                        .code_verifier
                        .as_deref()
                        .ok_or(OAuthError::InvalidGrant)?;
                    if !(43..=128).contains(&verifier.len())
                        || !verifier
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
                        || oauth_s256(verifier) != p.input.code_challenge
                        || p.consumed
                        || p.denied
                        || p.code_expires <= now
                        || p.expires <= now
                        || p.input.oauth_client_id != input.oauth_client_id
                        || input.redirect_uri.as_ref() != Some(&p.input.redirect_uri)
                        || input.resource != p.input.resource
                    {
                        return Err(OAuthError::InvalidGrant);
                    }
                    if p.approval_fingerprint.as_ref() != Some(&review(p, &current)?.1) {
                        return Err(OAuthError::RevisionConflict);
                    }
                    (
                        p.approved.clone().ok_or(OAuthError::InvalidGrant)?,
                        entropy.id.clone(),
                        now.saturating_add(OAUTH_FAMILY_MS),
                        None,
                        Some(digest),
                        Some(i),
                    )
                }
                "refresh_token" => {
                    if !current.registration.allow_refresh
                        || input.code.is_some()
                        || input.code_verifier.is_some()
                        || input.redirect_uri.is_some()
                    {
                        return Err(OAuthError::InvalidGrant);
                    }
                    let secret = input
                        .refresh_token
                        .as_ref()
                        .filter(|s| s.len() == 68 && s.starts_with("or1_"))
                        .ok_or(OAuthError::InvalidGrant)?;
                    let digest = hash(secret);
                    let old = self
                        .store
                        .oauth_refresh(digest, input.oauth_client_id.clone(), now)
                        .await?;
                    let mut grants = old.grants;
                    if let Some(requested) = &input.scopes {
                        scopes(requested)?;
                        if requested
                            .iter()
                            .any(|s| !grants.iter().any(|g| &g.permission == s))
                        {
                            return Err(OAuthError::InvalidScope);
                        }
                        grants.retain(|g| requested.contains(&g.permission));
                    }
                    (
                        grants,
                        old.family_id,
                        old.family_expires_at_ms,
                        Some(digest),
                        None,
                        None,
                    )
                }
                _ => return Err(OAuthError::InvalidInput),
            };
        if grants.iter().any(|g| !current.client.grants.contains(g)) {
            // Revocation narrows refresh; a code approved against stale grants must be reviewed again.
            if parent_refresh.is_none() {
                return Err(OAuthError::RevisionConflict);
            }
        }
        let grants: Vec<_> = grants
            .into_iter()
            .filter(|g| current.client.grants.contains(g))
            .collect();
        if grants.is_empty() {
            return Err(OAuthError::InvalidGrant);
        }
        let access_token = entropy.access();
        let refresh_token = current
            .registration
            .allow_refresh
            .then(|| entropy.refresh());
        self.store
            .oauth_issue(
                OAuthIssue {
                    registration: current,
                    grants: grants.clone(),
                    family_id,
                    family_expires_at_ms,
                    parent_refresh,
                    source_code_hash,
                    access_hash: hash(&access_token),
                    refresh_hash: refresh_token.as_deref().map(hash),
                },
                now,
            )
            .await?;
        if let Some(i) = index {
            pending[i].consumed = true
        }
        let mut scopes: Vec<_> = grants.iter().map(|g| g.permission.clone()).collect();
        scopes.sort();
        scopes.dedup();
        Ok(OAuthTokenResult {
            access_token,
            token_type: "Bearer".into(),
            expires_in: (family_expires_at_ms
                .saturating_sub(now)
                .min(OAUTH_ACCESS_MS))
                / 1000,
            scope: scopes.join(" "),
            refresh_token,
        })
    }
    pub async fn revoke(&self, input: OAuthRevokeParams, now: u64) -> Result<(), OAuthError> {
        validate_id(&input.oauth_client_id)?;
        let current = self
            .store
            .oauth_registration(input.oauth_client_id.clone())
            .await?;
        client_auth(&current, input.client_secret.as_deref())?;
        if input.token.len() > 256 || input.token_type_hint.as_ref().is_some_and(|s| s.len() > 32) {
            return Err(OAuthError::InvalidInput);
        }
        self.store
            .oauth_revoke_token(hash(&input.token), input.oauth_client_id, now)
            .await
    }
}
fn status(p: &Pending) -> &'static str {
    if p.denied {
        "denied"
    } else if p.consumed {
        "consumed"
    } else if p.approved.is_some() {
        "approved"
    } else {
        "pending"
    }
}
fn page(
    cursor: &Option<String>,
    fp: &str,
    len: usize,
    limit: Option<u32>,
) -> Result<(usize, usize), OAuthError> {
    if cursor
        .as_ref()
        .is_some_and(|s| s.len() > 512 || !s.is_ascii())
    {
        return Err(OAuthError::InvalidInput);
    }
    let limit = limit.unwrap_or(64);
    if !(1..=128).contains(&limit) {
        return Err(OAuthError::InvalidInput);
    }
    let start = match cursor {
        None => 0,
        Some(c) => {
            let (old, n) = c.split_once(':').ok_or(OAuthError::InvalidInput)?;
            if old != fp {
                return Err(OAuthError::RevisionConflict);
            }
            n.parse::<usize>().map_err(|_| OAuthError::InvalidInput)?
        }
    };
    if start > len {
        return Err(OAuthError::RevisionConflict);
    }
    Ok((start, limit as usize))
}
