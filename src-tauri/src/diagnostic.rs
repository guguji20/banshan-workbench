use crate::protocol::{
    DiagnosticRecord, DiagnosticReportPayload, DiagnosticSeverity, DiagnosticStatus, HostError,
};
use rusqlite::{params, Connection, OptionalExtension, Row, TransactionBehavior};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_MESSAGE_CHARS: usize = 8_192;
const MAX_MESSAGE_INPUT_CHARS: usize = 65_536;
const MAX_CONTEXT_BYTES: usize = 65_536;
const MAX_CONTEXT_DEPTH: usize = 8;
const MAX_CONTEXT_NODES: usize = 512;
const MAX_CONTEXT_TEXT_CHARS: usize = 32_768;
const MAX_CONTEXT_STRING_INPUT_CHARS: usize = 16_384;
const MAX_CONTEXT_STRING_CHARS: usize = 2_048;
const MAX_CONTAINER_ITEMS: usize = 64;
const MAX_IDENTIFIER_CHARS: usize = 256;
const MAX_QUEUED_BATCH: usize = 500;
const REDACTED: &str = "[REDACTED]";
const REDACTED_PATH: &str = "[REDACTED_PATH]";
const TRUNCATED: &str = "[TRUNCATED]";

const DIAGNOSTIC_COLUMNS: &str = "id, fingerprint, code, message, component, severity, status, \
    trace_id, project_id, context_json, occurrences, first_seen_at, last_seen_at, uploaded_at";

/// Durable local-only queue. Upload policy and network transport deliberately live elsewhere.
pub struct DiagnosticOutbox {
    connection: Mutex<Connection>,
}

