import { api, inTauri } from "./api";
import { state, subscribe, update } from "./stores/session";
import { renderBrowser } from "./views/browser";
import { renderCreate } from "./views/create";
import { renderReady, renderUnlock } from "./views/unlock";
import { renderWelcome } from "./views/welcome";

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("Interface introuvable");

let autoLock = 0;

function render(): void {
  const model = state();
  if (model.screen === "welcome") renderWelcome(app!, model.discoveredPath);
  else if (model.screen === "create") renderCreate(app!, model.discoveredPath);
  else if (model.screen === "unlock") renderUnlock(app!, model);
  else if (model.screen === "ready") renderReady(app!, model.readyMessage ?? "Votre coffre est prêt.");
  else renderBrowser(app!, model);
  scheduleLock();
}

function scheduleLock(): void {
  window.clearTimeout(autoLock);
  const seconds = state().info?.autoLockSecs ?? 0;
  if (state().screen !== "browser" || seconds === 0) return;
  autoLock = window.setTimeout(() => {
    void api.lock().then(() => {
      update({
        screen: "unlock",
        info: null,
        entries: [],
        selected: [],
        discoveredPath: state().discoveredPath,
        error: null,
        busy: null,
      });
    });
  }, seconds * 1000);
}

window.addEventListener("pointerdown", () => scheduleLock());
window.addEventListener("keydown", () => scheduleLock());

subscribe(render);
render();

if (inTauri()) {
  void api.discover().then((path) => {
    if (path) update({ discoveredPath: path, screen: "unlock" });
  });
}
