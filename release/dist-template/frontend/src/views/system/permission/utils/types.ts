interface PermissionFormItem {
  id?: string;
  code: string;
  name: string;
  description?: string;
  category: number;
  resource?: string;
  action?: string;
  sort_order?: number;
  status?: string;
}

interface PermissionFormProps {
  formInline: PermissionFormItem;
  isEdit: boolean;
}

export type { PermissionFormItem, PermissionFormProps };
