use rusqlite::Connection;

const CURRENT_VERSION: i32 = 4;

pub fn run_migrations(conn: &mut Connection) -> Result<(), String> {
    let version: i32 = conn
        .query_row(
            "SELECT user_version FROM pragma_user_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if version < 4 {
        migrate_v4(conn)?;
    }

    conn.pragma_update(None, "user_version", CURRENT_VERSION)
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn migrate_v4(conn: &mut Connection) -> Result<(), String> {
    // Clean-slate migration: drop old workspace/annotation tables and create only recent_workspaces cache.
    conn.execute_batch(
        "
        DROP TABLE IF EXISTS bookmarks;
        DROP TABLE IF EXISTS annotations;
        DROP TABLE IF EXISTS trajectory_sources;
        DROP TABLE IF EXISTS workspaces;

        CREATE TABLE IF NOT EXISTS recent_workspaces (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            last_opened TEXT NOT NULL
        );
        "
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
