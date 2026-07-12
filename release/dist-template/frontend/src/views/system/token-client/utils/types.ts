interface FormItemProps {
  client_id?: string;
  name: string;
  permissions: string[];
  grant_types: string[];
  access_token_ttl_sec: number;
  refresh_token_ttl_sec: number;
  status: string;
}

interface FormProps {
  formInline: FormItemProps;
  isEdit: boolean;
}

export type { FormItemProps, FormProps };
