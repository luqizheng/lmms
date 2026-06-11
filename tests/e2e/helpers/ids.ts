const userIds: number[] = [];
const roleIds: number[] = [];

export function trackUser(id: number) {
  userIds.push(id);
}
export function trackRole(id: number) {
  roleIds.push(id);
}
export function trackedUsers() {
  return [...userIds];
}
export function trackedRoles() {
  return [...roleIds];
}