impl DiagnosticOutbox {
    pub fn open(database_path: &Path) -> Result<Self, HostError> {
        if let Some(parent) = database_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                HostError::internal(format!("create diagnostic data directory failed: {error}"))
            })?;
        }
        let connection = Connection::open(database_path).map_err(sql_error)?;
        Self::from_connection(connection)
    }

    pub fn from_connection(connection: Connection) -> Result<Self, HostError> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(sql_error)?;
        migrate(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn migrate(connection: &Connection) -> Result<(), HostError> {
        migrate(connection)
    }

    pub fn report(&self, payload: DiagnosticReportPayload) -> Result<DiagnosticRecord, HostError> {
        let sanitized = sanitize_payload(payload)?;
        let fingerprint = stable_fingerprint(&[
            &sanitized.code,
            &sanitized.component,
            sanitized.severity.as_db_str(),
            &sanitized.message,
        ]);
        let context_json = serde_json::to_string(&sanitized.context).map_err(json_error)?;
        if context_json.len() > MAX_CONTEXT_BYTES {
            return Err(HostError::internal(
                "sanitized diagnostic context exceeds storage limit",
            ));
        }

        let now = now_millis();
        let id = Uuid::new_v4().to_string();
        let severity = sanitized.severity.as_db_str();
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        transaction
            .execute(
                r#"
                INSERT INTO diagnostic_outbox (
                    id, fingerprint, code, message, component, severity, status,
                    trace_id, project_id, context_json, occurrences, first_seen_at,
                    last_seen_at, uploaded_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, 'queued', ?7, ?8, ?9, 1, ?10, ?10, NULL
                )
                ON CONFLICT(fingerprint) DO UPDATE SET
                    code = excluded.code,
                    message = excluded.message,
                    component = excluded.component,
                    severity = excluded.severity,
                    status = 'queued',
                    trace_id = excluded.trace_id,
                    project_id = excluded.project_id,
                    context_json = excluded.context_json,
                    occurrences = MIN(diagnostic_outbox.occurrences + 1, 4294967295),
                    last_seen_at = excluded.last_seen_at,
                    uploaded_at = NULL
                "#,
                params![
                    id,
                    fingerprint,
                    sanitized.code,
                    sanitized.message,
                    sanitized.component,
                    severity,
                    sanitized.trace_id,
                    sanitized.project_id,
                    context_json,
                    now,
                ],
            )
            .map_err(sql_error)?;
        let record = find_by_fingerprint(&transaction, &fingerprint)?.ok_or_else(|| {
            HostError::internal("diagnostic upsert committed without a readable record")
        })?;
        transaction.commit().map_err(sql_error)?;
        Ok(record)
    }

    pub fn list_queued(&self, limit: usize) -> Result<Vec<DiagnosticRecord>, HostError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit = limit.min(MAX_QUEUED_BATCH) as i64;
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {DIAGNOSTIC_COLUMNS} FROM diagnostic_outbox \
                 WHERE status = 'queued' ORDER BY first_seen_at, id LIMIT ?1"
            ))
            .map_err(sql_error)?;
        let records = statement
            .query_map([limit], record_from_row)
            .map_err(sql_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        Ok(records)
    }

    pub fn mark_uploaded(&self, ids: &[String]) -> Result<usize, HostError> {
        if ids.is_empty() {
            return Ok(0);
        }
        if ids.len() > MAX_QUEUED_BATCH {
            return Err(HostError::validation(format!(
                "diagnostic upload acknowledgement exceeds {MAX_QUEUED_BATCH} ids"
            )));
        }

        let unique_ids = ids.iter().collect::<HashSet<_>>();
        let now = now_millis();
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        let mut changed = 0;
        for id in unique_ids {
            changed += transaction
                .execute(
                    "UPDATE diagnostic_outbox SET status = 'uploaded', uploaded_at = ?2 \
                     WHERE id = ?1 AND status = 'queued'",
                    params![id, now],
                )
                .map_err(sql_error)?;
        }
        transaction.commit().map_err(sql_error)?;
        Ok(changed)
    }

    pub fn suppress(&self, id: &str) -> Result<DiagnosticRecord, HostError> {
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        let changed = transaction
            .execute(
                "UPDATE diagnostic_outbox SET status = 'suppressed', uploaded_at = NULL \
                 WHERE id = ?1",
                [id],
            )
            .map_err(sql_error)?;
        if changed == 0 {
            return Err(not_found(id));
        }
        let record = find_by_id(&transaction, id)?.ok_or_else(|| not_found(id))?;
        transaction.commit().map_err(sql_error)?;
        Ok(record)
    }

    pub fn get(&self, id: &str) -> Result<DiagnosticRecord, HostError> {
        let connection = self.lock()?;
        find_by_id(&connection, id)?.ok_or_else(|| not_found(id))
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, HostError> {
        self.connection
            .lock()
            .map_err(|_| HostError::internal("diagnostic SQLite lock is poisoned"))
    }
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS diagnostic_outbox (
                id TEXT PRIMARY KEY NOT NULL,
                fingerprint TEXT NOT NULL UNIQUE,
                code TEXT NOT NULL,
                message TEXT NOT NULL,
                component TEXT NOT NULL,
                severity TEXT NOT NULL CHECK(severity IN ('info','warning','error','critical')),
                status TEXT NOT NULL CHECK(status IN ('queued','uploaded','suppressed')),
                trace_id TEXT,
                project_id TEXT,
                context_json TEXT NOT NULL,
                occurrences INTEGER NOT NULL CHECK(occurrences BETWEEN 1 AND 4294967295),
                first_seen_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL,
                uploaded_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_diagnostic_outbox_queue
                ON diagnostic_outbox(status, first_seen_at, id);
            CREATE INDEX IF NOT EXISTS idx_diagnostic_outbox_last_seen
                ON diagnostic_outbox(last_seen_at DESC);
            "#,
        )
        .map_err(sql_error)
}

struct SanitizedPayload {
    code: String,
    message: String,
    component: String,
    severity: DiagnosticSeverity,
    trace_id: Option<String>,
    project_id: Option<String>,
    context: Value,
}

