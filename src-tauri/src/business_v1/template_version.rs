use super::error::{require_non_empty, DomainError, DomainResult};

const SHA256_HEX_LENGTH: usize = 64;
const MAX_ACTOR_LENGTH: usize = 256;
const MAX_NOTE_LENGTH: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateArtifact {
    asset_id: String,
    sha256: String,
}

impl TemplateArtifact {
    pub fn new(asset_id: impl Into<String>, sha256: impl Into<String>) -> DomainResult<Self> {
        let artifact = Self {
            asset_id: asset_id.into(),
            sha256: sha256.into().to_ascii_lowercase(),
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn asset_id(&self) -> &str {
        &self.asset_id
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.asset_id, "template_artifact.asset_id")?;
        if self.sha256.len() != SHA256_HEX_LENGTH
            || !self.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(DomainError::InvalidValue {
                field: "template_artifact.sha256",
                reason: "must be a 64-character hexadecimal SHA-256 digest",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateConverter {
    engine: String,
    version: String,
    policy: String,
}

impl TemplateConverter {
    pub fn new(
        engine: impl Into<String>,
        version: impl Into<String>,
        policy: impl Into<String>,
    ) -> DomainResult<Self> {
        let converter = Self {
            engine: engine.into(),
            version: version.into(),
            policy: policy.into(),
        };
        converter.validate()?;
        Ok(converter)
    }

    pub fn engine(&self) -> &str {
        &self.engine
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn policy(&self) -> &str {
        &self.policy
    }

    fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.engine, "template_converter.engine")?;
        require_non_empty(&self.version, "template_converter.version")?;
        require_non_empty(&self.policy, "template_converter.policy")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateVersionStatus {
    PendingReview,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateVersionDecision {
    actor: String,
    timestamp_millis: i64,
    note: String,
}

impl TemplateVersionDecision {
    pub fn actor(&self) -> &str {
        &self.actor
    }

    pub fn timestamp_millis(&self) -> i64 {
        self.timestamp_millis
    }

    pub fn note(&self) -> &str {
        &self.note
    }

    fn new(
        actor: impl Into<String>,
        timestamp_millis: i64,
        note: impl Into<String>,
    ) -> DomainResult<Self> {
        let decision = Self {
            actor: actor.into(),
            timestamp_millis,
            note: note.into(),
        };
        decision.validate()?;
        Ok(decision)
    }

    fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.actor, "template_version.decision.actor")?;
        require_non_empty(&self.note, "template_version.decision.note")?;
        if self.actor.chars().count() > MAX_ACTOR_LENGTH {
            return Err(DomainError::InvalidValue {
                field: "template_version.decision.actor",
                reason: "must be at most 256 characters",
            });
        }
        if self.timestamp_millis <= 0 {
            return Err(DomainError::InvalidValue {
                field: "template_version.decision.timestamp_millis",
                reason: "must be positive",
            });
        }
        if self.note.chars().count() > MAX_NOTE_LENGTH {
            return Err(DomainError::InvalidValue {
                field: "template_version.decision.note",
                reason: "must be at most 2000 characters",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateVersion {
    id: String,
    source: TemplateArtifact,
    normalized: TemplateArtifact,
    template_key: String,
    mapping_version: String,
    converter: TemplateConverter,
    status: TemplateVersionStatus,
    decision: Option<TemplateVersionDecision>,
    revision: u64,
}

impl TemplateVersion {
    pub fn new(
        id: impl Into<String>,
        source: TemplateArtifact,
        normalized: TemplateArtifact,
        template_key: impl Into<String>,
        mapping_version: impl Into<String>,
        converter: TemplateConverter,
    ) -> DomainResult<Self> {
        let version = Self {
            id: id.into(),
            source,
            normalized,
            template_key: template_key.into(),
            mapping_version: mapping_version.into(),
            converter,
            status: TemplateVersionStatus::PendingReview,
            decision: None,
            revision: 1,
        };
        version.validate()?;
        Ok(version)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn source(&self) -> &TemplateArtifact {
        &self.source
    }

    pub fn normalized(&self) -> &TemplateArtifact {
        &self.normalized
    }

    pub fn template_key(&self) -> &str {
        &self.template_key
    }

    pub fn mapping_version(&self) -> &str {
        &self.mapping_version
    }

    pub fn converter(&self) -> &TemplateConverter {
        &self.converter
    }

    pub fn status(&self) -> TemplateVersionStatus {
        self.status
    }

    pub fn decision(&self) -> Option<&TemplateVersionDecision> {
        self.decision.as_ref()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn approve(
        &mut self,
        expected_revision: u64,
        actor: impl Into<String>,
        timestamp_millis: i64,
        note: impl Into<String>,
    ) -> DomainResult<()> {
        self.transition(
            expected_revision,
            TemplateVersionStatus::Approved,
            actor,
            timestamp_millis,
            note,
        )
    }

    pub fn reject(
        &mut self,
        expected_revision: u64,
        actor: impl Into<String>,
        timestamp_millis: i64,
        note: impl Into<String>,
    ) -> DomainResult<()> {
        self.transition(
            expected_revision,
            TemplateVersionStatus::Rejected,
            actor,
            timestamp_millis,
            note,
        )
    }

    fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.id, "template_version.id")?;
        require_non_empty(&self.template_key, "template_version.template_key")?;
        require_non_empty(&self.mapping_version, "template_version.mapping_version")?;
        self.source.validate()?;
        self.normalized.validate()?;
        self.converter.validate()?;
        if self.source.asset_id == self.normalized.asset_id {
            return Err(DomainError::InvalidValue {
                field: "template_version.normalized.asset_id",
                reason: "must differ from the source asset",
            });
        }
        Ok(())
    }

    fn transition(
        &mut self,
        expected_revision: u64,
        target: TemplateVersionStatus,
        actor: impl Into<String>,
        timestamp_millis: i64,
        note: impl Into<String>,
    ) -> DomainResult<()> {
        self.require_revision(expected_revision)?;
        if self.status != TemplateVersionStatus::PendingReview {
            return Err(DomainError::InvalidValue {
                field: "template_version.status",
                reason: "terminal status cannot transition",
            });
        }

        let decision = TemplateVersionDecision::new(actor, timestamp_millis, note)?;
        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or(DomainError::ArithmeticOverflow)?;
        self.status = target;
        self.decision = Some(decision);
        self.revision = next_revision;
        Ok(())
    }

    fn require_revision(&self, expected_revision: u64) -> DomainResult<()> {
        if expected_revision != self.revision {
            return Err(DomainError::MismatchedReference {
                field: "template_version.revision",
                expected: self.revision.to_string(),
                actual: expected_revision.to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_SHA: &str = "E1BF122AFDF3EF15017F3D82E9CAB5DA1C8D3BE38FEA40299906EE61538D5072";
    const NORMALIZED_SHA: &str = "CC8F0473C25EE8AFCFD0925871B440402C4AE574C25ACC74CCE99323D98F975D";

    fn template_version() -> TemplateVersion {
        TemplateVersion::new(
            "template-version-1",
            TemplateArtifact::new("source-asset", SOURCE_SHA).unwrap(),
            TemplateArtifact::new("normalized-asset", NORMALIZED_SHA).unwrap(),
            "payment-request-with-settlement",
            "mapping-v1",
            TemplateConverter::new("Microsoft Word", "16.0", "WordOnly").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn constructs_pending_version_with_frozen_metadata() {
        let version = template_version();

        assert_eq!(version.status(), TemplateVersionStatus::PendingReview);
        assert_eq!(version.revision(), 1);
        assert_eq!(version.decision(), None);
        assert_eq!(version.source().asset_id(), "source-asset");
        assert_eq!(version.source().sha256(), SOURCE_SHA.to_ascii_lowercase());
        assert_eq!(version.normalized().asset_id(), "normalized-asset");
        assert_eq!(version.template_key(), "payment-request-with-settlement");
        assert_eq!(version.mapping_version(), "mapping-v1");
        assert_eq!(version.converter().engine(), "Microsoft Word");
        assert_eq!(version.converter().version(), "16.0");
        assert_eq!(version.converter().policy(), "WordOnly");
    }

    #[test]
    fn approve_is_revision_checked_and_terminal() {
        let mut version = template_version();

        version
            .approve(1, "reviewer-1", 1_785_283_200_000, "verified")
            .unwrap();

        assert_eq!(version.status(), TemplateVersionStatus::Approved);
        assert_eq!(version.revision(), 2);
        let decision = version.decision().unwrap();
        assert_eq!(decision.actor(), "reviewer-1");
        assert_eq!(decision.timestamp_millis(), 1_785_283_200_000);
        assert_eq!(decision.note(), "verified");
        assert_eq!(
            version.reject(2, "reviewer-2", 1_785_283_200_001, "changed mind"),
            Err(DomainError::InvalidValue {
                field: "template_version.status",
                reason: "terminal status cannot transition",
            })
        );
        assert_eq!(version.status(), TemplateVersionStatus::Approved);
        assert_eq!(version.revision(), 2);
    }

    #[test]
    fn reject_is_revision_checked_and_terminal() {
        let mut version = template_version();

        version
            .reject(1, "reviewer-1", 1_785_283_200_000, "unsafe residual values")
            .unwrap();

        assert_eq!(version.status(), TemplateVersionStatus::Rejected);
        assert_eq!(version.revision(), 2);
        assert_eq!(
            version.approve(2, "reviewer-2", 1_785_283_200_001, "override"),
            Err(DomainError::InvalidValue {
                field: "template_version.status",
                reason: "terminal status cannot transition",
            })
        );
        assert_eq!(version.status(), TemplateVersionStatus::Rejected);
        assert_eq!(version.revision(), 2);
    }

    #[test]
    fn stale_revision_does_not_mutate_pending_version() {
        let mut version = template_version();

        assert_eq!(
            version.approve(0, "reviewer-1", 1_785_283_200_000, "verified"),
            Err(DomainError::MismatchedReference {
                field: "template_version.revision",
                expected: "1".to_owned(),
                actual: "0".to_owned(),
            })
        );
        assert_eq!(version.status(), TemplateVersionStatus::PendingReview);
        assert_eq!(version.revision(), 1);
        assert_eq!(version.decision(), None);
    }

    #[test]
    fn invalid_decision_does_not_mutate_pending_version() {
        let mut version = template_version();

        assert_eq!(
            version.approve(1, " ", 0, " "),
            Err(DomainError::EmptyField("template_version.decision.actor"))
        );
        assert_eq!(version.status(), TemplateVersionStatus::PendingReview);
        assert_eq!(version.revision(), 1);
        assert_eq!(version.decision(), None);

        assert_eq!(
            version.approve(1, "reviewer-1", 1_785_283_200_000, " "),
            Err(DomainError::EmptyField("template_version.decision.note"))
        );
        assert_eq!(version.status(), TemplateVersionStatus::PendingReview);
        assert_eq!(version.revision(), 1);
        assert_eq!(version.decision(), None);

        assert_eq!(
            version.reject(1, "reviewer-1", 0, "reason"),
            Err(DomainError::InvalidValue {
                field: "template_version.decision.timestamp_millis",
                reason: "must be positive",
            })
        );
        assert_eq!(version.status(), TemplateVersionStatus::PendingReview);
        assert_eq!(version.revision(), 1);
    }

    #[test]
    fn constructor_rejects_invalid_frozen_metadata() {
        assert_eq!(
            TemplateArtifact::new("asset-1", "not-a-sha"),
            Err(DomainError::InvalidValue {
                field: "template_artifact.sha256",
                reason: "must be a 64-character hexadecimal SHA-256 digest",
            })
        );

        let result = TemplateVersion::new(
            "template-version-1",
            TemplateArtifact::new("same-asset", SOURCE_SHA).unwrap(),
            TemplateArtifact::new("same-asset", NORMALIZED_SHA).unwrap(),
            "payment-request-with-settlement",
            "mapping-v1",
            TemplateConverter::new("Microsoft Word", "16.0", "WordOnly").unwrap(),
        );
        assert_eq!(
            result,
            Err(DomainError::InvalidValue {
                field: "template_version.normalized.asset_id",
                reason: "must differ from the source asset",
            })
        );
    }
}
