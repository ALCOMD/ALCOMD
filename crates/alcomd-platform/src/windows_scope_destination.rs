use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use semver::Version;

use super::{
    CORE_EXECUTABLE_NAME, InstallationTrust, MAX_INTENT_BYTES, WINDOWS_ACTIVE_RUNTIME_RECORD,
    WINDOWS_BOOTSTRAP_EXECUTABLE_NAME, active_version, installation_not_ready,
    invalid_registration, lease_runtime, maintenance_instance, parse_version, publish_owned_record,
    remove_maintenance_intent, require_maintenance_intent, require_no_maintenance_record,
    require_no_scope_transition, require_no_update_intent, validate_maintenance_intent,
    windows_current_user_installation_root, windows_machine_installation_root,
};
use crate::{DaemonInstance, resolve_plain_storage_directory, sync_directory};

/// Exact scope plan accepted for a signed, inactive destination runtime.
pub const WINDOWS_SCOPE_DESTINATION_RECORD: &str = "scope-destination-v1";

/// Exact scope plan approved for activation after the previous scope was retired.
pub const WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD: &str = "scope-destination-commit-v1";

/// Holds the destination's manifest lease across verification and activation.
/// Bootstrap must verify both payloads, health, and retirement of the source scope.
pub struct WindowsScopeTransitionDestination {
    root: PathBuf,
    root_identity: Vec<u8>,
    runtime: PathBuf,
    runtime_identity: Vec<u8>,
    version: Version,
    trust: InstallationTrust,
    _lease: File,
    _instance: Option<DaemonInstance>,
}

struct DestinationState {
    prepared: Option<File>,
    commit: Option<File>,
    active: bool,
}

impl WindowsScopeTransitionDestination {
    /// Acquires the staged destination in its standard product root and token scope.
    pub fn acquire(machine: bool, version: &str) -> io::Result<Self> {
        if crate::windows_process_is_elevated()? != machine {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let (root, trust) = if machine {
            (
                windows_machine_installation_root()?,
                InstallationTrust::Machine,
            )
        } else {
            (
                windows_current_user_installation_root()?,
                InstallationTrust::User,
            )
        };
        let mut destination = Self::acquire_at(root, trust, parse_version(version)?)?;
        destination._instance = maintenance_instance(trust)?;
        Ok(destination)
    }

    fn acquire_at(root: PathBuf, trust: InstallationTrust, version: Version) -> io::Result<Self> {
        let (resolved_root, root_identity) = resolve_plain_storage_directory(&root)?;
        if root != resolved_root {
            return Err(invalid_registration());
        }
        let runtime = root.join("runtime").join(version.to_string());
        trust.check_executable(&runtime.join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME))?;
        trust.check_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
        let (resolved_runtime, runtime_identity) = resolve_plain_storage_directory(&runtime)?;
        if runtime != resolved_runtime {
            return Err(invalid_registration());
        }
        let lease = lease_runtime(&runtime, true, trust)?;
        require_no_update_intent(&root)?;
        require_no_scope_transition(&root)?;
        Ok(Self {
            root,
            root_identity,
            runtime,
            runtime_identity,
            version,
            trust,
            _lease: lease,
            _instance: None,
        })
    }

    /// The destination directory to verify before recording or activating it.
    #[must_use]
    pub fn runtime(&self) -> &Path {
        &self.runtime
    }

    /// Reads the protected plan for recovery, still subject to Bootstrap's proof checks.
    pub fn prepared_plan(&self) -> io::Result<Option<Vec<u8>>> {
        self.validate_destination()?;
        if let Some(prepared) = open_prepared(self.trust, &self.root)? {
            let plan = read_plan(&prepared)?;
            self.inspect(&plan)?;
            Ok(Some(plan))
        } else {
            require_no_maintenance_record(&self.root, WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD)?;
            Ok(None)
        }
    }

    /// Persists the exact authorized plan without making the destination active.
    pub fn prepare(&self, intent: &[u8]) -> io::Result<()> {
        let state = self.inspect(intent)?;
        if state.prepared.is_some() {
            return sync_directory(&self.root);
        }
        publish_owned_record(
            &self.root.join(WINDOWS_SCOPE_DESTINATION_RECORD),
            intent,
            None,
            self.trust,
        )
    }