fn sanitize_payload(payload: DiagnosticReportPayload) -> Result<SanitizedPayload, HostError> {
    let code = sanitize_required_identifier("diagnostic code", &payload.code)?;
    let component = sanitize_required_identifier("diagnostic component", &payload.component)?;
    let trace_id = sanitize_optional_identifier(payload.trace_id);
    let project_id = sanitize_optional_identifier(payload.project_id);

    let bounded_message = take_chars(&payload.message, MAX_MESSAGE_INPUT_CHARS);
    let message = take_chars(&redact_text(bounded_message), MAX_MESSAGE_CHARS)
        .trim()
        .to_string();
    let message = if message.is_empty() {
        "diagnostic message was empty".to_string()
    } else {
        message
    };

    let mut budget = ContextBudget {
        nodes_left: MAX_CONTEXT_NODES,
        text_chars_left: MAX_CONTEXT_TEXT_CHARS,
        truncated: false,
    };
    let mut context = sanitize_context(&payload.context, 0, None, &mut budget);
    if budget.truncated {
        attach_truncated_marker(&mut context);
    }
    if serialized_len(&context)? > MAX_CONTEXT_BYTES {
        context = serde_json::json!({
            "_truncated": true,
            "_reason": "diagnostic context exceeded storage limit"
        });
    }

    Ok(SanitizedPayload {
        code,
        message,
        component,
        severity: payload.severity,
        trace_id,
        project_id,
        context,
    })
}

fn sanitize_required_identifier(label: &str, value: &str) -> Result<String, HostError> {
    let sanitized = take_chars(&redact_text(value.trim()), MAX_IDENTIFIER_CHARS)
        .trim()
        .to_string();
    if sanitized.is_empty() {
        return Err(HostError::validation(format!("{label} is required")));
    }
    Ok(sanitized)
}

fn sanitize_optional_identifier(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = take_chars(&redact_text(value.trim()), MAX_IDENTIFIER_CHARS)
            .trim()
            .to_string();
        (!value.is_empty()).then_some(value)
    })
}

struct ContextBudget {
    nodes_left: usize,
    text_chars_left: usize,
    truncated: bool,
}

fn sanitize_context(
    value: &Value,
    depth: usize,
    key: Option<&str>,
    budget: &mut ContextBudget,
) -> Value {
    if key.is_some_and(is_sensitive_key) {
        return Value::String(REDACTED.to_string());
    }
    if depth >= MAX_CONTEXT_DEPTH || budget.nodes_left == 0 {
        budget.truncated = true;
        return Value::String(TRUNCATED.to_string());
    }
    budget.nodes_left -= 1;

    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
        Value::String(value) => {
            let bounded = take_chars(value, MAX_CONTEXT_STRING_INPUT_CHARS);
            if bounded.len() < value.len() {
                budget.truncated = true;
            }
            let redacted = redact_text(bounded);
            let per_value = take_chars(&redacted, MAX_CONTEXT_STRING_CHARS);
            if per_value.len() < redacted.len() {
                budget.truncated = true;
            }
            let allowed = per_value.chars().count().min(budget.text_chars_left);
            if allowed < per_value.chars().count() {
                budget.truncated = true;
            }
            budget.text_chars_left = budget.text_chars_left.saturating_sub(allowed);
            Value::String(take_chars(per_value, allowed).to_string())
        }
        Value::Array(values) => {
            let mut output = Vec::with_capacity(values.len().min(MAX_CONTAINER_ITEMS) + 1);
            for value in values.iter().take(MAX_CONTAINER_ITEMS) {
                if budget.nodes_left == 0 || budget.text_chars_left == 0 {
                    budget.truncated = true;
                    break;
                }
                output.push(sanitize_context(value, depth + 1, None, budget));
            }
            if values.len() > output.len() {
                budget.truncated = true;
                output.push(Value::String(TRUNCATED.to_string()));
            }
            Value::Array(output)
        }
        Value::Object(values) => {
            let mut output = Map::new();
            for (raw_key, value) in values.iter().take(MAX_CONTAINER_ITEMS) {
                if budget.nodes_left == 0 || budget.text_chars_left == 0 {
                    budget.truncated = true;
                    break;
                }
                let key = take_chars(&redact_text(raw_key), MAX_IDENTIFIER_CHARS).to_string();
                output.insert(
                    key,
                    sanitize_context(value, depth + 1, Some(raw_key), budget),
                );
            }
            if values.len() > output.len() {
                budget.truncated = true;
                output.insert("_truncated".to_string(), Value::Bool(true));
            }
            Value::Object(output)
        }
    }
}

