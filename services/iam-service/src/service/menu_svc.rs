use crate::{
    error::IamError,
    models::{Menu},
    state::AppState,
    dto::{CreateMenuRequest, UpdateMenuRequest, MenuResponse, MenuListResponse},
};
use chrono::{DateTime, Utc};
use nanoid::nanoid;

pub struct MenuService {
    state: AppState,
}

impl MenuService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn create(&self, req: CreateMenuRequest) -> Result<MenuResponse, IamError> {
        let exists = sqlx::query!(
            "SELECT id FROM menus WHERE code = ? AND deleted_at IS NULL",
            req.code
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if exists {
            return Err(IamError::DuplicateMenu);
        }

        let menu_id = nanoid!();

        let menu = sqlx::query_as!(
            Menu,
            "INSERT INTO menus (id, parent_id, name, code, path, component, icon, sort_order, visible, status, permission_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'enabled', ?, ?, ?) RETURNING *",
            menu_id,
            req.parent_id,
            req.name,
            req.code,
            req.path,
            req.component,
            req.icon,
            req.sort_order.unwrap_or(0),
            req.visible.unwrap_or(true),
            req.permission_id,
            Utc::now(),
            Utc::now()
        )
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&menu, vec![]))
    }

    pub async fn get(&self, menu_id: &str) -> Result<MenuResponse, IamError> {
        let menu = sqlx::query_as!(
            Menu,
            "SELECT * FROM menus WHERE id = ? AND deleted_at IS NULL",
            menu_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::MenuNotFound)?;

        let children = self.get_children(&menu_id).await?;

        Ok(self.to_response(&menu, children))
    }

    pub async fn list(&self) -> Result<MenuListResponse, IamError> {
        let menus = sqlx::query_as!(
            Menu,
            "SELECT * FROM menus WHERE parent_id IS NULL AND deleted_at IS NULL ORDER BY sort_order"
        )
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for menu in menus {
            let children = self.get_children(&menu.id).await?;
            result.push(self.to_response(&menu, children));
        }

        Ok(MenuListResponse {
            items: result,
            total: result.len() as u64,
        })
    }

    async fn get_children(&self, parent_id: &str) -> Result<Vec<MenuResponse>, IamError> {
        let children = sqlx::query_as!(
            Menu,
            "SELECT * FROM menus WHERE parent_id = ? AND deleted_at IS NULL ORDER BY sort_order",
            parent_id
        )
        .fetch_all(&self.state.db)
        .await?;

        let mut result = Vec::new();
        for child in children {
            let grandchildren = self.get_children(&child.id).await?;
            result.push(self.to_response(&child, grandchildren));
        }

        Ok(result)
    }

    pub async fn update(&self, menu_id: &str, req: UpdateMenuRequest) -> Result<MenuResponse, IamError> {
        let mut update_fields = vec![];
        let mut params: Vec<sqlx::types::JsonValue> = vec![];

        if let Some(parent_id) = &req.parent_id {
            update_fields.push("parent_id = ?");
            params.push(parent_id.into());
        }
        if let Some(name) = &req.name {
            update_fields.push("name = ?");
            params.push(name.into());
        }
        if let Some(code) = &req.code {
            update_fields.push("code = ?");
            params.push(code.into());
        }
        if let Some(path) = &req.path {
            update_fields.push("path = ?");
            params.push(path.into());
        }
        if let Some(component) = &req.component {
            update_fields.push("component = ?");
            params.push(component.into());
        }
        if let Some(icon) = &req.icon {
            update_fields.push("icon = ?");
            params.push(icon.into());
        }
        if let Some(sort_order) = &req.sort_order {
            update_fields.push("sort_order = ?");
            params.push(sort_order.into());
        }
        if let Some(visible) = &req.visible {
            update_fields.push("visible = ?");
            params.push(visible.into());
        }
        if let Some(permission_id) = &req.permission_id {
            update_fields.push("permission_id = ?");
            params.push(permission_id.into());
        }
        if let Some(status) = &req.status {
            update_fields.push("status = ?");
            params.push(status.into());
        }

        if update_fields.is_empty() {
            return self.get(menu_id);
        }

        update_fields.push("updated_at = ?");
        params.push(Utc::now().into());
        params.push(menu_id.into());

        let query_str = format!("UPDATE menus SET {} WHERE id = ? AND deleted_at IS NULL RETURNING *", update_fields.join(", "));

        let menu = sqlx::query_as!(Menu, &query_str, params[..])
            .fetch_optional(&self.state.db)
            .await?
            .ok_or(IamError::MenuNotFound)?;

        Ok(self.to_response(&menu, vec![]))
    }

    pub async fn delete(&self, menu_id: &str) -> Result<(), IamError> {
        let has_children = sqlx::query!(
            "SELECT id FROM menus WHERE parent_id = ? AND deleted_at IS NULL",
            menu_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .is_some();

        if has_children {
            return Err(IamError::Conflict("Cannot delete menu with children".to_string()));
        }

        let result = sqlx::query!(
            "UPDATE menus SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            Utc::now(),
            Utc::now(),
            menu_id
        )
        .execute(&self.state.db)
        .await?;

        if result.rows_affected() == 0 {
            return Err(IamError::MenuNotFound);
        }

        Ok(())
    }

    fn to_response(&self, menu: &Menu, children: Vec<MenuResponse>) -> MenuResponse {
        MenuResponse {
            id: menu.id.clone(),
            parent_id: menu.parent_id.clone(),
            name: menu.name.clone(),
            code: menu.code.clone(),
            path: menu.path.clone(),
            component: menu.component.clone(),
            icon: menu.icon.clone(),
            sort_order: menu.sort_order,
            visible: menu.visible,
            status: format!("{}", menu.status),
            permission_id: menu.permission_id.clone(),
            created_at: menu.created_at,
            updated_at: menu.updated_at,
            children,
        }
    }
}
