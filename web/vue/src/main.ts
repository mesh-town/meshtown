/* Page entry point: the token from the address (exchanged for a session), the store, and the shell app mounted in `#app`. */

import { createApp } from "vue";
import { COMMON_SHELL_NO_APP } from "./common/common-texts.ts";
import { STORE_KEY, createStore } from "./core/store.ts";
import { adoptAndExchangeToken } from "./login/token-exchange.ts";
import AppShell from "./shell/AppShell.vue";
import { defaultShellDeps } from "./shell/shell-deps.ts";

async function start(root: HTMLElement): Promise<void> {
  await adoptAndExchangeToken(defaultShellDeps);
  const app = createApp(AppShell);
  app.provide(STORE_KEY, createStore());
  app.mount(root);
}

const root = document.getElementById("app");
if (!root) console.error(COMMON_SHELL_NO_APP);
else void start(root);
