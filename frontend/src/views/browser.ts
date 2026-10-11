import { getCurrentWebview } from "@tauri-apps/api/webview";

import { api, inTauri, onProgress, pickDirectory, pickFiles } from "../api";
import { ask, confirmBox } from "../components/dialogs";
import { state, update, type AppModel } from "../stores/session";
import type { Entry, ViewId } from "../types";

const VIEWS: { id: ViewId; label: string }[] = [
  { id: "folder", label: "Tous les fichiers" },
  { id: "documents", label: "Documents" },
  { id: "images", label: "Images" },
  { id: "favorites", label: "Favoris" },
  { id: "trash", label: "Corbeille" },
];

let dragInstalled = false;
let poll = 0;
const dismissedTemps = new Set<string>();

export async function refresh(): Promise<void> {
  const current = state();
  if (current.screen !== "browser") return;
  const folderId = current.view === "folder" ? current.folderId : null;
  const [entries, crumbs] = await Promise.all([
    api.list(current.view, folderId),
    current.view === "folder" ? api.breadcrumbs(current.folderId) : Promise.resolve([]),
  ]);
  update({ entries, crumbs });
}

export function renderBrowser(root: HTMLElement, model: AppModel): void {
  root.replaceChildren();
  const shell = document.createElement("div");
  shell.className = "shell";
  shell.append(sidebar(model), mainPane(model));
  root.append(shell);
  if (model.busy) root.append(progressOverlay(model));
  installDrag();
  armTempPoll();
}

function sidebar(model: AppModel): HTMLElement {
  const aside = document.createElement("aside");
  const mark = document.createElement("p");
  mark.className = "mark light";
  mark.textContent = "SecureKey";
  const name = document.createElement("p");
  name.className = "vault-name";
  name.textContent = model.info?.vaultName ?? "Coffre";
  aside.append(mark, name);
  for (const view of VIEWS) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = model.view === view.id ? "nav active" : "nav";
    button.textContent = view.label;
    button.addEventListener("click", () => {
      update({ view: view.id, folderId: view.id === "folder" ? model.folderId : null, selected: [] });
      void refresh().catch(showError);
    });
    aside.append(button);
  }
  if (model.info?.readOnly) {
    const badge = document.createElement("p");
    badge.className = "badge";
    badge.textContent = "Lecture seule";
    aside.append(badge);
  }
  const lock = document.createElement("button");
  lock.type = "button";
  lock.className = "nav lock";
  lock.textContent = "Verrouiller";
  lock.addEventListener("click", () => void lockVault());
  aside.append(lock);
  return aside;
}

function mainPane(model: AppModel): HTMLElement {
  const pane = document.createElement("section");
  pane.className = "pane";
  pane.append(toolbar(model), crumbs(model), listing(model), status(model));
  return pane;
}

function toolbar(model: AppModel): HTMLElement {
  const bar = document.createElement("div");
  bar.className = "toolbar";
  const readOnly = model.info?.readOnly === true;
  const trash = model.view === "trash";
  bar.append(
    action("Ajouter fichiers", readOnly || trash, () => void addFiles()),
    action("Ajouter dossier", readOnly || trash, () => void addFolder()),
    action("Créer dossier", readOnly || trash, () => void createFolder()),
    action("Exporter", model.selected.length === 0, () => void exportSelected()),
    action(trash ? "Supprimer" : "Mettre à la corbeille", readOnly || model.selected.length === 0, () =>
      void removeSelected(trash),
    ),
    action("Renommer", readOnly || model.selected.length !== 1, () => void renameSelected()),
    action("Ouvrir", model.selected.length !== 1, () => void openSelected()),
  );
  if (trash) {
    bar.append(action("Restaurer", readOnly || model.selected.length === 0, () => void restoreSelected()));
  }
  const spacer = document.createElement("span");
  spacer.className = "spacer";
  const layout = document.createElement("button");
  layout.type = "button";
  layout.className = "ghost";
  layout.textContent = model.layout === "list" ? "Icônes" : "Liste";
  layout.addEventListener("click", () => update({ layout: model.layout === "list" ? "icons" : "list" }));
  const more = document.createElement("button");
  more.type = "button";
  more.className = "ghost";
  more.textContent = "Coffre";
  more.addEventListener("click", () => void vaultMenu());
  bar.append(spacer, layout, more);
  return bar;
}