fn attach_truncated_marker(context: &mut Value) {
    match context {
        Value::Object(values) => {
            values.insert("_truncated".to_string(), Value::Bool(true));
        }
        Value::Array(values) => values.push(Value::String(TRUNCATED.to_string())),
        _ => {
            let original = std::mem::replace(context, Value::Null);
            *context = serde_json::json!({ "value": original, "_truncated": true });
        }
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "token"
            | "accesstoken"
            | "refreshtoken"
            | "idtoken"
            | "apikey"
            | "authorization"
            | "auth"
            | "secret"
            | "clientsecret"
            | "password"
            | "passwd"
            | "credential"
            | "credentials"
            | "cookie"
            | "setcookie"
            | "bearer"
    ) || normalized.ends_with("token")
        || normalized.ends_with("apikey")
        || normalized.ends_with("secret")
        || normalized.ends_with("password")
        || normalized.ends_with("passwd")
        || normalized.ends_with("credential")
        || normalized.ends_with("credentials")
        || normalized.ends_with("privatekey")
}

fn redact_text(input: &str) -> String {
    let urls = redact_url_queries(input);
    let assignments = redact_secret_assignments(&urls);
    let credentials = redact_credential_patterns(&assignments);
    redact_absolute_paths(&credentials)
}

fn redact_url_queries(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if url_prefix_len(input, index).is_some() {
            let end = scan_until(input, index, is_url_terminator);
            let url = &input[index..end];
            if let Some(query) = url.find('?') {
                output.push_str(&url[..query]);
                output.push_str("?[REDACTED_QUERY]");
            } else {
                output.push_str(url);
            }
            index = end;
            continue;
        }
        push_next_char(input, &mut index, &mut output);
    }
    output
}

fn redact_secret_assignments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if starts_secret_word(input, index, "bearer") {
            let after_word = index + "bearer".len();
            let value_start = skip_ascii_whitespace(input, after_word);
            if value_start > after_word && value_start < input.len() {
                output.push_str("Bearer ");
                output.push_str(REDACTED);
                index = scan_secret_value(input, value_start);
                continue;
            }
        }

        if let Some(word_len) = secret_assignment_word_len(input, index) {
            let mut separator = skip_ascii_whitespace(input, index + word_len);
            if byte_at(input, separator) == Some(b'\"') || byte_at(input, separator) == Some(b'\'')
            {
                separator += 1;
                separator = skip_ascii_whitespace(input, separator);
            }
            if matches!(byte_at(input, separator), Some(b'=') | Some(b':')) {
                let value_start = skip_ascii_whitespace(input, separator + 1);
                output.push_str(&input[index..=separator]);
                output.push_str(REDACTED);
                index = scan_secret_value(input, value_start);
                continue;
            }
        }
        push_next_char(input, &mut index, &mut output);
    }
    output
}

fn redact_credential_patterns(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if starts_ascii_case_insensitive(input, index, "sk-") {
            let end = scan_ascii_credential(input, index + 3);
            if end.saturating_sub(index) >= 8 {
                output.push_str(REDACTED);
                index = end;
                continue;
            }
        }
        if starts_ascii_case_insensitive(input, index, "akia") {
            let end = scan_ascii_credential(input, index + 4);
            if end.saturating_sub(index) >= 12 {
                output.push_str(REDACTED);
                index = end;
                continue;
            }
        }
        push_next_char(input, &mut index, &mut output);
    }
    output
}

