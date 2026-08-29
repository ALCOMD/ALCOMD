use std::fmt;
use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, Permission, PrincipalId, Revision, StoreErrorKind, UserPackageId,
};

pub const DEFAULT_USER_PACKAGE_PAGE_LIMIT: u32 = 100;
pub const MAX_USER_PACKAGE_PAGE_LIMIT: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPackageCursor {
    pub updated_at_ms: u64,
    pub user_package_id: UserPackageId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPackageRecord {
    pub user_package_id: UserPackageId,
    #[serde(skip_serializing, default = "PrincipalId::local_owner")]
    pub owner: PrincipalId,
    pub source_root_path: String,
    #[serde(skip_serializing, default)]
    pub source_identity_key: Vec<u8>,
    pub package_id: String,
    pub version: String,
    pub display_name: Option<String>,
    #[serde(skip_serializing, default)]
    pub manifest_json: String,
    #[serde(skip_serializing, default)]
    pub dependencies_json: String,
    #[serde(skip_serializing, default)]
    pub manifest_fingerprint: [u8; 32],
    #[serde(skip_serializing, default)]
    pub content_fingerprint: [u8; 32],
    #[serde(with = "sha256_hex")]
    pub archive_sha256: [u8; 32],
    pub revision: Revision,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPackageSnapshot {
    pub source_root_path: String,
    pub source_identity_key: Vec<u8>,
    pub package_id: String,
    pub version: String,
    pub display_name: Option<String>,
    pub manifest_json: String,
    pub dependencies_json: String,
    pub manifest_fingerprint: [u8; 32],
    pub content_fingerprint: [u8; 32],
    pub archive_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPackagePage {
    pub user_packages: Vec<UserPackageRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<UserPackageCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPackageWriteResult {
    pub user_package: UserPackageRecord,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPackageRemoveResult {
    pub user_package_id: UserPackageId,
    pub revision: Revision,
    pub removed: bool,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserPackageErrorCode {
    InvalidInput,
    PermissionDenied,
    NotFound,
    AlreadyEnrolled,
    SourceUnavailable,
    SourceUnsafe,
    SourceChanged,
    ManifestInvalid,
    LimitExceeded,
    RevisionConflict,
    IdempotencyConflict,
    StoreUnavailable,
    Internal,
}

impl UserPackageErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_request",
            Self::PermissionDenied => "permission_denied",
            Self::NotFound => "user_package_not_found",
            Self::AlreadyEnrolled => "user_package_already_enrolled",
            Self::SourceUnavailable => "user_package_source_unavailable",
            Self::SourceUnsafe => "user_package_source_unsafe",
            Self::SourceChanged => "user_package_source_changed",
            Self::ManifestInvalid => "user_package_manifest_invalid",
            Self::LimitExceeded => "user_package_limit_exceeded",
            Self::RevisionConflict => "revision_conflict",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::StoreUnavailable => "store_unavailable",
            Self::Internal => "internal_error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPackageError {
    code: UserPackageErrorCode,
}

impl UserPackageError {
    #[must_use]
    pub const fn new(code: UserPackageErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(&self) -> UserPackageErrorCode {
        self.code
    }
}

impl fmt::Display for UserPackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "User Package request failed: {:?}", self.code)
    }
}

impl std::error::Error for UserPackageError {}

impl From<StoreErrorKind> for UserPackageError {
    fn from(value: StoreErrorKind) -> Self {
        Self::new(match value {
            StoreErrorKind::RevisionConflict => UserPackageErrorCode::RevisionConflict,
            StoreErrorKind::IdempotencyConflict => UserPackageErrorCode::IdempotencyConflict,
            StoreErrorKind::Unavailable => UserPackageErrorCode::StoreUnavailable,
            StoreErrorKind::CorruptState
            | StoreErrorKind::OperationNotFound
            | StoreErrorKind::OperationNotCancellable => UserPackageErrorCode::Internal,
        })
    }
}

pub trait UserPackageStore: Clone + Send + Sync + 'static {
    fn replay_user_package_enroll(
        &self,
        owner: PrincipalId,
        source_path: String,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<UserPackageWriteResult>, UserPackageError>> + Send;

    fn replay_user_package_refresh(
        &self,
        owner: PrincipalId,
        user_package_id: UserPackageId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<UserPackageWriteResult>, UserPackageError>> + Send;

    fn list_user_packages(
        &self,
        owner: PrincipalId,
        cursor: Option<UserPackageCursor>,
        limit: u32,
    ) -> impl Future<Output = Result<UserPackagePage, UserPackageError>> + Send;

    fn get_user_package(
        &self,
        owner: PrincipalId,
        user_package_id: UserPackageId,
    ) -> impl Future<Output = Result<UserPackageRecord, UserPackageError>> + Send;

    fn enroll_user_package(
        &self,
        owner: PrincipalId,
        snapshot: UserPackageSnapshot,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<UserPackageWriteResult, UserPackageError>> + Send;

    fn refresh_user_package(
        &self,
        owner: PrincipalId,
        user_package_id: UserPackageId,
        expected_revision: Revision,
        snapshot: UserPackageSnapshot,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<UserPackageWriteResult, UserPackageError>> + Send;

    fn remove_user_package(
        &self,
        owner: PrincipalId,
        user_package_id: UserPackageId,
        expected_revision: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<UserPackageRemoveResult, UserPackageError>> + Send;
}

pub trait UserPackageAdapter: Clone + Send + Sync + 'static {
    fn snapshot(
        &self,
        source_path: String,
    ) -> impl Future<Output = Result<UserPackageSnapshot, UserPackageError>> + Send;
}

#[derive(Clone)]
pub struct UserPackageApplication<S, A> {
    store: S,
    adapter: A,
}

impl<S, A> UserPackageApplication<S, A>
where
    S: UserPackageStore,
    A: UserPackageAdapter,
{
    #[must_use]
    pub const fn new(store: S, adapter: A) -> Self {
        Self { store, adapter }
    }

    pub async fn list(
        &self,
        access: &AccessContext,
        cursor: Option<UserPackageCursor>,
        limit: u32,
    ) -> Result<UserPackagePage, UserPackageError> {
        require(access, Permission::PackagesRead)?;
        if !(1..=MAX_USER_PACKAGE_PAGE_LIMIT).contains(&limit) {
            return Err(UserPackageError::new(UserPackageErrorCode::InvalidInput));
        }
        self.store
            .list_user_packages(access.principal().clone(), cursor, limit)
            .await
    }

    pub async fn get(
        &self,
        access: &AccessContext,
        user_package_id: UserPackageId,
    ) -> Result<UserPackageRecord, UserPackageError> {
        require(access, Permission::PackagesRead)?;
        self.store
            .get_user_package(access.principal().clone(), user_package_id)
            .await
    }

    pub async fn enroll(
        &self,
        access: &AccessContext,
        source_path: String,
        key: IdempotencyKey,
    ) -> Result<UserPackageWriteResult, UserPackageError> {
        require_local_owner(access)?;
        if let Some(response) = self
            .store
            .replay_user_package_enroll(
                access.principal().clone(),
                source_path.clone(),
                key.clone(),
            )
            .await?
        {
            return Ok(response);
        }
        let snapshot = self.adapter.snapshot(source_path).await?;
        self.store
            .enroll_user_package(access.principal().clone(), snapshot, key, now_ms()?)
            .await
    }

    pub async fn refresh(
        &self,
        access: &AccessContext,
        user_package_id: UserPackageId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<UserPackageWriteResult, UserPackageError> {
        require_local_owner(access)?;
        if let Some(response) = self
            .store
            .replay_user_package_refresh(
                access.principal().clone(),
                user_package_id,
                expected_revision,
                key.clone(),
            )
            .await?
        {
            return Ok(response);
        }
        let current = self
            .store
            .get_user_package(access.principal().clone(), user_package_id)
            .await?;
        let snapshot = self.adapter.snapshot(current.source_root_path).await?;
        if snapshot.source_identity_key != current.source_identity_key
            || snapshot.package_id != current.package_id
        {
            return Err(UserPackageError::new(UserPackageErrorCode::SourceChanged));
        }
        self.store
            .refresh_user_package(
                access.principal().clone(),
                user_package_id,
                expected_revision,
                snapshot,
                key,
                now_ms()?,
            )
            .await
    }

    pub async fn remove(
        &self,
        access: &AccessContext,
        user_package_id: UserPackageId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<UserPackageRemoveResult, UserPackageError> {
        require_local_owner(access)?;
        self.store
            .remove_user_package(
                access.principal().clone(),
                user_package_id,
                expected_revision,
                key,
                now_ms()?,
            )
            .await
    }
}

fn require_local_owner(access: &AccessContext) -> Result<(), UserPackageError> {
    require(access, Permission::PackagesManage)?;
    if access.principal().as_str() != PrincipalId::LOCAL_OWNER {
        return Err(UserPackageError::new(
            UserPackageErrorCode::PermissionDenied,
        ));
    }
    Ok(())
}

fn require(access: &AccessContext, permission: Permission) -> Result<(), UserPackageError> {
    access
        .require(permission)
        .map_err(|_| UserPackageError::new(UserPackageErrorCode::PermissionDenied))
}

fn now_ms() -> Result<u64, UserPackageError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| UserPackageError::new(UserPackageErrorCode::Internal))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|_| UserPackageError::new(UserPackageErrorCode::Internal))
        })
}

mod sha256_hex {
    use serde::{Deserialize, Deserializer, Serializer, de::Error, ser::Error as _};

    pub fn serialize<S: Serializer>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        let text = value
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if text.len() != 64 {
            return Err(S::Error::custom("invalid SHA-256"));
        }
        serializer.serialize_str(&text)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() != 64 {
            return Err(D::Error::custom("invalid SHA-256"));
        }
        let mut result = [0_u8; 32];
        for (index, target) in result.iter_mut().enumerate() {
            *target = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
                .map_err(D::Error::custom)?;
        }
        Ok(result)
    }
}
