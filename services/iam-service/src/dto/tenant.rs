use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateTenantRequest {
    #[validate(custom(function = "validate_slug"))]
    pub tenant_slug: String,
    #[validate(length(min = 1, max = 128))]
    pub display_name: String,
    #[validate(length(min = 1, max = 50))]
    pub admin_username: String,
    #[validate(length(min = 8, max = 128))]
    pub admin_password: String,
    #[validate(email)]
    pub admin_email: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateTenantResponse {
    pub tenant_id: i64,
    pub admin_user_id: String,
}

fn validate_slug(s: &str) -> Result<(), ValidationError> {
    if s.len() < 2 || s.len() > 64 {
        return Err(ValidationError::new("slug_len"));
    }
    let first = s.chars().next().unwrap();
    let last = s.chars().last().unwrap();
    if !(first.is_ascii_lowercase() || first.is_ascii_digit())
        || !(last.is_ascii_lowercase() || last.is_ascii_digit())
    {
        return Err(ValidationError::new("slug_edge"));
    }
    if !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(ValidationError::new("slug_char"));
    }
    Ok(())
}
