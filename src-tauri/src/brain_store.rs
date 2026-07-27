use crate::protocol::{
    BrainThreadRecord, BrainThreadStatus, BrainTurnRecord, BrainTurnStatus, HostError,
};
use rusqlite::{params, Connection, OptionalExtension, Row};

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS brain_threads (
                id TEXT PRIMARY KEY NOT NULL,
                project_id TEXT,
                title TEXT,
                model TEXT,
                status TEXT NOT NULL,
                title_overridden INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_brain_threads_project
                ON brain_threads(project_id, updated_at DESC);
            CREATE TABLE IF NOT EXISTS brain_turns (
                id TEXT PRIMARY KEY NOT NULL,
                thread_id TEXT NOT NULL,
                status TEXT NOT NULL,
                input_text TEXT NOT NULL,
                assistant_text TEXT NOT NULL,
                error TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(thread_id) REFERENCES brain_threads(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_brain_turns_thread
                ON brain_turns(thread_id, created_at ASC);
            "#,
        )
        .map_err(sql_error)?;
    ensure_thread_title_override_column(connection)
}

fn ensure_thread_title_override_column(connection: &Connection) -> Result<(), HostError> {
    let mut statement = connection
        .prepare("PRAGMA table_info(brain_threads)")
        .map_err(sql_error)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    if !columns.iter().any(|column| column == "title_overridden") {
        connection
            .execute(
                "ALTER TABLE brain_threads ADD COLUMN title_overridden INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

pub fn upsert_thread(
    connection: &Connection,
    thread: &BrainThreadRecord,
) -> Result<BrainThreadRecord, HostError> {
    validate_thread(thread)?;
    connection
        .execute(
            "INSERT INTO brain_threads
             (id, project_id, title, model, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                project_id = COALESCE(excluded.project_id, brain_threads.project_id),
                title = CASE
                    WHEN brain_threads.title_overridden = 1 THEN brain_threads.title
                    ELSE COALESCE(excluded.title, brain_threads.title)
                END,
                model = COALESCE(excluded.model, brain_threads.model),
                status = excluded.status,
                updated_at = MAX(excluded.updated_at, brain_threads.updated_at)",
            params![
                thread.id,
                thread.project_id,
                thread.title,
                thread.model,
                thread_status_to_db(&thread.status),
                thread.created_at,
                thread.updated_at,
            ],
        )
        .map_err(sql_error)?;
    get_thread(connection, &thread.id)
}

pub fn get_thread(
    connection: &Connection,
    thread_id: &str,
) -> Result<BrainThreadRecord, HostError> {
    connection
        .query_row(
            "SELECT id, project_id, title, model, status, created_at, updated_at
             FROM brain_threads WHERE id = ?1",
            [thread_id],
            thread_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("BRAIN_THREAD_NOT_FOUND", "brain thread not found", false))
}

pub fn list_threads(
    connection: &Connection,
    project_id: Option<&str>,
) -> Result<Vec<BrainThreadRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, project_id, title, model, status, created_at, updated_at
             FROM brain_threads
             WHERE (?1 IS NULL OR project_id = ?1)
             ORDER BY updated_at DESC, id ASC",
        )
        .map_err(sql_error)?;
    let threads = statement
        .query_map([project_id], thread_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(threads)
}

pub fn set_thread_status(
    connection: &Connection,
    thread_id: &str,
    status: &BrainThreadStatus,
    updated_at: i64,
) -> Result<BrainThreadRecord, HostError> {
    let changed = connection
        .execute(
            "UPDATE brain_threads
             SET status = ?2, updated_at = MAX(?3, updated_at)
             WHERE id = ?1",
            params![thread_id, thread_status_to_db(status), updated_at],
        )
        .map_err(sql_error)?;
    if changed == 0 {
        return Err(HostError::new(
            "BRAIN_THREAD_NOT_FOUND",
            "brain thread not found",
            false,
        ));
    }
    get_thread(connection, thread_id)
}

pub fn set_thread_title(
    connection: &Connection,
    thread_id: &str,
    title: &str,
    updated_at: i64,
) -> Result<BrainThreadRecord, HostError> {
    let changed = connection
        .execute(
            "UPDATE brain_threads
             SET title = ?2, title_overridden = 1, updated_at = MAX(?3, updated_at)
             WHERE id = ?1",
            params![thread_id, title, updated_at],
        )
        .map_err(sql_error)?;
    if changed == 0 {
        return Err(HostError::new(
            "BRAIN_THREAD_NOT_FOUND",
            "brain thread not found",
            false,
        ));
    }
    get_thread(connection, thread_id)
}

pub fn delete_thread(connection: &Connection, thread_id: &str) -> Result<(), HostError> {
    connection
        .execute("DELETE FROM brain_turns WHERE thread_id = ?1", [thread_id])
        .map_err(sql_error)?;
    let changed = connection
        .execute("DELETE FROM brain_threads WHERE id = ?1", [thread_id])
        .map_err(sql_error)?;
    if changed == 0 {
        return Err(HostError::new(
            "BRAIN_THREAD_NOT_FOUND",
            "brain thread not found",
            false,
        ));
    }
    Ok(())
}

pub fn insert_turn(
    connection: &Connection,
    turn: &BrainTurnRecord,
) -> Result<BrainTurnRecord, HostError> {
    validate_turn(turn)?;
    get_thread(connection, &turn.thread_id)?;
    connection
        .execute(
            "INSERT INTO brain_turns
             (id, thread_id, status, input_text, assistant_text, error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                turn.id,
                turn.thread_id,
                turn_status_to_db(&turn.status),
                turn.input_text,
                turn.assistant_text,
                turn.error,
                turn.created_at,
                turn.updated_at,
            ],
        )
        .map_err(|error| {
            if is_constraint(&error) {
                HostError::new("BRAIN_TURN_EXISTS", "brain turn already exists", false)
            } else {
                sql_error(error)
            }
        })?;
    get_turn(connection, &turn.id)
}

pub fn finish_turn(
    connection: &Connection,
    turn_id: &str,
    status: BrainTurnStatus,
    assistant_text: &str,
    error: Option<&str>,
    updated_at: i64,
) -> Result<BrainTurnRecord, HostError> {
    if matches!(status, BrainTurnStatus::Running) {
        return Err(HostError::validation(
            "finishTurn requires a terminal turn status",
        ));
    }
    let changed = connection
        .execute(
            "UPDATE brain_turns SET status = ?2, assistant_text = ?3, error = ?4,
                    updated_at = ?5
             WHERE id = ?1 AND status = 'running'",
            params![
                turn_id,
                turn_status_to_db(&status),
                assistant_text,
                error,
                updated_at,
            ],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::conflict(
            "brain turn is missing or already terminal",
        ));
    }
    get_turn(connection, turn_id)
}

pub fn get_turn(connection: &Connection, turn_id: &str) -> Result<BrainTurnRecord, HostError> {
    connection
        .query_row(
            "SELECT id, thread_id, status, input_text, assistant_text, error, created_at, updated_at
             FROM brain_turns WHERE id = ?1",
            [turn_id],
            turn_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("BRAIN_TURN_NOT_FOUND", "brain turn not found", false))
}

pub fn list_turns(
    connection: &Connection,
    thread_id: &str,
) -> Result<Vec<BrainTurnRecord>, HostError> {
    let mut statement = connection
        .prepare(
            "SELECT id, thread_id, status, input_text, assistant_text, error, created_at, updated_at
             FROM brain_turns WHERE thread_id = ?1 ORDER BY created_at ASC, id ASC",
        )
        .map_err(sql_error)?;
    let turns = statement
        .query_map([thread_id], turn_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(turns)
}

fn validate_thread(thread: &BrainThreadRecord) -> Result<(), HostError> {
    if thread.id.trim().is_empty() {
        return Err(HostError::validation("brain thread id is required"));
    }
    if thread.created_at <= 0 || thread.updated_at <= 0 {
        return Err(HostError::validation(
            "brain thread timestamps must be positive",
        ));
    }
    Ok(())
}

fn validate_turn(turn: &BrainTurnRecord) -> Result<(), HostError> {
    if turn.id.trim().is_empty() || turn.thread_id.trim().is_empty() {
        return Err(HostError::validation(
            "brain turn and thread ids are required",
        ));
    }
    if turn.input_text.chars().count() > 100_000 {
        return Err(HostError::validation("brain turn input is too large"));
    }
    Ok(())
}

fn thread_from_row(row: &Row<'_>) -> rusqlite::Result<BrainThreadRecord> {
    let raw_status: String = row.get(4)?;
    let status = thread_status_from_db(&raw_status).map_err(|message| conversion(4, message))?;
    Ok(BrainThreadRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        model: row.get(3)?,
        status,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn turn_from_row(row: &Row<'_>) -> rusqlite::Result<BrainTurnRecord> {
    let raw_status: String = row.get(2)?;
    let status = turn_status_from_db(&raw_status).map_err(|message| conversion(2, message))?;
    Ok(BrainTurnRecord {
        id: row.get(0)?,
        thread_id: row.get(1)?,
        status,
        input_text: row.get(3)?,
        assistant_text: row.get(4)?,
        error: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn thread_status_to_db(status: &BrainThreadStatus) -> &'static str {
    match status {
        BrainThreadStatus::Ready => "ready",
        BrainThreadStatus::Running => "running",
        BrainThreadStatus::Error => "error",
        BrainThreadStatus::Archived => "archived",
    }
}

fn thread_status_from_db(value: &str) -> Result<BrainThreadStatus, String> {
    match value {
        "ready" => Ok(BrainThreadStatus::Ready),
        "running" => Ok(BrainThreadStatus::Running),
        "error" => Ok(BrainThreadStatus::Error),
        "archived" => Ok(BrainThreadStatus::Archived),
        _ => Err(format!("unknown brain thread status: {value}")),
    }
}

fn turn_status_to_db(status: &BrainTurnStatus) -> &'static str {
    match status {
        BrainTurnStatus::Running => "running",
        BrainTurnStatus::Completed => "completed",
        BrainTurnStatus::Interrupted => "interrupted",
        BrainTurnStatus::Failed => "failed",
    }
}

fn turn_status_from_db(value: &str) -> Result<BrainTurnStatus, String> {
    match value {
        "running" => Ok(BrainTurnStatus::Running),
        "completed" => Ok(BrainTurnStatus::Completed),
        "interrupted" => Ok(BrainTurnStatus::Interrupted),
        "failed" => Ok(BrainTurnStatus::Failed),
        _ => Err(format!("unknown brain turn status: {value}")),
    }
}

fn conversion(column: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn is_constraint(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!("brain SQLite operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .unwrap();
        migrate(&connection).unwrap();
        connection
    }

    fn thread() -> BrainThreadRecord {
        BrainThreadRecord {
            id: "thread-1".to_string(),
            project_id: Some("project-1".to_string()),
            title: Some("Creative brief".to_string()),
            model: Some("model".to_string()),
            status: BrainThreadStatus::Ready,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn thread_and_turn_are_durable() {
        let connection = database();
        upsert_thread(&connection, &thread()).unwrap();
        let turn = BrainTurnRecord {
            id: "turn-1".to_string(),
            thread_id: "thread-1".to_string(),
            status: BrainTurnStatus::Running,
            input_text: "Draft the brief".to_string(),
            assistant_text: String::new(),
            error: None,
            created_at: 2,
            updated_at: 2,
        };
        insert_turn(&connection, &turn).unwrap();
        let completed = finish_turn(
            &connection,
            "turn-1",
            BrainTurnStatus::Completed,
            "Done",
            None,
            3,
        )
        .unwrap();
        assert_eq!(completed.assistant_text, "Done");
        assert_eq!(
            list_threads(&connection, Some("project-1")).unwrap().len(),
            1
        );
        assert_eq!(list_turns(&connection, "thread-1").unwrap().len(), 1);
    }

    #[test]
    fn custom_thread_title_survives_remote_upserts() {
        let connection = database();
        let mut remote = thread();
        upsert_thread(&connection, &remote).unwrap();
        let renamed = set_thread_title(&connection, &remote.id, "客户报价讨论", 2).unwrap();
        assert_eq!(renamed.title.as_deref(), Some("客户报价讨论"));

        remote.title = Some("Remote generated title".to_string());
        remote.updated_at = 3;
        let refreshed = upsert_thread(&connection, &remote).unwrap();
        assert_eq!(refreshed.title.as_deref(), Some("客户报价讨论"));
    }

    #[test]
    fn terminal_turn_cannot_complete_twice() {
        let connection = database();
        upsert_thread(&connection, &thread()).unwrap();
        insert_turn(
            &connection,
            &BrainTurnRecord {
                id: "turn-1".to_string(),
                thread_id: "thread-1".to_string(),
                status: BrainTurnStatus::Running,
                input_text: "x".to_string(),
                assistant_text: String::new(),
                error: None,
                created_at: 2,
                updated_at: 2,
            },
        )
        .unwrap();
        finish_turn(
            &connection,
            "turn-1",
            BrainTurnStatus::Interrupted,
            "partial",
            None,
            3,
        )
        .unwrap();
        assert_eq!(
            finish_turn(
                &connection,
                "turn-1",
                BrainTurnStatus::Completed,
                "late",
                None,
                4,
            )
            .unwrap_err()
            .code,
            "REVISION_CONFLICT"
        );
    }
}
