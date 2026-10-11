import { inTauri, pickDirectory } from "../api";
import { update } from "../stores/session";

export function renderWelcome(root: HTMLElement, discovered: string | null): void {
  root.replaceChildren();
  const screen = document.createElement("section");
  screen.className = "gate";
  const card = document.createElement("div");
  card.className = "card hero";
  const mark = document.createElement("p");
  mark.className = "mark";
  mark.textContent = "NomadVault";
  const title = document.createElement("h1");
  title.textContent = "Coffre-fort de fichiers";
  const text = document.createElement("p");
  text.textContent = "Gardez vos documents sur une clé USB. Personne ne peut lire les noms ni le contenu tant que le coffre est verrouillé.";

  const actions = document.createElement("div");
  actions.className = "stack";
  if (discovered) {
    const found = document.createElement("p");
    found.className = "muted";
    found.textContent = "Un coffre a été trouvé à côté de l'application.";
    const openFound = document.createElement("button");
    openFound.type = "button";
    openFound.textContent = "Déverrouiller";
    openFound.addEventListener("click", () => update({ screen: "unlock", discoveredPath: discovered, error: null }));
    card.append(found);
    actions.append(openFound);
  }
  const create = document.createElement("button");
  create.type = "button";
  create.textContent = "Créer un nouveau coffre";
  const open = document.createElement("button");
  open.type = "button";
  open.className = "ghost";
  open.textContent = "Ouvrir un coffre existant";
  actions.append(create, open);
  card.prepend(mark, title, text);
  card.append(actions);
  if (!inTauri()) {
    const note = document.createElement("p");
    note.className = "hint";
    note.textContent = "Aperçu de l'interface. Le coffre se déverrouille dans l'application NomadVault.";
    card.append(note);
  }
  screen.append(card);
  root.append(screen);

  create.addEventListener("click", () => update({ screen: "create", error: null }));
  open.addEventListener("click", async () => {
    if (!inTauri()) {
      update({ screen: "unlock", discoveredPath: null, error: "Ouvrez NomadVault pour choisir un coffre." });
      return;
    }
    const picked = await pickDirectory();
    if (picked) update({ screen: "unlock", discoveredPath: picked, error: null });
  });
}
