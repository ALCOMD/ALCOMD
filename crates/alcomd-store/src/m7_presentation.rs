use alcomd_application::{
    InstalledPackagePresentation, MAX_PRESENTATION_BYTES, MAX_PRESENTATION_PACKAGES,
    PresentationState, ProjectId, ProjectPackagePresentations,
};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn write(
    connection: &Connection,
    project: ProjectId,
    project_revision: u64,
    value: &ProjectPackagePresentations,
) -> rusqlite::Result<bool> {
    if value.state == PresentationState::Unobserved {
        return Ok(false);
    }
    if value.packages.len() > MAX_PRESENTATION_PACKAGES
        || value.fingerprint.len() != 64
        || !value.fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let encoded = value
        .packages
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    if encoded.iter().map(String::len).sum::<usize>() > MAX_PRESENTATION_BYTES {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let project_revision =
        i64::try_from(project_revision).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let project = project.to_string();
    let previous: Option<(String, i64)> = connection.query_row(
        "SELECT fingerprint,project_revision FROM project_presentation_observations WHERE project_id=?1",
        [&project], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?;
    if previous.as_ref().is_some_and(|(fingerprint, revision)| {
        fingerprint == &value.fingerprint && *revision == project_revision
    }) {
        return Ok(false);
    }
    let state = match value.state {
        PresentationState::Complete => "complete",
        PresentationState::Partial => "partial",
        PresentationState::Unobserved => return Ok(false),
    };
    connection.execute(
        "DELETE FROM project_package_presentations WHERE project_id=?1",
        [&project],
    )?;
    for (package, json) in value.packages.iter().zip(encoded) {
        connection.execute(
            "INSERT INTO project_package_presentations(project_id,package_id,presentation_json) VALUES(?1,?2,?3)",
            params![project, package.package_id, json],
        )?;
    }
    connection.execute(
        "INSERT INTO project_presentation_observations(project_id,revision,project_revision,state,fingerprint)
         VALUES(?1,1,?2,?3,?4) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1,
         project_revision=excluded.project_revision,state=excluded.state,fingerprint=excluded.fingerprint",
        params![project, project_revision, state, value.fingerprint],
    )?;
    Ok(true)
}

pub(super) fn load(
    connection: &Connection,
    project: ProjectId,
    project_revision: u64,
) -> rusqlite::Result<ProjectPackagePresentations> {
    let project_revision =
        i64::try_from(project_revision).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let project = project.to_string();
    let observation: Option<(String, String, i64)> = connection.query_row(
        "SELECT state,fingerprint,project_revision FROM project_presentation_observations WHERE project_id=?1",
        [&project], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).optional()?;
    let Some((state, fingerprint, revision)) = observation else {
        return Ok(ProjectPackagePresentations::default());
    };
    if revision != project_revision {
        return Ok(ProjectPackagePresentations {
            state: PresentationState::Partial,
            ..Default::default()
        });
    }
    let state = match state.as_str() {
        "complete" => PresentationState::Complete,
        "partial" => PresentationState::Partial,
        "unobserved" => PresentationState::Unobserved,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let mut statement = connection.prepare("SELECT presentation_json FROM project_package_presentations WHERE project_id=?1 ORDER BY package_id LIMIT 4097")?;
    let encoded = statement
        .query_map([&project], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if encoded.len() > MAX_PRESENTATION_PACKAGES
        || encoded.iter().map(String::len).sum::<usize>() > MAX_PRESENTATION_BYTES
    {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let packages = encoded
        .into_iter()
        .map(|value| {
            serde_json::from_str::<InstalledPackagePresentation>(&value)
                .map_err(|_| rusqlite::Error::InvalidQuery)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProjectPackagePresentations {
        state,
        packages,
        fingerprint,
    })
}