function crumbs(model: AppModel): HTMLElement {
  const nav = document.createElement("nav");
  nav.className = "crumbs";
  if (model.view !== "folder") {
    nav.textContent = VIEWS.find((view) => view.id === model.view)?.label ?? "";
    return nav;
  }
  const root = document.createElement("button");
  root.type = "button";
  root.className = "link";
  root.textContent = model.info?.vaultName ?? "Coffre";
  root.addEventListener("click", () => {
    update({ folderId: null, selected: [] });
    void refresh().catch(showError);
  });
  nav.append(root);
  for (const crumb of model.crumbs) {
    const sep = document.createElement("span");
    sep.textContent = " / ";
    const button = document.createElement("button");
    button.type = "button";
    button.className = "link";
    button.textContent = crumb.name;
    button.addEventListener("click", () => {
      update({ folderId: crumb.id, selected: [] });
      void refresh().catch(showError);
    });
    nav.append(sep, button);
  }
  return nav;
}

function listing(model: AppModel): HTMLElement {
  const area = document.createElement("div");
  area.className = model.layout === "icons" ? "files icons drop" : "files drop";
  area.id = "dropzone";
  if (model.entries.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = model.view === "trash" ? "La corbeille est vide." : "Aucun fichier pour le moment.";
    area.append(empty);
    return area;
  }
  for (const entry of model.entries) {
    area.append(row(entry, model));
  }
  return area;
}

function row(entry: Entry, model: AppModel): HTMLElement {
  const item = document.createElement(model.layout === "icons" ? "button" : "div");
  item.className = model.selected.includes(entry.id) ? "item selected" : "item";
  if (model.layout === "icons") item.setAttribute("type", "button");
  const icon = document.createElement("span");
  icon.className = entry.kind === "folder" ? "glyph folder" : "glyph file";
  icon.textContent = entry.kind === "folder" ? "Dossier" : ext(entry.name);
  const label = document.createElement("span");
  label.className = "name";
  label.textContent = entry.name;
  const meta = document.createElement("span");
  meta.className = "meta";
  meta.textContent = entry.kind === "folder" ? "" : formatSize(entry.size);
  const date = document.createElement("span");
  date.className = "meta";
  date.textContent = new Date(entry.modifiedMs).toLocaleString("fr");
  item.append(icon, label, meta, date);
  item.addEventListener("click", (event) => {
    const mouse = event as MouseEvent;
    select(entry.id, mouse.metaKey || mouse.ctrlKey);
  });
  item.addEventListener("dblclick", () => void openEntry(entry));
  return item;
}

function status(model: AppModel): HTMLElement {
  const line = document.createElement("footer");
  line.className = "status";
  const count = document.createElement("span");
  count.textContent = `${model.entries.length} élément${model.entries.length > 1 ? "s" : ""}`;
  const lock = document.createElement("span");
  const seconds = model.info?.autoLockSecs ?? 0;
  lock.textContent = seconds === 0 ? "Verrouillage automatique désactivé" : `Verrouillage après ${labelFor(seconds)}`;
  if (model.error) {
    const error = document.createElement("span");
    error.className = "error";
    error.textContent = model.error;
    line.append(error);
  }
  line.append(count, lock);
  return line;
}

function progressOverlay(model: AppModel): HTMLElement {
  const overlay = document.createElement("div");
  overlay.className = "overlay";
  const box = document.createElement("div");
  box.className = "dialog";
  const title = document.createElement("h2");
  title.textContent = model.busy ?? "Opération en cours";
  const bar = document.createElement("div");
  bar.className = "bar";
  const fill = document.createElement("span");
  const total = model.progress?.total;
  const done = model.progress?.done ?? 0;
  fill.style.width = total && total > 0 ? `${Math.min(100, Math.round((done / total) * 100))}%` : "40%";
  bar.append(fill);
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "ghost";
  cancel.textContent = "Annuler";
  cancel.addEventListener("click", () => void api.cancel());
  box.append(title, bar, cancel);
  overlay.append(box);
  return overlay;
}

function action(label: string, disabled: boolean, run: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.disabled = disabled;
  if (label !== "Ajouter fichiers" && label !== "Ajouter dossier" && label !== "Créer dossier" && label !== "Exporter" && label !== "Ouvrir" && label !== "Restaurer") {
    button.className = label.startsWith("Supprimer") || label.startsWith("Mettre") ? "danger" : "";
  }
  button.addEventListener("click", run);
  return button;
}

function select(id: string, additive: boolean): void {
  const current = state().selected;
  if (additive) {
    const selected = current.includes(id) ? current.filter((item) => item !== id) : [...current, id];
    update({ selected });
    return;
  }
  update({ selected: [id] });
}

async function addFiles(): Promise<void> {
  const paths = await pickFiles();
  if (paths.length === 0) return;
  await runJob("Ajout des fichiers…", () => api.importPaths(paths, folderTarget(), false));
}

