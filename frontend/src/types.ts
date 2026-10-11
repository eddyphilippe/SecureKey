export interface SessionInfo {
  vaultName: string;
  readOnly: boolean;
  autoLockSecs: number;
  vaultPath: string;
}

export interface Entry {
  id: string;
  parentId: string | null;
  kind: "file" | "folder";
  name: string;
  size: number;
  createdMs: number;
  modifiedMs: number;
  favorite: boolean;
  trashed: boolean;
  comment: string;
}

export interface Crumb {
  id: string;
  name: string;
}

export interface VerifyIssue {
  severity: "error" | "warning" | string;
  code: string;
  detail: string;
}

export interface VerifyReport {
  ok: boolean;
  checked: number;
  issues: VerifyIssue[];
}

export interface TempStatus {
  entryId: string;
  name: string;
  changed: boolean;
}

export type ViewId = "folder" | "documents" | "images" | "favorites" | "trash";
export type Layout = "list" | "icons";
export type Screen = "welcome" | "create" | "unlock" | "ready" | "browser";
