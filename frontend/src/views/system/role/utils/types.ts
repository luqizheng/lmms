// 虽然字段很少 但是抽离出来 后续有扩展字段需求就很方便了

interface FormItemProps {
  /** 是否新增（用于 form 中区分是否显示 code 等） */
  isCreate?: boolean;
  /** 角色名称 */
  name: string;
  /** 角色编号 */
  code: string;
  /** 描述 */
  description: string;
  /** 数据范围 0-self 1-dept 2-dept_and_below 3-all */
  data_scope: number;
  /** 状态 active / disabled */
  status: string;
}
interface FormProps {
  formInline: FormItemProps;
}

export type { FormItemProps, FormProps };
