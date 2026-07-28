import { RolePage } from '../pages/role.page';
import { DeptPage } from '../pages/dept.page';
import { PermissionPage } from '../pages/permission.page';
import { SessionPage } from '../pages/session.page';
import { UserPage } from '../pages/user.page';
import { loginFirst } from './helpers';
import type { Flow } from './types';

export const systemFlows: Record<string, Flow> = {
  createRole: async (page, params) => {
    await loginFirst(page);
    const rp = new RolePage(page);
    const toast = await rp.createRole({
      name: String(params.name),
      code: String(params.code),
      description: params.description ? String(params.description) : undefined,
    });
    return { toast };
  },

  editRole: async (page, params) => {
    await loginFirst(page);
    const rp = new RolePage(page);
    // 幂等：先建再改
    await rp.createRole({ name: String(params.name), code: String(params.code) });
    const toast = await rp.editRole({
      name: String(params.name),
      newName: String(params.newName),
    });
    return { toast };
  },

  deleteRole: async (page, params) => {
    await loginFirst(page);
    const rp = new RolePage(page);
    // 幂等：先建再删
    await rp.createRole({ name: String(params.name), code: String(params.code) });
    const toast = await rp.deleteRole(String(params.name));
    return { toast };
  },

  assignRolePermissions: async (page, params) => {
    await loginFirst(page);
    const rp = new RolePage(page);
    await rp.createRole({ name: String(params.name), code: String(params.code) });
    const toast = await rp.assignPermissions({
      name: String(params.name),
      checkText: params.checkText ? String(params.checkText) : undefined,
    });
    return { toast };
  },

  createDept: async (page, params) => {
    await loginFirst(page);
    const dp = new DeptPage(page);
    let parentName: string | undefined;
    if (params.withParent) {
      // 自包含：先建父部门，再把目标部门挂到其下（覆盖树形创建）
      parentName = `p-${String(params.name)}`;
      await dp.createDept({ name: parentName, code: `p-${String(params.code)}` });
    }
    const toast = await dp.createDept({
      name: String(params.name),
      code: String(params.code),
      parentName,
    });
    return { toast };
  },

  editDept: async (page, params) => {
    await loginFirst(page);
    const dp = new DeptPage(page);
    await dp.createDept({ name: String(params.name), code: String(params.code) });
    const toast = await dp.editDept({
      name: String(params.name),
      newName: String(params.newName),
    });
    return { toast };
  },

  deleteDept: async (page, params) => {
    await loginFirst(page);
    const dp = new DeptPage(page);
    await dp.createDept({ name: String(params.name), code: String(params.code) });
    const toast = await dp.deleteDept(String(params.name));
    return { toast };
  },

  searchPermission: async (page, params) => {
    await loginFirst(page);
    const pp = new PermissionPage(page);
    const rowText = await pp.searchAndExpectRow(String(params.keyword));
    // 列表/搜索类无 toast，用 dialog 槽位承载命中行文本供 expect 断言
    return { dialog: rowText };
  },

  editUser: async (page, params) => {
    await loginFirst(page);
    const up = new UserPage(page);
    await up.createUser({ username: String(params.username) });
    const toast = await up.editUser({
      username: String(params.username),
      newNickname: String(params.newNickname),
    });
    return { toast };
  },

  deleteUser: async (page, params) => {
    await loginFirst(page);
    const up = new UserPage(page);
    await up.createUser({ username: String(params.username) });
    const toast = await up.deleteUser(String(params.username));
    return { toast };
  },

  assignUserRole: async (page, params) => {
    await loginFirst(page);
    const rp = new RolePage(page);
    await rp.createRole({ name: String(params.roleName), code: String(params.roleCode) });
    const up = new UserPage(page);
    await up.createUser({ username: String(params.username) });
    const toast = await up.assignRole({
      username: String(params.username),
      roleName: String(params.roleName),
    });
    return { toast };
  },

  viewSessionPage: async (page) => {
    await loginFirst(page);
    const sp = new SessionPage(page);
    await sp.goto();
    const title = await sp.waitLoaded();
    return { dialog: title };
  },
};
