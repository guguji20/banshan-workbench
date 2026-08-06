use super::error::{require_non_empty, DomainError, DomainResult};
use super::BasisPoints;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectStatus {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Company {
    pub id: String,
    pub legal_name: String,
    pub short_name: String,
    pub unified_social_credit_code: String,
    pub default_tax_rate_basis_points: BasisPoints,
    pub active: bool,
}

impl Company {
    pub fn new(
        id: impl Into<String>,
        legal_name: impl Into<String>,
        short_name: impl Into<String>,
        unified_social_credit_code: impl Into<String>,
        default_tax_rate_basis_points: BasisPoints,
    ) -> DomainResult<Self> {
        let company = Self {
            id: id.into(),
            legal_name: legal_name.into(),
            short_name: short_name.into(),
            unified_social_credit_code: unified_social_credit_code.into(),
            default_tax_rate_basis_points,
            active: true,
        };
        company.validate()?;
        Ok(company)
    }

    pub fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.id, "company.id")?;
        require_non_empty(&self.legal_name, "company.legal_name")?;
        require_non_empty(&self.short_name, "company.short_name")?;
        require_non_empty(
            &self.unified_social_credit_code,
            "company.unified_social_credit_code",
        )?;
        if self.default_tax_rate_basis_points > 10_000 {
            return Err(DomainError::InvalidValue {
                field: "company.default_tax_rate_basis_points",
                reason: "must be at most 10000",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerProject {
    pub id: String,
    pub customer_group_id: String,
    pub name: String,
    pub customer_legal_name: String,
    pub owner_company_id: String,
    pub status: ProjectStatus,
}

impl CustomerProject {
    pub fn new(
        id: impl Into<String>,
        customer_group_id: impl Into<String>,
        name: impl Into<String>,
        customer_legal_name: impl Into<String>,
        owner_company_id: impl Into<String>,
    ) -> DomainResult<Self> {
        let project = Self {
            id: id.into(),
            customer_group_id: customer_group_id.into(),
            name: name.into(),
            customer_legal_name: customer_legal_name.into(),
            owner_company_id: owner_company_id.into(),
            status: ProjectStatus::Active,
        };
        project.validate()?;
        Ok(project)
    }

    pub fn validate(&self) -> DomainResult<()> {
        require_non_empty(&self.id, "customer_project.id")?;
        require_non_empty(
            &self.customer_group_id,
            "customer_project.customer_group_id",
        )?;
        require_non_empty(&self.name, "customer_project.name")?;
        require_non_empty(
            &self.customer_legal_name,
            "customer_project.customer_legal_name",
        )?;
        require_non_empty(&self.owner_company_id, "customer_project.owner_company_id")?;
        Ok(())
    }
}
