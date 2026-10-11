import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import type { Crumb, Entry, SessionInfo, TempStatus, VerifyReport } from "./types";

export function inTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function pickDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, title: "Choisir un dossier" });
  return typeof selected === "string" ? selected : null;
}

export async function pickFiles(): Promise<string[]> {
  const selected = await open({ directory: false, multiple: true, title: "Ajouter des fichiers" });
  if (typeof selected === "string") return [selected];
  return Array.isArray(selected) ? selected : [];
}

export function onProgress(handler: (done: number, total: number | null) => void): Promise<() => void> {
  return listen<{ done: number; total: number | null }>("job-progress", (event) => {
    handler(event.payload.done, event.payload.total);
  });
}

export const api = {
  discover: () => invoke<string | null>("discover"),
  createVault: (directory: string, name: string, password: string, confirm: string) =>
    invoke<SessionInfo>("create_vault", { directory, name, password, confirm }),
  openVault: (directory: string, password: string, readOnly: boolean) =>
    invoke<SessionInfo>("open_vault", { directory, password, readOnly }),
  lock: () => invoke<void>("lock"),
  info: () => invoke<SessionInfo | null>("session_info"),
  changePassword: (current: string, next: string, confirm: string) =>
    invoke<void>("change_password", { current, next, confirm }),
  setAutoLock: (seconds: number) => invoke<void>("set_auto_lock", { seconds }),
  verify: () => invoke<VerifyReport>("verify_vault"),
  backup: (destination: string) => invoke<string>("backup_vault", { destination }),
  cancel: () => invoke<void>("cancel_job"),
  list: (view: string, folderId: string | null) =>
    invoke<Entry[]>("list_entries", { view, folderId }),
  breadcrumbs: (folderId: string | null) => invoke<Crumb[]>("breadcrumbs", { folderId }),
  importPaths: (paths: string[], parentId: string | null, folders: boolean) =>
    invoke<number>("import_paths", { paths, parentId, folders }),
  createFolder: (parentId: string | null, name: string) =>
    invoke<Entry>("create_folder", { parentId, name }),
  rename: (id: string, name: string) => invoke<Entry>("rename_entry", { id, name }),
  comment: (id: string, comment: string) => invoke<Entry>("set_comment", { id, comment }),
  favorite: (id: string, favorite: boolean) => invoke<Entry>("set_favorite", { id, favorite }),
  move: (id: string, parentId: string | null) => invoke<Entry>("move_entry", { id, parentId }),
  trash: (ids: string[]) => invoke<void>("trash_entries", { ids }),
  restore: (ids: string[]) => invoke<void>("restore_entries", { ids }),
  deletePermanent: (ids: string[]) => invoke<void>("delete_permanent", { ids }),
  exportEntries: (ids: string[], destination: string) =>
    invoke<number>("export_entries", { ids, destination }),
  prepareTemp: (id: string) => invoke<string>("prepare_temp", { id }),
  launchTemp: (id: string) => invoke<void>("launch_temp", { id }),
  tempStatus: () => invoke<TempStatus[]>("temp_status"),
  reimportTemp: (id: string) => invoke<Entry>("reimport_temp", { id }),
  discardTemp: (id: string) => invoke<void>("discard_temp", { id }),
};
