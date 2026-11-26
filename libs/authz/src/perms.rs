// Auto-generated from contracts/permissions.yaml
// DO NOT EDIT MANUALLY

/// Permission code constants
#[allow(clippy::module_inception)]
pub mod perms {
    /// 创建用户
    pub const IAM_USER_CREATE: &str = "IAM_USER_CREATE";
    /// 查看用户
    pub const IAM_USER_READ: &str = "IAM_USER_READ";
    /// 编辑用户
    pub const IAM_USER_UPDATE: &str = "IAM_USER_UPDATE";
    /// 删除用户
    pub const IAM_USER_DELETE: &str = "IAM_USER_DELETE";
    /// 导出用户
    pub const IAM_USER_EXPORT: &str = "IAM_USER_EXPORT";
    /// 导入用户
    pub const IAM_USER_IMPORT: &str = "IAM_USER_IMPORT";
    /// 创建角色
    pub const IAM_ROLE_CREATE: &str = "IAM_ROLE_CREATE";
    /// 查看角色
    pub const IAM_ROLE_READ: &str = "IAM_ROLE_READ";
    /// 编辑角色
    pub const IAM_ROLE_UPDATE: &str = "IAM_ROLE_UPDATE";
    /// 删除角色
    pub const IAM_ROLE_DELETE: &str = "IAM_ROLE_DELETE";
    /// 查看权限
    pub const IAM_PERM_READ: &str = "IAM_PERM_READ";
    /// 分配权限
    pub const IAM_PERM_ASSIGN: &str = "IAM_PERM_ASSIGN";
    /// 创建菜单
    pub const IAM_MENU_CREATE: &str = "IAM_MENU_CREATE";
    /// 查看菜单
    pub const IAM_MENU_READ: &str = "IAM_MENU_READ";
    /// 编辑菜单
    pub const IAM_MENU_UPDATE: &str = "IAM_MENU_UPDATE";
    /// 删除菜单
    pub const IAM_MENU_DELETE: &str = "IAM_MENU_DELETE";
    /// 创建部门
    pub const IAM_ORG_CREATE: &str = "IAM_ORG_CREATE";
    /// 查看部门
    pub const IAM_ORG_READ: &str = "IAM_ORG_READ";
    /// 编辑部门
    pub const IAM_ORG_UPDATE: &str = "IAM_ORG_UPDATE";
    /// 删除部门
    pub const IAM_ORG_DELETE: &str = "IAM_ORG_DELETE";
    /// 注册租户
    pub const IAM_TENANT_CREATE: &str = "IAM_TENANT_CREATE";
    /// 创建配置项
    pub const CFG_CONFIG_CREATE: &str = "CFG_CONFIG_CREATE";
    /// 查看配置项
    pub const CFG_CONFIG_READ: &str = "CFG_CONFIG_READ";
    /// 编辑配置项
    pub const CFG_CONFIG_UPDATE: &str = "CFG_CONFIG_UPDATE";
    /// 删除配置项
    pub const CFG_CONFIG_DELETE: &str = "CFG_CONFIG_DELETE";
    /// 查看敏感配置项明文
    pub const CFG_CONFIG_REVEAL: &str = "CFG_CONFIG_REVEAL";
    /// 创建配置命名空间
    pub const CFG_NAMESPACE_CREATE: &str = "CFG_NAMESPACE_CREATE";
    /// 查看配置命名空间
    pub const CFG_NAMESPACE_READ: &str = "CFG_NAMESPACE_READ";
    /// 编辑配置命名空间
    pub const CFG_NAMESPACE_UPDATE: &str = "CFG_NAMESPACE_UPDATE";
    /// 删除配置命名空间
    pub const CFG_NAMESPACE_DELETE: &str = "CFG_NAMESPACE_DELETE";
    /// 导出命名空间配置
    pub const CFG_NAMESPACE_EXPORT: &str = "CFG_NAMESPACE_EXPORT";
    /// 导入命名空间配置
    pub const CFG_NAMESPACE_IMPORT: &str = "CFG_NAMESPACE_IMPORT";
    /// 查看审计日志
    pub const AUD_LOG_READ: &str = "AUD_LOG_READ";
    /// 导出审计日志
    pub const AUD_LOG_EXPORT: &str = "AUD_LOG_EXPORT";
    /// 创建通知模板
    pub const NTF_TEMPLATE_CREATE: &str = "NTF_TEMPLATE_CREATE";
    /// 查看通知模板
    pub const NTF_TEMPLATE_READ: &str = "NTF_TEMPLATE_READ";
    /// 编辑通知模板
    pub const NTF_TEMPLATE_UPDATE: &str = "NTF_TEMPLATE_UPDATE";
    /// 删除通知模板
    pub const NTF_TEMPLATE_DELETE: &str = "NTF_TEMPLATE_DELETE";
    /// 查看站内信
    pub const NTF_MESSAGE_READ: &str = "NTF_MESSAGE_READ";
    /// 上传文件
    pub const FILE_UPLOAD: &str = "FILE_UPLOAD";
    /// 下载文件
    pub const FILE_DOWNLOAD: &str = "FILE_DOWNLOAD";
    /// 删除文件
    pub const FILE_DELETE: &str = "FILE_DELETE";
    /// 查看文件列表与详情
    pub const FILE_READ: &str = "FILE_READ";
    /// 查看系统设置
    pub const SYS_SETTING_READ: &str = "SYS_SETTING_READ";
    /// 编辑系统设置
    pub const SYS_SETTING_UPDATE: &str = "SYS_SETTING_UPDATE";
    /// 查看 bootstrap token 元数据（不含明文）
    pub const TOKEN_BOOTSTRAP_READ: &str = "TOKEN_BOOTSTRAP_READ";
    /// 创建 bootstrap token
    pub const TOKEN_BOOTSTRAP_CREATE: &str = "TOKEN_BOOTSTRAP_CREATE";
    /// 轮换 / 启停 bootstrap token
    pub const TOKEN_BOOTSTRAP_UPDATE: &str = "TOKEN_BOOTSTRAP_UPDATE";
    /// 删除 bootstrap token
    pub const TOKEN_BOOTSTRAP_DELETE: &str = "TOKEN_BOOTSTRAP_DELETE";
    /// 查看 Service JWT audience 授权关系
    pub const TOKEN_SERVICE_JWT_GRANT_READ: &str = "TOKEN_SERVICE_JWT_GRANT_READ";
    /// 创建 Service JWT audience 授权关系
    pub const TOKEN_SERVICE_JWT_GRANT_CREATE: &str = "TOKEN_SERVICE_JWT_GRANT_CREATE";
    /// 启用 / 禁用 Service JWT audience 授权关系
    pub const TOKEN_SERVICE_JWT_GRANT_UPDATE: &str = "TOKEN_SERVICE_JWT_GRANT_UPDATE";
    /// 删除 Service JWT audience 授权关系
    pub const TOKEN_SERVICE_JWT_GRANT_DELETE: &str = "TOKEN_SERVICE_JWT_GRANT_DELETE";
    /// 创建项目
    pub const PM_PROJECT_CREATE: &str = "PM_PROJECT_CREATE";
    /// 查看项目
    pub const PM_PROJECT_READ: &str = "PM_PROJECT_READ";
    /// 编辑项目
    pub const PM_PROJECT_UPDATE: &str = "PM_PROJECT_UPDATE";
    /// 归档项目（软删）
    pub const PM_PROJECT_DELETE: &str = "PM_PROJECT_DELETE";
    /// 创建里程碑
    pub const PM_MILESTONE_CREATE: &str = "PM_MILESTONE_CREATE";
    /// 查看里程碑
    pub const PM_MILESTONE_READ: &str = "PM_MILESTONE_READ";
    /// 编辑里程碑
    pub const PM_MILESTONE_UPDATE: &str = "PM_MILESTONE_UPDATE";
    /// 删除里程碑
    pub const PM_MILESTONE_DELETE: &str = "PM_MILESTONE_DELETE";
    /// 编辑任务（含状态机转移）
    pub const PM_TASK_UPDATE: &str = "PM_TASK_UPDATE";
    /// 调用 LLM 细化需求（占位 provider）
    pub const PM_LLM_ASSIST: &str = "PM_LLM_ASSIST";
    /// 通过 MCP 读取任务/需求/文档
    pub const PM_MCP_READ: &str = "PM_MCP_READ";
    /// 通过 MCP 推送文档并建立任务/需求关联
    pub const PM_MCP_DOCUMENT_WRITE: &str = "PM_MCP_DOCUMENT_WRITE";
    /// 通过 MCP 更新任务状态与工作日志
    pub const PM_MCP_TASK_UPDATE: &str = "PM_MCP_TASK_UPDATE";
}

