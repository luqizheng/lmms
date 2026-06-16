interface FormItemProps {
  id?: string;
  /** 用于判断是`新增`还是`修改` */
  title: string;
  higherDeptOptions: Record<string, unknown>[];
  parentId: number;
  nickname: string;
  username: string;
  password: string;
  phone: string | number;
  email: string;
  sex: string | number;
  /** iam status: active / disabled */
  status: string;
  dept?: {
    id?: number;
    name?: string;
  };
  remark: string;
  avatar?: string;
}
interface FormProps {
  formInline: FormItemProps;
}

interface RoleFormItemProps {
  /** 用户 id（用于编辑场景） */
  userId?: string;
  /** 是否新增场景 */
  isCreate?: boolean;
  username: string;
  nickname: string;
  /** 角色列表 */
  roleOptions: any[];
  /** 选中的角色列表 */
  ids: string[];
}
interface RoleFormProps {
  formInline: RoleFormItemProps;
}

export type { FormItemProps, FormProps, RoleFormItemProps, RoleFormProps };
