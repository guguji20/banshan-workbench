use crate::protocol::{
    ApprovalRecord, ApprovalStatus, HostError, PermissionDecision, ResolveApprovalPayload,
};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const APPROVAL_TTL_MS: i64 = 15 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationEffect {
    Read,
    ReversibleWrite,
    Irreversible,
}

pub fn migrate(connection: &Connection) -> Result<(), HostError> {
    connection
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY NOT NULL,
                operation TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT,
                actor_id TEXT NOT NULL,
                status TEXT NOT NULL,
                reason TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                resolved_at INTEGER,
                resolved_by TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_approvals_pending
                ON approvals(actor_id, operation, resource_type, resource_id, status, expires_at);
            "#,
        )
        .map_err(sql_error)
}

pub fn authorize(
    connection: &Connection,
    actor_id: &str,
    operation: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    effect: OperationEffect,
    approval_id: Option<&str>,
) -> Result<PermissionDecision, HostError> {
    validate_identity(actor_id, operation, resource_type)?;
    if effect != OperationEffect::Irreversible {
        return Ok(PermissionDecision {
            allowed: true,
            approval_required: false,
            approval_id: None,
            reason: None,
        });
    }

    let now = now_millis();
    expire_pending(connection, now)?;
    if let Some(approval_id) = approval_id {
        let approval = get(connection, approval_id)?;
        let matches_operation = approval.actor_id == actor_id
            && approval.operation == operation
            && approval.resource_type == resource_type
            && approval.resource_id.as_deref() == resource_id;
        if matches_operation
            && approval.status == ApprovalStatus::Approved
            && approval.expires_at >= now
            && consume_approved(connection, &approval.id, now)?
        {
            return Ok(PermissionDecision {
                allowed: true,
                approval_required: false,
                approval_id: Some(approval.id),
                reason: None,
            });
        }
        return Ok(PermissionDecision {
            allowed: false,
            approval_required: true,
            approval_id: Some(approval.id),
            reason: Some("approval is not valid for this operation".to_string()),
        });
    }

    if let Some(approval) = find_approved(
        connection,
        actor_id,
        operation,
        resource_type,
        resource_id,
        now,
    )? {
        if consume_approved(connection, &approval.id, now)? {
            return Ok(PermissionDecision {
                allowed: true,
                approval_required: false,
                approval_id: Some(approval.id),
                reason: None,
            });
        }
    }

    if let Some(existing) = find_pending(
        connection,
        actor_id,
        operation,
        resource_type,
        resource_id,
        now,
    )? {
        return Ok(PermissionDecision {
            allowed: false,
            approval_required: true,
            approval_id: Some(existing.id),
            reason: Some(existing.reason),
        });
    }

    let approval = ApprovalRecord {
        id: Uuid::new_v4().to_string(),
        operation: operation.to_string(),
        resource_type: resource_type.to_string(),
        resource_id: resource_id.map(str::to_string),
        actor_id: actor_id.to_string(),
        status: ApprovalStatus::Pending,
        reason: "irreversible operation requires explicit approval".to_string(),
        created_at: now,
        expires_at: now + APPROVAL_TTL_MS,
        resolved_at: None,
        resolved_by: None,
    };
    insert(connection, &approval)?;
    Ok(PermissionDecision {
        allowed: false,
        approval_required: true,
        approval_id: Some(approval.id),
        reason: Some(approval.reason),
    })
}

pub fn resolve(
    connection: &Connection,
    resolver_id: &str,
    payload: &ResolveApprovalPayload,
) -> Result<ApprovalRecord, HostError> {
    if resolver_id.trim().is_empty() {
        return Err(HostError::validation("resolverId is required"));
    }
    let current = get(connection, &payload.approval_id)?;
    if current.status != ApprovalStatus::Pending {
        return Err(HostError::conflict("approval has already been resolved"));
    }
    let now = now_millis();
    if current.expires_at < now {
        expire_pending(connection, now)?;
        return Err(HostError::new(
            "APPROVAL_EXPIRED",
            "approval has expired",
            false,
        ));
    }
    let status = if payload.approved {
        ApprovalStatus::Approved
    } else {
        ApprovalStatus::Denied
    };
    let changed = connection
        .execute(
            "UPDATE approvals SET status = ?2, resolved_at = ?3, resolved_by = ?4
             WHERE id = ?1 AND status = 'pending'",
            params![payload.approval_id, status.as_db_str(), now, resolver_id],
        )
        .map_err(sql_error)?;
    if changed != 1 {
        return Err(HostError::conflict(
            "approval changed while it was being resolved",
        ));
    }
    get(connection, &payload.approval_id)
}

