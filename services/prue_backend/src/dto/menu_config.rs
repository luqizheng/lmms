use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use utoipa::ToSchema;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            message: "ok".to_string(),
            data,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct CreateMenuConfigRequest {
    pub name: String,
    pub description: Option<String>,
    pub routes_json: Value,
    pub created_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct UpdateMenuConfigRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub routes_json: Option<Value>,
    pub if_match_etag: Option<String>,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct MenuConfigSummary {
    pub id: String,
    pub version: i64,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub is_active: bool,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub routes_etag: String,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct MenuConfigDetail {
    pub id: String,
    pub version: i64,
    pub name: String,
    pub description: Option<String>,
    pub routes_json: Value,
    pub status: String,
    pub is_active: bool,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub routes_etag: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MenuConfigListResponse {
    pub items: Vec<MenuConfigSummary>,
    pub total: u64,
}

pub fn validate_routes_json(value: &Value) -> Result<(), String> {
    let routes = value.as_array().ok_or("routes_json must be an array")?;
    let mut paths = HashSet::new();
    let mut names = HashSet::new();

    for route in routes {
        validate_route(route, &mut paths, &mut names)?;
    }

    Ok(())
}

fn validate_route(
    route: &Value,
    paths: &mut HashSet<String>,
    names: &mut HashSet<String>,
) -> Result<(), String> {
    let obj = route.as_object().ok_or("route must be an object")?;

    let path = obj
        .get("path")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or("route.path is required")?;
    if !paths.insert(path.to_string()) {
        return Err(format!("duplicate route.path: {path}"));
    }

    if let Some(name) = obj
        .get("name")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        if !names.insert(name.to_string()) {
            return Err(format!("duplicate route.name: {name}"));
        }
    }

    if let Some(meta) = obj.get("meta") {
        let meta_obj = meta.as_object().ok_or("route.meta must be an object")?;
        validate_string_array(meta_obj.get("roles"), "route.meta.roles")?;
        validate_string_array(meta_obj.get("auths"), "route.meta.auths")?;
    }

    if let Some(children) = obj.get("children") {
        let child_routes = children.as_array().ok_or("route.children must be an array")?;
        for child in child_routes {
            validate_route(child, paths, names)?;
        }
    }

    Ok(())
}

fn validate_string_array(value: Option<&Value>, field: &str) -> Result<(), String> {
    if let Some(value) = value {
        let arr = value
            .as_array()
            .ok_or_else(|| format!("{field} must be an array"))?;
        if arr.iter().any(|item| item.as_str().is_none()) {
            return Err(format!("{field} must contain strings only"));
        }
    }
    Ok(())
}
