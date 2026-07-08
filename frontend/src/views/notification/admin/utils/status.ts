export const STATUS_LABEL_MAP: Record<
  string,
  {
    label: string;
    type: "primary" | "success" | "warning" | "info" | "danger";
  }
> = {
  draft: { label: "草稿", type: "info" },
  published: { label: "已发布", type: "success" },
  revoked: { label: "已撤销", type: "danger" }
};

export function getStatusInfo(s: string) {
  return (
    STATUS_LABEL_MAP[s] || { label: s, type: "info" as const }
  );
}