async function addFolder(): Promise<void> {
  const path = await pickDirectory();
  if (!path) return;
  await runJob("Ajout du dossier…", () => api.importPaths([path], folderTarget(), true));
}

async function createFolder(): Promise<void> {
  const name = await ask("Créer un dossier", "Nom");
  if (!name) return;
  try {
    await api.createFolder(folderTarget(), name.trim());
    await refresh();
  } catch (caught) {
    showError(caught);
  }
}

async function renameSelected(): Promise<void> {
  const entry = selectedEntry();
  if (!entry) return;
  const name = await ask("Renommer", "Nouveau nom", entry.name);
  if (!name) return;
  try {
    await api.rename(entry.id, name.trim());
    await refresh();
  } catch (caught) {
    showError(caught);
  }
}

async function removeSelected(permanent: boolean): Promise<void> {
  const ids = state().selected;
  if (ids.length === 0) return;
  const agreed = await confirmBox(
    permanent ? "Supprimer définitivement" : "Mettre à la corbeille",
    permanent
      ? "Ces éléments seront retirés du coffre. Le fichier d'origine sur votre ordinateur n'est pas touché."
      : "Vous pourrez encore les restaurer depuis la corbeille.",
  );
  if (!agreed) return;
  try {
    if (permanent) await api.deletePermanent(ids);
    else await api.trash(ids);
    update({ selected: [] });
    await refresh();
  } catch (caught) {
    showError(caught);
  }
}

async function restoreSelected(): Promise<void> {
  try {
    await api.restore(state().selected);
    update({ selected: [] });
    await refresh();
  } catch (caught) {
    showError(caught);
  }
}

async function exportSelected(): Promise<void> {
  const destination = await pickDirectory();
  if (!destination) return;
  const agreed = await confirmBox(
    "Exporter",
    "Les fichiers exportés ne sont plus chiffrés. Ils seront écrits à l'emplacement choisi, jamais sur la clé dans le coffre.",
  );
  if (!agreed) return;
  await runJob("Export…", () => api.exportEntries(state().selected, destination));
}

async function openSelected(): Promise<void> {
  const entry = selectedEntry();
  if (entry) await openEntry(entry);
}

async function openEntry(entry: Entry): Promise<void> {
  if (entry.kind === "folder") {
    if (state().view !== "folder" || entry.trashed) return;
    update({ folderId: entry.id, view: "folder", selected: [] });
    await refresh();
    return;
  }
  try {
    await runJob("Préparation du fichier…", async () => {
      await api.prepareTemp(entry.id);
    });
    await api.launchTemp(entry.id);
  } catch (caught) {
    showError(caught);
  }
}

function vaultMenu(): void {
  const overlay = document.createElement("div");
  overlay.className = "overlay";
  const dialog = document.createElement("div");
  dialog.className = "dialog";
  const title = document.createElement("h2");
  title.textContent = "Coffre";
  const readOnly = state().info?.readOnly === true;
  const close = () => overlay.remove();
  const verify = menuButton("Vérifier le coffre", async () => {
    close();
    const report = await api.verify();
    const lines = report.issues.length
      ? report.issues.map((issue) => `${issue.code} (${issue.detail})`).join("\n")
      : "Aucune anomalie trouvée.";
    await confirmBox(report.ok ? "Coffre vérifié" : "Anomalies trouvées", `${report.checked} fichier(s) contrôlé(s).\n${lines}`);
  });
  const backup = menuButton("Sauvegarder le coffre", async () => {
    close();
    const destination = await pickDirectory();
    if (!destination) return;
    await runJob("Sauvegarde…", () => api.backup(destination));
  });
  const password = menuButton("Changer le mot de passe", async () => {
    close();
    const current = await ask("Mot de passe actuel", "Mot de passe", "", true);
    if (!current) return;
    const next = await ask("Nouveau mot de passe", "Nouveau mot de passe", "", true);
    if (!next) return;
    const confirm = await ask("Confirmer", "Confirmez le nouveau mot de passe", "", true);
    if (!confirm) return;
    update({ busy: "Mise à jour du mot de passe…" });
    await api.changePassword(current, next, confirm);
    update({ busy: null });
  });
  password.disabled = readOnly;
  const delayLabel = document.createElement("label");
  delayLabel.textContent = "Verrouillage automatique";
  const select = document.createElement("select");
  for (const [seconds, label] of [
    [60, "1 minute"],
    [300, "5 minutes"],
    [900, "15 minutes"],
    [1800, "30 minutes"],
    [0, "Jamais"],
  ] as const) {
    const option = document.createElement("option");
    option.value = String(seconds);
    option.textContent = label;
    option.selected = state().info?.autoLockSecs === seconds;
    select.append(option);
  }
  select.disabled = readOnly;
  select.addEventListener("change", () => {
    void api.setAutoLock(Number(select.value)).then(async () => {
      update({ info: await api.info() });
    }).catch(showError);
  });
  delayLabel.append(select);
  const help = menuButton("Aide", async () => {
    close();
    await confirmBox(
      "Ce que SecureKey protège",
      "Le chiffrement protège les données lorsque le coffre est verrouillé. Il ne protège pas contre un ordinateur compromis, un keylogger ou un malware lorsque vous saisissez votre mot de passe.",
    );
  });
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "ghost";
  cancel.textContent = "Fermer";
  cancel.addEventListener("click", close);
  dialog.append(title, verify, backup, password, delayLabel, help, cancel);
  overlay.append(dialog);
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) close();
  });
  document.body.append(overlay);
}

