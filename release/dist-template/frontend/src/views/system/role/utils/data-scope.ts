export const DATA_SCOPE_LABEL: Record<string, string> = {
  "0": "仅本人",
  "1": "本部门",
  "2": "本部门及以下",
  "3": "全部",
  self: "仅本人",
  dept: "本部门",
  dept_and_below: "本部门及以下",
  all: "全部"
};

export function dataScopeText(v: string | number | undefined) {
  if (v === undefined || v === null) return "-";
  return DATA_SCOPE_LABEL[String(v)] ?? String(v);
}

export function dataScopeToNumber(v: string | number | undefined): number {
  if (typeof v === "number") return v;
  if (v === undefined || v === null || v === "") return 0;
  const n = Number(v);
  if (!Number.isNaN(n)) return n;
  switch (v) {
    case "self":
      return 0;
    case "dept":
      return 1;
    case "dept_and_below":
      return 2;
    case "all":
      return 3;
    default:
      return 0;
  }
}
