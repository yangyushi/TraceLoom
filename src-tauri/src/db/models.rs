use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecentWorkspace {
    pub id: i64,
    pub file_path: String,
    pub name: String,
    pub last_opened: DateTime<Utc>,
}
