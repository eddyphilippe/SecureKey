import { api } from "../api";
import { update, type AppModel } from "../stores/session";
import { refresh } from "./browser";

const WARNING =
  "Le chiffrement protège les données lorsque le coffre est verrouillé. Il ne protège pas contre un ordinateur compromis, un keylogger ou un malware lorsque vous saisissez votre mot de passe.";

export function renderUnlock(root: HTMLElement, model: AppModel): void {
  root.replaceChildren();
  const screen = document.createElement("section");
  screen.className = "gate";
  const card = document.createElement("form");
  card.className = "card";
  const mark = document.createElement("p");
  mark.className = "mark";
  mark.textContent = "SECUREKEY";
  const title = document.createElement("h1");
  title.textContent = "Coffre verrouillé";
  const path = document.createElement("p");
  path.className = "muted path";
  path.textContent = model.discoveredPath ?? "";

  const label = document.createElement("label");
  label.textContent = "Mot de passe";
  const password = document.createElement("input");
  password.type = "password";
  password.autocomplete = "current-password";
  password.required = true;
  label.append(password);

  const readOnlyLabel = document.createElement("label");
  readOnlyLabel.className = "check";
  const readOnly = document.createElement("input");
  readOnly.type = "checkbox";
  readOnlyLabel.append(readOnly, document.createTextNode(" Ouvrir en lecture seule"));

  const warning = document.createElement("p");
  warning.className = "hint";
  warning.textContent = WARNING;

  const error = document.createElement("p");
  error.className = "error";
  error.hidden = !model.error;
  error.textContent = model.error ?? "";

  const actions = document.createElement("div");
  actions.className = "row";
  const other = document.createElement("button");
  other.type = "button";
  other.className = "ghost";
  other.textContent = "Autre coffre";
  const submit = document.createElement("button");
  submit.type = "submit";
  submit.textContent = "Déverrouiller";
  actions.append(other, submit);
  card.append(mark, title, path, label, readOnlyLabel, warning, error, actions);
  screen.append(card);
  root.append(screen);

  other.addEventListener("click", async () => {
    const { pickDirectory } = await import("../api");
    const picked = await pickDirectory();
    if (picked) update({ discoveredPath: picked, error: null, screen: "unlock" });
  });
  card.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!model.discoveredPath) return;
    submit.disabled = true;
    update({ busy: "Vérification du mot de passe…", error: null });
    try {
      const info = await api.openVault(model.discoveredPath, password.value, readOnly.checked);
      password.value = "";
      update({ screen: "browser", info, busy: null, error: null, folderId: null, view: "folder", selected: [] });
      await refresh();
    } catch (caught) {
      password.value = "";
      update({ busy: null, error: String(caught), screen: "unlock" });
    }
  });
}

export function renderReady(root: HTMLElement, message: string): void {
  root.replaceChildren();
  const screen = document.createElement("section");
  screen.className = "gate";
  const card = document.createElement("div");
  card.className = "card";
  const mark = document.createElement("p");
  mark.className = "mark";
  mark.textContent = "SecureKey";
  const title = document.createElement("h1");
  title.textContent = message;
  const text = document.createElement("p");
  text.textContent = "Vous pouvez maintenant ajouter des fichiers. Ils seront chiffrés avant d'être écrits sur la clé.";
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = "Ouvrir le coffre";
  button.addEventListener("click", () => {
    update({ screen: "browser", readyMessage: null });
    void refresh();
  });
  card.append(mark, title, text, button);
  screen.append(card);
  root.append(screen);
}