function menuButton(label: string, run: () => Promise<void>): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "ghost";
  button.textContent = label;
  button.addEventListener("click", () => {
    void run().catch(showError);
  });
  return button;
}

async function lockVault(): Promise<void> {
  const temps = await api.tempStatus().catch(() => []);
  const dirty = temps.filter((item) => item.changed);
  if (dirty.length > 0) {
    const agreed = await confirmBox(
      "Fichier modifié",
      `${dirty[0].name} a été modifié. Voulez-vous réimporter cette version avant de verrouiller ?`,
    );
    if (agreed) {
      await runJob("Réimport…", () => api.reimportTemp(dirty[0].entryId));
    } else {
      await api.discardTemp(dirty[0].entryId);
    }
  }
  for (const item of temps) {
    if (!dirty.some((changed) => changed.entryId === item.entryId)) {
      await api.discardTemp(item.entryId);
    }
  }
  await api.lock();
  window.clearInterval(poll);
  update({
    screen: "unlock",
    info: null,
    entries: [],
    selected: [],
    crumbs: [],
    busy: null,
    error: null,
    discoveredPath: state().info?.vaultPath ?? state().discoveredPath,
  });
}

async function runJob(label: string, job: () => Promise<unknown>): Promise<void> {
  const stop = await onProgress((done, total) => update({ progress: { done, total } }));
  update({ busy: label, progress: { done: 0, total: null }, error: null });
  try {
    await job();
    await refresh();
  } catch (caught) {
    showError(caught);
  } finally {
    stop();
    update({ busy: null, progress: null });
  }
}

function installDrag(): void {
  if (dragInstalled || !inTauri()) return;
  dragInstalled = true;
  void getCurrentWebview().onDragDropEvent((event) => {
    const zone = document.getElementById("dropzone");
    if (event.payload.type === "over" || event.payload.type === "enter") {
      zone?.classList.add("over");
    } else {
      zone?.classList.remove("over");
    }
    if (event.payload.type === "drop" && !state().info?.readOnly) {
      const paths = event.payload.paths;
      if (paths.length > 0) void runJob("Ajout des fichiers…", () => api.importPaths(paths, folderTarget(), false));
    }
  });
}

function armTempPoll(): void {
  window.clearInterval(poll);
  poll = window.setInterval(() => {
    if (state().screen !== "browser") return;
    void api.tempStatus().then(async (temps) => {
      const changed = temps.find((item) => item.changed && !dismissedTemps.has(item.entryId));
      if (!changed || state().busy) return;
      const agreed = await confirmBox("Nouvelle version", `${changed.name} a changé. Réimporter cette version dans le coffre ?`);
      if (agreed && !state().info?.readOnly) {
        dismissedTemps.delete(changed.entryId);
        await runJob("Réimport…", () => api.reimportTemp(changed.entryId));
      } else {
        dismissedTemps.add(changed.entryId);
      }
    });
  }, 4000);
}

function folderTarget(): string | null {
  return state().view === "folder" ? state().folderId : null;
}

function selectedEntry(): Entry | undefined {
  const id = state().selected[0];
  return state().entries.find((entry) => entry.id === id);
}

function showError(caught: unknown): void {
  update({ error: String(caught), busy: null, progress: null });
}

function formatSize(size: number): string {
  if (size < 1024) return `${size} o`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} Ko`;
  if (size < 1024 * 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} Mo`;
  return `${(size / 1024 / 1024 / 1024).toFixed(1)} Go`;
}

function ext(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return "Fichier";
  return name.slice(dot + 1, dot + 5).toUpperCase();
}

function labelFor(seconds: number): string {
  if (seconds === 60) return "1 minute";
  if (seconds % 60 === 0) return `${seconds / 60} minutes`;
  return `${seconds} secondes`;
}