pub fn list_pending(connection: &Connection) -> Result<Vec<ApprovalRecord>, HostError> {
    expire_pending(connection, now_millis())?;
    let mut statement = connection
        .prepare(
            "SELECT id, operation, resource_type, resource_id, actor_id, status, reason,
                    created_at, expires_at, resolved_at, resolved_by
             FROM approvals WHERE status = 'pending' ORDER BY created_at ASC",
        )
        .map_err(sql_error)?;
    let approvals = statement
        .query_map([], approval_from_row)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(approvals)
}

fn validate_identity(
    actor_id: &str,
    operation: &str,
    resource_type: &str,
) -> Result<(), HostError> {
    if actor_id.trim().is_empty() || operation.trim().is_empty() || resource_type.trim().is_empty()
    {
        return Err(HostError::validation(
            "actorId, operation and resourceType are required",
        ));
    }
    Ok(())
}

fn insert(connection: &Connection, approval: &ApprovalRecord) -> Result<(), HostError> {
    connection
        .execute(
            "INSERT INTO approvals
             (id, operation, resource_type, resource_id, actor_id, status, reason,
              created_at, expires_at, resolved_at, resolved_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                approval.id,
                approval.operation,
                approval.resource_type,
                approval.resource_id,
                approval.actor_id,
                approval.status.as_db_str(),
                approval.reason,
                approval.created_at,
                approval.expires_at,
                approval.resolved_at,
                approval.resolved_by,
            ],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn get(connection: &Connection, approval_id: &str) -> Result<ApprovalRecord, HostError> {
    connection
        .query_row(
            "SELECT id, operation, resource_type, resource_id, actor_id, status, reason,
                    created_at, expires_at, resolved_at, resolved_by
             FROM approvals WHERE id = ?1",
            [approval_id],
            approval_from_row,
        )
        .optional()
        .map_err(sql_error)?
        .ok_or_else(|| HostError::new("APPROVAL_NOT_FOUND", "approval does not exist", false))
}

fn find_pending(
    connection: &Connection,
    actor_id: &str,
    operation: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    now: i64,
) -> Result<Option<ApprovalRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, operation, resource_type, resource_id, actor_id, status, reason,
                    created_at, expires_at, resolved_at, resolved_by
             FROM approvals
             WHERE actor_id = ?1 AND operation = ?2 AND resource_type = ?3
               AND resource_id IS ?4 AND status = 'pending' AND expires_at >= ?5
             ORDER BY created_at DESC LIMIT 1",
            params![actor_id, operation, resource_type, resource_id, now],
            approval_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn find_approved(
    connection: &Connection,
    actor_id: &str,
    operation: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    now: i64,
) -> Result<Option<ApprovalRecord>, HostError> {
    connection
        .query_row(
            "SELECT id, operation, resource_type, resource_id, actor_id, status, reason,
                    created_at, expires_at, resolved_at, resolved_by
             FROM approvals
             WHERE actor_id = ?1 AND operation = ?2 AND resource_type = ?3
               AND resource_id IS ?4 AND status = 'approved' AND expires_at >= ?5
             ORDER BY resolved_at DESC, created_at DESC LIMIT 1",
            params![actor_id, operation, resource_type, resource_id, now],
            approval_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn consume_approved(
    connection: &Connection,
    approval_id: &str,
    now: i64,
) -> Result<bool, HostError> {
    connection
        .execute(
            "UPDATE approvals SET status = 'consumed'
             WHERE id = ?1 AND status = 'approved' AND expires_at >= ?2",
            params![approval_id, now],
        )
        .map(|changed| changed == 1)
        .map_err(sql_error)
}

fn expire_pending(connection: &Connection, now: i64) -> Result<(), HostError> {
    connection
        .execute(
            "UPDATE approvals SET status = 'expired', resolved_at = ?1
             WHERE status = 'pending' AND expires_at < ?1",
            [now],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn approval_from_row(row: &Row<'_>) -> rusqlite::Result<ApprovalRecord> {
    let raw_status: String = row.get(5)?;
    let status = ApprovalStatus::from_db_str(&raw_status).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            raw_status.len(),
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown approval status: {raw_status}"),
            )),
        )
    })?;
    Ok(ApprovalRecord {
        id: row.get(0)?,
        operation: row.get(1)?,
        resource_type: row.get(2)?,
        resource_id: row.get(3)?,
        actor_id: row.get(4)?,
        status,
        reason: row.get(6)?,
        created_at: row.get(7)?,
        expires_at: row.get(8)?,
        resolved_at: row.get(9)?,
        resolved_by: row.get(10)?,
    })
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn sql_error(error: rusqlite::Error) -> HostError {
    HostError::internal(format!("approval SQLite operation failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        connection
    }

    #[test]
    fn reversible_operations_do_not_create_approval() {
        let connection = database();
        let decision = authorize(
            &connection,
            "operator",
            "project.updateBrief",
            "project",
            Some("p1"),
            OperationEffect::ReversibleWrite,
            None,
        )
        .unwrap();
        assert!(decision.allowed);
        assert!(list_pending(&connection).unwrap().is_empty());
    }

    #[test]
    fn irreversible_operation_requires_matching_approval() {
        let connection = database();
        let first = authorize(
            &connection,
            "operator",
            "asset.delete",
            "asset",
            Some("a1"),
            OperationEffect::Irreversible,
            None,
        )
        .unwrap();
        assert!(!first.allowed);
        let approval_id = first.approval_id.unwrap();

        resolve(
            &connection,
            "reviewer",
            &ResolveApprovalPayload {
                approval_id: approval_id.clone(),
                approved: true,
            },
        )
        .unwrap();
        let allowed = authorize(
            &connection,
            "operator",
            "asset.delete",
            "asset",
            Some("a1"),
            OperationEffect::Irreversible,
            Some(&approval_id),
        )
        .unwrap();
        assert!(allowed.allowed);

        let wrong_resource = authorize(
            &connection,
            "operator",
            "asset.delete",
            "asset",
            Some("a2"),
            OperationEffect::Irreversible,
            Some(&approval_id),
        )
        .unwrap();
        assert!(!wrong_resource.allowed);
    }

    #[test]
    fn repeated_request_reuses_pending_approval() {
        let connection = database();
        let one = authorize(
            &connection,
            "operator",
            "task.retryPaid",
            "task",
            Some("t1"),
            OperationEffect::Irreversible,
            None,
        )
        .unwrap();
        let two = authorize(
            &connection,
            "operator",
            "task.retryPaid",
            "task",
            Some("t1"),
            OperationEffect::Irreversible,
            None,
        )
        .unwrap();
        assert_eq!(one.approval_id, two.approval_id);
        assert_eq!(list_pending(&connection).unwrap().len(), 1);
    }

    #[test]
    fn resolved_approval_is_consumed_implicitly_and_cannot_be_reused() {
        let connection = database();
        let pending = authorize(
            &connection,
            "operator",
            "businessWorkspace.approve",
            "businessWorkspace",
            Some("workspace-1"),
            OperationEffect::Irreversible,
            None,
        )
        .unwrap();
        let approval_id = pending.approval_id.unwrap();
        resolve(
            &connection,
            "operator",
            &ResolveApprovalPayload {
                approval_id: approval_id.clone(),
                approved: true,
            },
        )
        .unwrap();

        let allowed = authorize(
            &connection,
            "operator",
            "businessWorkspace.approve",
            "businessWorkspace",
            Some("workspace-1"),
            OperationEffect::Irreversible,
            None,
        )
        .unwrap();
        assert!(allowed.allowed);
        assert_eq!(allowed.approval_id.as_deref(), Some(approval_id.as_str()));
        assert_eq!(
            get(&connection, &approval_id).unwrap().status,
            ApprovalStatus::Consumed
        );

        let second = authorize(
            &connection,
            "operator",
            "businessWorkspace.approve",
            "businessWorkspace",
            Some("workspace-1"),
            OperationEffect::Irreversible,
            None,
        )
        .unwrap();
        assert!(!second.allowed);
        assert_ne!(second.approval_id.as_deref(), Some(approval_id.as_str()));
    }
}