fn redact_absolute_paths(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if url_prefix_len(input, index).is_some() {
            let end = scan_until(input, index, is_url_terminator);
            output.push_str(&input[index..end]);
            index = end;
            continue;
        }

        if is_windows_absolute_path(input, index)
            || is_unc_path(input, index)
            || is_unix_absolute_path(input, index)
            || is_tilde_path(input, index)
        {
            output.push_str(REDACTED_PATH);
            index = scan_path_end(input, index);
            continue;
        }
        push_next_char(input, &mut index, &mut output);
    }
    output
}

fn secret_assignment_word_len(input: &str, index: usize) -> Option<usize> {
    let before_ok = index == 0
        || input[..index]
            .chars()
            .next_back()
            .is_none_or(|character| !is_identifier_character(character));
    if !before_ok {
        return None;
    }
    let mut end = index;
    while let Some(byte) = byte_at(input, end) {
        if !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) {
            break;
        }
        end += 1;
    }
    (end > index && is_sensitive_key(&input[index..end])).then_some(end - index)
}

fn starts_secret_word(input: &str, index: usize, word: &str) -> bool {
    if !starts_ascii_case_insensitive(input, index, word) {
        return false;
    }
    let before_ok = index == 0
        || input[..index]
            .chars()
            .next_back()
            .is_none_or(|character| !is_identifier_character(character));
    let end = index + word.len();
    let after_ok = end == input.len()
        || input[end..]
            .chars()
            .next()
            .is_none_or(|character| !is_identifier_character(character));
    before_ok && after_ok
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
}

fn scan_secret_value(input: &str, mut index: usize) -> usize {
    let quote = match byte_at(input, index) {
        Some(b'\"') | Some(b'\'') => {
            let quote = byte_at(input, index);
            index += 1;
            quote
        }
        _ => None,
    };
    if quote.is_none() && starts_secret_word(input, index, "bearer") {
        let token_start = skip_ascii_whitespace(input, index + "bearer".len());
        if token_start > index + "bearer".len() {
            index = token_start;
        }
    }
    while index < input.len() {
        let byte = input.as_bytes()[index];
        if quote == Some(byte)
            || (quote.is_none()
                && (byte.is_ascii_whitespace() || matches!(byte, b',' | b';' | b'&' | b'}' | b']')))
        {
            if quote == Some(byte) {
                index += 1;
            }
            break;
        }
        index += utf8_char_len(byte);
    }
    index
}

fn scan_ascii_credential(input: &str, mut index: usize) -> usize {
    while let Some(byte) = byte_at(input, index) {
        if !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
            break;
        }
        index += 1;
    }
    index
}

fn is_windows_absolute_path(input: &str, index: usize) -> bool {
    let bytes = input.as_bytes();
    index + 2 < bytes.len()
        && bytes[index].is_ascii_alphabetic()
        && bytes[index + 1] == b':'
        && matches!(bytes[index + 2], b'\\' | b'/')
        && path_boundary_before(input, index)
}

fn is_unc_path(input: &str, index: usize) -> bool {
    let bytes = input.as_bytes();
    index + 2 < bytes.len()
        && ((bytes[index] == b'\\' && bytes[index + 1] == b'\\')
            || (bytes[index] == b'/' && bytes[index + 1] == b'/'))
        && !matches!(input[..index].chars().next_back(), Some(':'))
        && path_boundary_before(input, index)
}

fn is_unix_absolute_path(input: &str, index: usize) -> bool {
    byte_at(input, index) == Some(b'/')
        && byte_at(input, index + 1)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && path_boundary_before(input, index)
}

