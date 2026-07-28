import type { Flow } from './types';
import { userFlows } from './user';
import { tokenFlows } from './token';
import { accountFlows } from './account';
import { notificationFlows } from './notification';
import { systemFlows } from './system';
import { fileFlows } from './file';
import { configFlows } from './config';
import { monitorFlows } from './monitor';
import { authFlows } from './auth';
import { inboxFlows } from './inbox';
import { smokeFlows } from './smoke';

// 按域聚合：新增域 = 新建 flows/<domain>.ts 并在此加一行 spread
export const flows: Record<string, Flow> = {
  ...userFlows,
  ...tokenFlows,
  ...accountFlows,
  ...notificationFlows,
  ...systemFlows,
  ...fileFlows,
  ...configFlows,
  ...monitorFlows,
  ...authFlows,
  ...inboxFlows,
  ...smokeFlows,
};
