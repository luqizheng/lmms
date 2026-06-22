import type { ScopeType } from "@/api/notification";

export interface FormItemProps {
  id?: number;
  title: string;
  body: string;
  publish_at: string;
  expire_at: string | null;
  pinned: boolean;
  popover: boolean;
  recipients: {
    scope_type: ScopeType;
    scope_id: string;
    name: string;
  }[];
}

export interface FormProps {
  formInline: FormItemProps;
  isEdit: boolean;
}
