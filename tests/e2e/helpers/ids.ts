const userIds: number[] = [];
const roleIds: number[] = [];
const namespaceIds: string[] = [];
const configItemIds: string[] = [];
const fileIds: string[] = [];
const announcementIds: number[] = [];
const oauthClientIds: string[] = [];
const menuIds: number[] = [];
const orgUnitIds: number[] = [];

export function trackUser(id: number) { userIds.push(id); }
export function trackRole(id: number) { roleIds.push(id); }
export function trackNamespace(id: string) { namespaceIds.push(id); }
export function trackConfigItem(id: string) { configItemIds.push(id); }
export function trackFile(id: string) { fileIds.push(id); }
export function trackAnnouncement(id: number) { announcementIds.push(id); }
export function trackOAuthClient(id: string) { oauthClientIds.push(id); }
export function trackMenu(id: number) { menuIds.push(id); }
export function trackOrgUnit(id: number) { orgUnitIds.push(id); }

export function trackedUsers() { return [...userIds]; }
export function trackedRoles() { return [...roleIds]; }
export function trackedNamespaces() { return [...namespaceIds]; }
export function trackedConfigItems() { return [...configItemIds]; }
export function trackedFiles() { return [...fileIds]; }
export function trackedAnnouncements() { return [...announcementIds]; }
export function trackedOAuthClients() { return [...oauthClientIds]; }
export function trackedMenus() { return [...menuIds]; }
export function trackedOrgUnits() { return [...orgUnitIds]; }