/// Check if permission code is valid
pub fn is_valid(perm: &str) -> bool {
    matches!(
        perm,
        "IAM_USER_CREATE"
            | "IAM_USER_READ"
            | "IAM_USER_UPDATE"
            | "IAM_USER_DELETE"
            | "IAM_USER_EXPORT"
            | "IAM_USER_IMPORT"
            | "IAM_ROLE_CREATE"
            | "IAM_ROLE_READ"
            | "IAM_ROLE_UPDATE"
            | "IAM_ROLE_DELETE"
            | "IAM_PERM_READ"
            | "IAM_PERM_ASSIGN"
            | "IAM_MENU_CREATE"
            | "IAM_MENU_READ"
            | "IAM_MENU_UPDATE"
            | "IAM_MENU_DELETE"
            | "IAM_ORG_CREATE"
            | "IAM_ORG_READ"
            | "IAM_ORG_UPDATE"
            | "IAM_ORG_DELETE"
            | "IAM_TENANT_CREATE"
            | "CFG_CONFIG_CREATE"
            | "CFG_CONFIG_READ"
            | "CFG_CONFIG_UPDATE"
            | "CFG_CONFIG_DELETE"
            | "CFG_CONFIG_REVEAL"
            | "CFG_NAMESPACE_CREATE"
            | "CFG_NAMESPACE_READ"
            | "CFG_NAMESPACE_UPDATE"
            | "CFG_NAMESPACE_DELETE"
            | "CFG_NAMESPACE_EXPORT"
            | "CFG_NAMESPACE_IMPORT"
            | "AUD_LOG_READ"
            | "AUD_LOG_EXPORT"
            | "NTF_TEMPLATE_CREATE"
            | "NTF_TEMPLATE_READ"
            | "NTF_TEMPLATE_UPDATE"
            | "NTF_TEMPLATE_DELETE"
            | "NTF_MESSAGE_READ"
            | "FILE_UPLOAD"
            | "FILE_DOWNLOAD"
            | "FILE_DELETE"
            | "FILE_READ"
            | "SYS_SETTING_READ"
            | "SYS_SETTING_UPDATE"
            | "TOKEN_BOOTSTRAP_READ"
            | "TOKEN_BOOTSTRAP_CREATE"
            | "TOKEN_BOOTSTRAP_UPDATE"
            | "TOKEN_BOOTSTRAP_DELETE"
            | "TOKEN_SERVICE_JWT_GRANT_READ"
            | "TOKEN_SERVICE_JWT_GRANT_CREATE"
            | "TOKEN_SERVICE_JWT_GRANT_UPDATE"
            | "TOKEN_SERVICE_JWT_GRANT_DELETE"
            | "PM_PROJECT_CREATE"
            | "PM_PROJECT_READ"
            | "PM_PROJECT_UPDATE"
            | "PM_PROJECT_DELETE"
            | "PM_MILESTONE_CREATE"
            | "PM_MILESTONE_READ"
            | "PM_MILESTONE_UPDATE"
            | "PM_MILESTONE_DELETE"
            | "PM_TASK_UPDATE"
            | "PM_LLM_ASSIST"
            | "PM_MCP_READ"
            | "PM_MCP_DOCUMENT_WRITE"
            | "PM_MCP_TASK_UPDATE"
    )
}