    /// Returns the destination phase for the exact authorized plan.
    pub fn phase(&self, intent: &[u8]) -> io::Result<&'static str> {
        let state = self.inspect(intent)?;
        if state.active {
            Ok("active")
        } else if state.prepared.is_some() {
            Ok("prepared")
        } else {
            Ok("inactive")
        }
    }

    /// Durably commits the verified scope change and publishes the sole active selector.
    /// The caller must first verify retirement of the source for this exact plan.
    pub fn activate(&self, intent: &[u8]) -> io::Result<()> {
        let state = self.inspect(intent)?;
        let _prepared = state.prepared.ok_or_else(installation_not_ready)?;
        if state.active {
            return sync_directory(&self.root);
        }
        let _commit = match state.commit {
            Some(receipt) => receipt,
            None => {
                let path = self.root.join(WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD);
                publish_owned_record(&path, intent, None, self.trust)?;
                let receipt = self.trust.open_record(&path)?;
                require_maintenance_intent(&receipt, intent)?;
                receipt
            }
        };
        publish_owned_record(
            &self.root.join(WINDOWS_ACTIVE_RUNTIME_RECORD),
            format!("{}\n", self.version).as_bytes(),
            None,
            self.trust,
        )
    }

    /// Removes this exact preparation when the destination has not committed or activated.
    pub fn abort(&self, intent: &[u8]) -> io::Result<()> {
        let state = self.inspect(intent)?;
        if state.active || state.commit.is_some() {
            return Err(installation_not_ready());
        }
        drop(state);
        remove_maintenance_intent(
            &self.root,
            WINDOWS_SCOPE_DESTINATION_RECORD,
            intent,
            self.trust,
        )
    }

    fn inspect(&self, intent: &[u8]) -> io::Result<DestinationState> {
        validate_maintenance_intent(intent)?;
        self.validate_destination()?;
        let prepared = self.open_matching_record(WINDOWS_SCOPE_DESTINATION_RECORD, intent)?;
        let commit = self.open_matching_record(WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD, intent)?;
        let active = match active_version(&self.root, self.trust) {
            Ok(version) if version == self.version => true,
            Ok(_) => return Err(installation_not_ready()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(error) => return Err(error),
        };
        if (commit.is_some() && prepared.is_none()) || (active && commit.is_none()) {
            return Err(installation_not_ready());
        }
        Ok(DestinationState {
            prepared,
            commit,
            active,
        })
    }

    fn validate_destination(&self) -> io::Result<()> {
        if resolve_plain_storage_directory(&self.root)?.1 != self.root_identity
            || resolve_plain_storage_directory(&self.runtime)?.1 != self.runtime_identity
        {
            return Err(invalid_registration());
        }
        require_no_update_intent(&self.root)?;
        require_no_scope_transition(&self.root)
    }

    fn open_matching_record(&self, name: &str, intent: &[u8]) -> io::Result<Option<File>> {
        match self.trust.open_record(&self.root.join(name)) {
            Ok(file) => {
                require_maintenance_intent(&file, intent)?;
                Ok(Some(file))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn open_prepared(trust: InstallationTrust, root: &Path) -> io::Result<Option<File>> {
    match trust.open_record(&root.join(WINDOWS_SCOPE_DESTINATION_RECORD)) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_plan(prepared: &File) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    prepared
        .take((MAX_INTENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    validate_maintenance_intent(&bytes)?;
    Ok(bytes)
}

pub(super) fn require_committed_scope_destination(
    root: &Path,
    trust: InstallationTrust,
) -> io::Result<()> {
    let Some(prepared) = open_prepared(trust, root)? else {
        return require_no_maintenance_record(root, WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD);
    };
    let bytes = read_plan(&prepared)?;
    let commit = trust
        .open_record(&root.join(WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD))
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                installation_not_ready()
            } else {
                error
            }
        })?;
    require_maintenance_intent(&commit, &bytes)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::{
        WINDOWS_CLI_EXECUTABLE_NAME, WINDOWS_GUI_EXECUTABLE_NAME, hold_windows_installation,
        windows_installation_bootstrap,
    };
    use super::*;
    use crate::PRODUCT_PAYLOAD_MANIFEST_NAME;

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    const ACTIVE: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\n").as_bytes();

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> io::Result<Self> {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos();
            let parent = crate::local_app_data_directory()?.join(format!(
                "alcomd-scope-destination-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&parent)?;
            let fixture = Self(fs::canonicalize(parent)?);
            let runtime = fixture.0.join("runtime").join(VERSION);
            fs::create_dir_all(&runtime)?;
            fs::write(runtime.join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME), [])?;
            fs::write(runtime.join(CORE_EXECUTABLE_NAME), [])?;
            fs::write(
                runtime.join(PRODUCT_PAYLOAD_MANIFEST_NAME),
                b"signed fixture",
            )?;
            Ok(fixture)
        }

        fn acquire(&self) -> io::Result<WindowsScopeTransitionDestination> {
            // The standard-root entry point takes an account mutex. Keep the
            // publication race test independent of concurrently running IPC tests.
            WindowsScopeTransitionDestination::acquire_at(
                self.0.clone(),
                InstallationTrust::User,
                parse_version(VERSION)?,
            )
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn scope_destination_rejects_foreign_active_selector_and_recovers_commit() -> io::Result<()> {
        let scope = Fixture::new()?;
        let plan = b"exact approved source and destination";
        let active = scope.0.join(WINDOWS_ACTIVE_RUNTIME_RECORD);
        let prepared = scope.0.join(WINDOWS_SCOPE_DESTINATION_RECORD);
        let commit = scope.0.join(WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD);
        let destination = scope.acquire()?;
        assert_eq!(destination.phase(plan)?, "inactive");
        assert_eq!(destination.prepared_plan()?, None);
        assert!(destination.activate(plan).is_err());
        assert!(!active.exists());

        destination.prepare(plan)?;
        destination.prepare(plan)?;
        assert_eq!(destination.phase(plan)?, "prepared");
        assert_eq!(
            destination.prepared_plan()?.as_deref(),
            Some(plan.as_slice())
        );
        assert_eq!(fs::read(&prepared)?, plan);
        assert!(destination.prepare(b"different plan").is_err());
        assert!(scope.acquire().is_err(), "must hold the manifest lease");

        let bin = scope.0.join("bin");
        let cli = bin.join(WINDOWS_CLI_EXECUTABLE_NAME);
        let gui = destination.runtime().join(WINDOWS_GUI_EXECUTABLE_NAME);
        let core = destination.runtime().join(CORE_EXECUTABLE_NAME);
        let bootstrap = destination
            .runtime()
            .join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME);
        fs::create_dir(&bin)?;
        fs::write(&cli, [])?;
        fs::write(&gui, [])?;
        fs::write(&active, ACTIVE)?;
        assert!(destination.activate(plan).is_err());
        drop(destination);
        assert!(require_committed_scope_destination(&scope.0, InstallationTrust::User).is_err());
        assert!(hold_windows_installation(&core).is_err());
        assert!(windows_installation_bootstrap(&cli).is_err());
        assert!(windows_installation_bootstrap(&gui).is_err());
        fs::remove_file(&active)?;

        // Simulate an interruption immediately after the durable commit record.
        fs::write(&commit, plan)?;
        let resumed = scope.acquire()?;
        assert!(resumed.abort(plan).is_err());
        resumed.activate(plan)?;
        resumed.activate(plan)?;
        assert!(resumed.activate(b"different plan").is_err());
        assert_eq!(resumed.phase(plan)?, "active");
        assert_eq!(fs::read(&active)?, ACTIVE);
        drop(resumed);
        assert!(hold_windows_installation(&core)?.is_some());
        assert_eq!(
            windows_installation_bootstrap(&cli)?,
            Some(bootstrap.clone())
        );
        assert_eq!(windows_installation_bootstrap(&gui)?, Some(bootstrap));
        Ok(())
    }

    #[test]
    fn scope_destination_aborts_only_exact_uncommitted_preparation() -> io::Result<()> {
        let scope = Fixture::new()?;
        let plan = b"approved scope plan";
        let destination = scope.acquire()?;
        assert!(destination.prepare(b"").is_err());
        assert!(
            destination
                .prepare(&vec![b'x'; MAX_INTENT_BYTES + 1])
                .is_err()
        );
        destination.prepare(plan)?;
        assert!(destination.abort(b"different plan").is_err());
        destination.abort(plan)?;
        destination.abort(plan)?;
        assert_eq!(destination.phase(plan)?, "inactive");
        assert!(destination.runtime().is_dir());
        assert!(!scope.0.join(WINDOWS_ACTIVE_RUNTIME_RECORD).exists());

        destination.prepare(plan)?;
        destination.activate(plan)?;
        assert_eq!(destination.phase(plan)?, "active");
        assert_eq!(
            fs::read(scope.0.join(WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD))?,
            plan
        );
        assert!(destination.abort(plan).is_err());
        require_committed_scope_destination(&scope.0, InstallationTrust::User)
    }
}
