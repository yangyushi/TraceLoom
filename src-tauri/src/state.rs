use std::sync::Mutex;
use rusqlite::Connection;

pub struct AppState {
    pub db_path: std::path::PathBuf,
    pub conn: Mutex<Connection>,
}

impl AppState {
    pub fn new(db_path: std::path::PathBuf, conn: Connection) -> Self {
        Self {
            db_path,
            conn: Mutex::new(conn),
        }
    }
}
