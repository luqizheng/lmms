// Auto-generated from contracts/permissions.yaml
// DO NOT EDIT MANUALLY

/// Permission code constants
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
}

/// Check if permission code is valid
pub fn is_valid(perm: &str) -> bool {
    matches!(perm,
        "IAM_USER_CREATE"
        |
        "IAM_USER_READ"
        |
        "IAM_USER_UPDATE"
        |
        "IAM_USER_DELETE"
        |
        "IAM_USER_EXPORT"
        |
        "IAM_USER_IMPORT"
        |
        "IAM_ROLE_CREATE"
        |
        "IAM_ROLE_READ"
        |
        "IAM_ROLE_UPDATE"
        |
        "IAM_ROLE_DELETE"
        |
        "IAM_PERM_READ"
        |
        "IAM_PERM_ASSIGN"
        |
        "IAM_MENU_CREATE"
        |
        "IAM_MENU_READ"
        |
        "IAM_MENU_UPDATE"
        |
        "IAM_MENU_DELETE"
        |
        "IAM_ORG_CREATE"
        |
        "IAM_ORG_READ"
        |
        "IAM_ORG_UPDATE"
        |
        "IAM_ORG_DELETE"
        |
        "IAM_TENANT_CREATE"
        |
        "CFG_CONFIG_CREATE"
        |
        "CFG_CONFIG_READ"
        |
        "CFG_CONFIG_UPDATE"
        |
        "CFG_CONFIG_DELETE"
        |
        "CFG_CONFIG_REVEAL"
        |
        "CFG_NAMESPACE_CREATE"
        |
        "CFG_NAMESPACE_READ"
        |
        "CFG_NAMESPACE_UPDATE"
        |
        "CFG_NAMESPACE_DELETE"
        |
        "CFG_NAMESPACE_EXPORT"
        |
        "CFG_NAMESPACE_IMPORT"
        |
        "AUD_LOG_READ"
        |
        "AUD_LOG_EXPORT"
        |
        "NTF_TEMPLATE_CREATE"
        |
        "NTF_TEMPLATE_READ"
        |
        "NTF_TEMPLATE_UPDATE"
        |
        "NTF_TEMPLATE_DELETE"
        |
        "NTF_MESSAGE_READ"
        |
        "FILE_UPLOAD"
        |
        "FILE_DOWNLOAD"
        |
        "FILE_DELETE"
        |
        "FILE_READ"
        |
        "SYS_SETTING_READ"
        |
        "SYS_SETTING_UPDATE"
    )
}