use crate::{
    error::IamError,
    models::{Menu},
    state::AppState,
    dto::{CreateMenuRequest, UpdateMenuRequest, MenuResponse, MenuListResponse},
};
use chrono::Utc;
use nanoid::nanoid;
use sqlx::mysql::MySqlQueryResult;

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
        // Check if menu exists
        let existing = sqlx::query_as!(
            Menu,
            "SELECT * FROM menus WHERE id = ? AND deleted_at IS NULL",
            menu_id
        )
        .fetch_optional(&self.state.db)
        .await?
        .ok_or(IamError::MenuNotFound)?;

        let parent_id = req.parent_id.or(existing.parent_id);
        let name = req.name.unwrap_or(existing.name);
        let code = req.code.unwrap_or(existing.code);
        let path = req.path.unwrap_or(existing.path);
        let component = req.component.or(existing.component);
        let icon = req.icon.or(existing.icon);
        let sort_order = req.sort_order.unwrap_or(existing.sort_order);
        let visible = req.visible.unwrap_or(existing.visible);
        let permission_id = req.permission_id.or(existing.permission_id);
        let status = req.status.unwrap_or_else(|| format!("{}", existing.status));

        let menu: Menu = sqlx::query_as(
            "UPDATE menus SET parent_id = ?, name = ?, code = ?, path = ?, component = ?, icon = ?, sort_order = ?, visible = ?, permission_id = ?, status = ?, updated_at = NOW() WHERE id = ? AND deleted_at IS NULL RETURNING *",
        )
        .bind(&parent_id)
        .bind(&name)
        .bind(&code)
        .bind(&path)
        .bind(&component)
        .bind(&icon)
        .bind(sort_order)
        .bind(visible)
        .bind(&permission_id)
        .bind(&status)
        .bind(menu_id)
        .fetch_one(&self.state.db)
        .await?;

        Ok(self.to_response(&menu, vec![]))
    }

    pub async fn delete(&self, menu_id: &str) -> Result<(), IamError> {
        let has_children: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM menus WHERE parent_id = ? AND deleted_at IS NULL",
        )
        .bind(menu_id)
        .fetch_one(&self.state.db)
        .await?;

        if has_children.0 > 0 {
            return Err(IamError::Conflict("Cannot delete menu with children".to_string()));
        }

        let result: MySqlQueryResult = sqlx::query(
            "UPDATE menus SET deleted_at = NOW(), updated_at = NOW() WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(menu_id)
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
