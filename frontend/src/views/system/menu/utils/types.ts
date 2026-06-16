// 菜单表单字段：对接 iam-service 的 IamMenu 结构
interface FormItemProps {
  /** 菜单类型（保留 UI 选择，不发送给后端）：0 菜单、1 iframe、2 外链、3 按钮 */
  menuType: number;
  /** 上级菜单级联选项（树） */
  higherMenuOptions: Record<string, unknown>[];
  /** 主键（编辑时存在） */
  id?: string;
  /** 父级菜单 id；空字符串表示无父级 */
  parentId: string;
  /** 菜单名称 */
  name: string;
  /** 菜单编码 */
  code: string;
  /** 路由路径 */
  path: string;
  /** 组件路径 */
  component: string;
  /** 排序值 */
  sortOrder: number;
  /** 图标 */
  icon: string;
  /** 是否可见 */
  visible: boolean;
  /** 关联权限 id */
  permissionId: string;
  /** 状态（仅编辑时有效） */
  status: string;
}
interface FormProps {
  formInline: FormItemProps;
}

export type { FormItemProps, FormProps };
