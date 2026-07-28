import { LoginPage } from '../pages/login.page';
import { UserPage } from '../pages/user.page';
import { ChangePasswordPage } from '../pages/change-password.page';
import type { Flow } from './types';

export const accountFlows: Record<string, Flow> = {
  changeOwnPassword: async (page, params) => {
    const username = String(params.username);
    const oldPassword = String(params.oldPassword);
    const newPassword = String(params.newPassword);

    const login = new LoginPage(page);
    await login.login();
    await new UserPage(page).createUser({ username, password: oldPassword });

    await login.logout();

    await login.login(username, oldPassword);

    const cpp = new ChangePasswordPage(page);
    await cpp.openDialog();
    await cpp.submit(oldPassword, newPassword);

    return { toast: await cpp.lastToastText() };
  },
};
