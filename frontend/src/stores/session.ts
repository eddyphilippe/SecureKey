import type { Entry, Layout, Screen, SessionInfo, ViewId } from "../types";

export interface AppModel {
  screen: Screen;
  info: SessionInfo | null;
  discoveredPath: string | null;
  error: string | null;
  busy: string | null;
  progress: { done: number; total: number | null } | null;
  view: ViewId;
  layout: Layout;
  folderId: string | null;
  entries: Entry[];
  crumbs: { id: string; name: string }[];
  selected: string[];
  readyMessage: string | null;
}

const model: AppModel = {
  screen: "welcome",
  info: null,
  discoveredPath: null,
  error: null,
  busy: null,
  progress: null,
  view: "folder",
  layout: "list",
  folderId: null,
  entries: [],
  crumbs: [],
  selected: [],
  readyMessage: null,
};

const listeners = new Set<() => void>();

export function state(): AppModel {
  return model;
}

export function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function update(patch: Partial<AppModel>): void {
  Object.assign(model, patch);
  for (const listener of listeners) listener();
}
