import { api } from "./api";
import { update } from "./stores/session";

export async function chooseVaultFolder(picked: string): Promise<void> {
  const kind = await api.classify(picked);
  if (kind === "blank") {
    update({ screen: "create", discoveredPath: picked, error: null });
    return;
  }
  if (kind === "vault") {
    update({ screen: "unlock", discoveredPath: picked, error: null });
    return;
  }
  update({
    screen: "unlock",
    discoveredPath: null,
    error: "Ce dossier n'est pas un coffre NomadVault.",
  });
}
