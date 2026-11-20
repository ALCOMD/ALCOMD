use rusqlite::Connection;

const INITIAL_SCHEMA: &str = include_str!("../../../alcomd-store/src/schema.sql");

pub fn current() -> Connection {
    assert_eq!(alcomd_store::CURRENT_DATA_SCHEMA, 1);
    let db = Connection::open_in_memory().expect("open isolated State database");
    db.execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable foreign keys");
    db.execute_batch(INITIAL_SCHEMA)
        .expect("execute current State initialization");
    db
}

pub fn definition(db: &Connection, kind: &str, name: &str) -> String {
    db.query_row(
        "SELECT sql FROM sqlite_schema WHERE type=?1 AND name=?2",
        [kind, name],
        |row| row.get(0),
    )
    .unwrap_or_else(|error| panic!("missing {kind} {name}: {error}"))
}
