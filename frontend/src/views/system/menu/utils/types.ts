interface FormItemProps {
  id?: number;
  /** 菜单类型（0代表菜单、1代表iframe、2代表外链、3代表按钮）*/
  menuType: number;
  higherMenuOptions: Record<string, unknown>[];
  parentId: number;
  title: string;
  name: string;
  path: string;
  component: string;
  rank: number;
  redirect: string;
  icon: string;
  extraIcon: string;
  enterTransition: string;
  leaveTransition: string;
  activePath: string;
  auths: string;
  frameSrc: string;
  frameLoading: boolean;
  keepAlive: boolean;
  hiddenTag: boolean;
  fixedTag: boolean;
  showLink: boolean;
  showParent: boolean;
  children?: FormItemProps[];
}

interface FormProps {
  formInline: FormItemProps;
}

interface MenuConfigSummary {
  id: string;
  version: number;
  name: string;
  description?: string | null;
  status: "draft" | "published" | "archived" | string;
  is_active: boolean;
  created_by?: string | null;
  created_at: string;
  updated_at: string;
  published_at?: string | null;
}

interface MenuConfigDetail extends MenuConfigSummary {
  routes_json: unknown[];
}

interface MenuConfigListResponse {
  items: MenuConfigSummary[];
  total: number;
}

export type {
  FormItemProps,
  FormProps,
  MenuConfigSummary,
  MenuConfigDetail,
  MenuConfigListResponse
};
