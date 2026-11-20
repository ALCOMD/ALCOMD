use alcomd_application::{Permission, PrincipalId, ToolAuthentication, ToolError};
use rusqlite::Connection;
use std::cell::RefCell;

thread_local! {
    static AUTHENTICATION: RefCell<Option<ToolAuthentication>> = const { RefCell::new(None) };
}

pub(super) struct ContextGuard(Option<ToolAuthentication>);

impl ContextGuard {
    pub(super) fn install(authentication: Option<ToolAuthentication>) -> Self {
        Self(AUTHENTICATION.with(|slot| slot.replace(authentication)))
    }
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        AUTHENTICATION.with(|slot| slot.replace(self.0.take()));
    }
}

pub(super) fn require(
    db: &Connection,
    caller: &PrincipalId,
    permissions: &[Permission],
) -> Result<(), ToolError> {
    if !caller.as_str().starts_with("mcp:") {
        return Ok(());
    }
    let authentication = AUTHENTICATION
        .with(|slot| slot.borrow().clone())
        .ok_or(ToolError::PermissionDenied)?;
    let client = super::m8_tools::authenticate(db, &authentication)?;
    if &client.principal_id != caller {
        return Err(ToolError::PermissionDenied);
    }
    let access = authentication.access(client)?;
    for permission in permissions {
        access
            .require(*permission)
            .map_err(|_| ToolError::PermissionDenied)?;
    }
    Ok(())
}
