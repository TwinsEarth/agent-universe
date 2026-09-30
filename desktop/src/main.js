// Agent Universe v2.9.0 — 桌面工作台入口
import { App } from './app.js';

import { dashboardView } from './views/dashboard.js';
import { tasksView } from './views/tasks.js';
import { agentsView } from './views/agents.js';
import { terminalView } from './views/terminal.js';
import { filesView } from './views/files.js';
import { toolsView } from './views/tools.js';
import { modelsView } from './views/models.js';
import { pluginsView } from './views/plugins.js';
import { settingsView } from './views/settings.js';

const app = new App(document.getElementById('app'));

[
  dashboardView,
  tasksView,
  agentsView,
  terminalView,
  filesView,
  toolsView,
  modelsView,
  pluginsView,
  settingsView,
].forEach((v) => app.registerView(v));

app.init();
