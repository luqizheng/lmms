// 部门表单字段：对接 iam-service IamOrgUnit
interface FormItemProps {
  /** 上级组织单元级联选项（树） */
  higherDeptOptions: Record<string, unknown>[];
  /** 主键（编辑时存在） */
  id?: string;
  /** 父级 id；空字符串表示无父级 */
  parentId: string;
  /** 名称 */
  name: string;
  /** 编码 */
  code: string;
  /** 排序 */
  sortOrder: number;
  /** 状态（仅编辑时有效） */
  status: string;
}
interface FormProps {
  formInline: FormItemProps;
}

export type { FormItemProps, FormProps };