fn is_tilde_path(input: &str, index: usize) -> bool {
    byte_at(input, index) == Some(b'~')
        && matches!(byte_at(input, index + 1), Some(b'/') | Some(b'\\'))
        && path_boundary_before(input, index)
}

fn path_boundary_before(input: &str, index: usize) -> bool {
    index == 0
        || input[..index].chars().next_back().is_some_and(|character| {
            character.is_whitespace()
                || matches!(
                    character,
                    '\"' | '\'' | '(' | '[' | '{' | '=' | ':' | ',' | ';'
                )
        })
}

fn scan_path_end(input: &str, start: usize) -> usize {
    let quote = input[..start]
        .chars()
        .next_back()
        .filter(|character| matches!(character, '\"' | '\''));
    let mut index = start;
    while index < input.len() {
        let character = input[index..].chars().next().expect("valid char boundary");
        if quote == Some(character)
            || character == '\n'
            || character == '\r'
            || matches!(character, '<' | '>' | '|' | ',' | ';' | ')' | ']' | '}')
        {
            break;
        }
        index += character.len_utf8();
    }
    index.max(start + 1)
}

fn is_url_terminator(character: char) -> bool {
    character.is_whitespace() || matches!(character, '\"' | '\'' | '<' | '>' | ')' | ']' | '}')
}

fn scan_until(input: &str, start: usize, predicate: fn(char) -> bool) -> usize {
    let mut index = start;
    while index < input.len() {
        let character = input[index..].chars().next().expect("valid char boundary");
        if predicate(character) {
            break;
        }
        index += character.len_utf8();
    }
    index
}

fn starts_ascii_case_insensitive(input: &str, index: usize, needle: &str) -> bool {
    input
        .as_bytes()
        .get(index..index.saturating_add(needle.len()))
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(needle.as_bytes()))
}

fn url_prefix_len(input: &str, index: usize) -> Option<usize> {
    ["https://", "http://", "wss://", "ws://", "ftp://"]
        .iter()
        .find(|prefix| starts_ascii_case_insensitive(input, index, prefix))
        .map(|prefix| prefix.len())
}

fn skip_ascii_whitespace(input: &str, mut index: usize) -> usize {
    while byte_at(input, index).is_some_and(|byte| byte.is_ascii_whitespace()) {
        index += 1;
    }
    index
}

fn byte_at(input: &str, index: usize) -> Option<u8> {
    input.as_bytes().get(index).copied()
}

fn utf8_char_len(first_byte: u8) -> usize {
    if first_byte < 0x80 {
        1
    } else if first_byte < 0xE0 {
        2
    } else if first_byte < 0xF0 {
        3
    } else {
        4
    }
}

fn push_next_char(input: &str, index: &mut usize, output: &mut String) {
    let character = input[*index..].chars().next().expect("valid char boundary");
    output.push(character);
    *index += character.len_utf8();
}

fn take_chars(value: &str, limit: usize) -> &str {
    match value.char_indices().nth(limit) {
        Some((index, _)) => &value[..index],
        None => value,
    }
}

fn stable_fingerprint(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn find_by_fingerprint(
    connection: &Connection,
    fingerprint: &str,
) -> Result<Option<DiagnosticRecord>, HostError> {
    connection
        .query_row(
            &format!("SELECT {DIAGNOSTIC_COLUMNS} FROM diagnostic_outbox WHERE fingerprint = ?1"),
            [fingerprint],
            record_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn find_by_id(connection: &Connection, id: &str) -> Result<Option<DiagnosticRecord>, HostError> {
    connection
        .query_row(
            &format!("SELECT {DIAGNOSTIC_COLUMNS} FROM diagnostic_outbox WHERE id = ?1"),
            [id],
            record_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn record_from_row(row: &Row<'_>) -> rusqlite::Result<DiagnosticRecord> {
    let severity: String = row.get(5)?;
    let status: String = row.get(6)?;
    let context_json: String = row.get(9)?;
    let occurrences: i64 = row.get(10)?;
    Ok(DiagnosticRecord {
        id: row.get(0)?,
        fingerprint: row.get(1)?,
        code: row.get(2)?,
        message: row.get(3)?,
        component: row.get(4)?,
        severity: DiagnosticSeverity::from_db_str(&severity).ok_or_else(|| {
            invalid_sql(format!("unknown diagnostic severity stored: {severity}"))
        })?,
        status: DiagnosticStatus::from_db_str(&status)
            .ok_or_else(|| invalid_sql(format!("unknown diagnostic status stored: {status}")))?,
        trace_id: row.get(7)?,
        project_id: row.get(8)?,
        context: serde_json::from_str(&context_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                context_json.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        occurrences: u32::try_from(occurrences).map_err(invalid_sql)?,
        first_seen_at: row.get(11)?,
        last_seen_at: row.get(12)?,
        uploaded_at: row.get(13)?,
    })
}

fn serialized_len(value: &Value) -> Result<usize, HostError> {
    serde_json::to_vec(value)
        .map(|value| value.len())
        .map_err(json_error)
}

fn not_found(id: &str) -> HostError {
    HostError::new(
        "DIAGNOSTIC_NOT_FOUND",
        format!("diagnostic {id} was not found"),
        false,
    )
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::new(
        "DIAGNOSTIC_STORAGE_FAILED",
        format!("diagnostic SQLite operation failed: {error}"),
        true,
    )
}

fn json_error(error: serde_json::Error) -> HostError {
    HostError::internal(format!("diagnostic JSON operation failed: {error}"))
}

fn invalid_sql(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outbox() -> DiagnosticOutbox {
        DiagnosticOutbox::from_connection(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn payload(message: impl Into<String>, context: Value) -> DiagnosticReportPayload {
        DiagnosticReportPayload {
            code: "PROVIDER_FAILURE".to_string(),
            message: message.into(),
            component: "model-router".to_string(),
            severity: DiagnosticSeverity::Error,
            trace_id: Some("trace-1".to_string()),
            project_id: Some("project-1".to_string()),
            context,
        }
    }

    #[test]
    fn duplicate_fingerprint_upserts_occurrences_and_requeues() {
        let outbox = outbox();
        let first = outbox
            .report(payload(
                "request failed",
                serde_json::json!({ "attempt": 1 }),
            ))
            .unwrap();
        assert_eq!(
            outbox
                .mark_uploaded(std::slice::from_ref(&first.id))
                .unwrap(),
            1
        );

        let second = outbox
            .report(payload(
                "request failed",
                serde_json::json!({ "attempt": 2 }),
            ))
            .unwrap();
        assert_eq!(second.id, first.id);
        assert_eq!(second.fingerprint, first.fingerprint);
        assert_eq!(second.occurrences, 2);
        assert_eq!(second.first_seen_at, first.first_seen_at);
        assert_eq!(second.status, DiagnosticStatus::Queued);
        assert_eq!(second.uploaded_at, None);
        assert_eq!(second.context["attempt"], 2);
    }

    #[test]
    fn redacts_paths_secrets_and_url_queries_before_storage() {
        let outbox = outbox();
        let record = outbox
            .report(payload(
                "C:\\Users\\admin\\Desktop\\secret.txt failed; Bearer top-secret-token \
                 sk-proj-1234567890 AKIA1234567890ABCD \
                 https://provider.example/v1/run?api_key=visible \
                 Authorization: Bearer header-secret \
                 OPENAI_API_KEY=provider-secret",
                serde_json::json!({
                    "cwd": "/home/operator/private/project",
                    "apiKey": "plain-context-secret",
                    "providerAccessToken": "prefixed-context-secret",
                    "nested": {
                        "profile": "C:/Users/operator/AppData/Roaming",
                        "url": "wss://example.test/file?token=leak",
                        "authorization": "Bearer context-secret"
                    }
                }),
            ))
            .unwrap();

        let stored = format!("{} {}", record.message, record.context);
        for secret in [
            "admin",
            "operator",
            "top-secret-token",
            "sk-proj-1234567890",
            "AKIA1234567890ABCD",
            "visible",
            "plain-context-secret",
            "prefixed-context-secret",
            "context-secret",
            "header-secret",
            "provider-secret",
        ] {
            assert!(
                !stored.contains(secret),
                "stored value leaked {secret}: {stored}"
            );
        }
        assert!(record.message.contains(REDACTED_PATH));
        assert!(record.message.contains("[REDACTED_QUERY]"));
        assert_eq!(record.context["apiKey"], REDACTED);
        assert_eq!(record.context["nested"]["authorization"], REDACTED);
    }

    #[test]
    fn records_survive_database_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("diagnostics.sqlite3");
        let created = {
            let outbox = DiagnosticOutbox::open(&database).unwrap();
            outbox
                .report(payload("persistent", serde_json::json!({ "ok": true })))
                .unwrap()
        };
        let reopened = DiagnosticOutbox::open(&database).unwrap();
        let queued = reopened.list_queued(10).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, created.id);
        assert_eq!(queued[0].context["ok"], true);
    }

    #[test]
    fn uploaded_and_suppressed_statuses_leave_the_queue() {
        let outbox = outbox();
        let first = outbox
            .report(payload("upload", serde_json::json!({})))
            .unwrap();
        let mut second_payload = payload("suppress", serde_json::json!({}));
        second_payload.code = "UI_FAILURE".to_string();
        let second = outbox.report(second_payload).unwrap();

        assert_eq!(
            outbox
                .mark_uploaded(&[first.id.clone(), first.id.clone()])
                .unwrap(),
            1
        );
        let uploaded = outbox.get(&first.id).unwrap();
        assert_eq!(uploaded.status, DiagnosticStatus::Uploaded);
        assert!(uploaded.uploaded_at.is_some());

        let suppressed = outbox.suppress(&second.id).unwrap();
        assert_eq!(suppressed.status, DiagnosticStatus::Suppressed);
        assert!(suppressed.uploaded_at.is_none());
        assert!(outbox.list_queued(10).unwrap().is_empty());
        assert_eq!(
            outbox.suppress("missing").unwrap_err().code,
            "DIAGNOSTIC_NOT_FOUND"
        );
    }

    #[test]
    fn oversized_input_is_bounded_without_retaining_tail_secrets() {
        let outbox = outbox();
        let huge_message = format!(
            "{} C:\\Users\\private\\tail.txt sk-should-never-survive",
            "x".repeat(MAX_MESSAGE_INPUT_CHARS * 2)
        );
        let mut deep = serde_json::json!({
            "huge": "y".repeat(MAX_CONTEXT_BYTES * 4),
            "items": (0..1_000).collect::<Vec<_>>(),
            "token": "large-secret"
        });
        for _ in 0..64 {
            deep = serde_json::json!({ "next": deep });
        }

        let record = outbox.report(payload(huge_message, deep)).unwrap();
        assert!(record.message.chars().count() <= MAX_MESSAGE_CHARS);
        assert!(!record.message.contains("private"));
        assert!(!record.message.contains("should-never-survive"));
        assert!(serde_json::to_vec(&record.context).unwrap().len() <= MAX_CONTEXT_BYTES);
        assert!(record.context.to_string().contains("truncated"));
        assert!(!record.context.to_string().contains("large-secret"));
    }

    #[test]
    fn fingerprint_is_process_stable_and_length_delimited() {
        assert_eq!(
            stable_fingerprint(&["abc", "def"]),
            stable_fingerprint(&["abc", "def"])
        );
        assert_ne!(
            stable_fingerprint(&["ab", "cdef"]),
            stable_fingerprint(&["abc", "def"])
        );
        assert_eq!(stable_fingerprint(&["a"]).len(), 64);
    }
}
