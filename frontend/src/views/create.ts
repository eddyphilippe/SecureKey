import { api, inTauri, pickDirectory } from "../api";
import { update } from "../stores/session";

function joinVault(parent: string): string {
  const separator = parent.includes("\\") ? "\\" : "/";
  const trimmed = parent.endsWith(separator) ? parent.slice(0, -1) : parent;
  const name = trimmed.split(separator).pop();
  if (name === "Vault") return trimmed;
  return `${trimmed}${separator}Vault`;
}

export function renderCreate(root: HTMLElement): void {
  root.replaceChildren();
  const screen = document.createElement("section");
  screen.className = "gate";
  const card = document.createElement("form");
  card.className = "card";
  card.innerHTML = "";

  const mark = document.createElement("p");
  mark.className = "mark";
  mark.textContent = "SecureZip";
  const title = document.createElement("h1");
  title.textContent = "Nouveau coffre";
  const intro = document.createElement("p");
  intro.className = "muted";
  intro.textContent = "Choisissez un dossier. Un dossier Vault y sera créé, avec vos fichiers chiffrés.";

  const nameLabel = document.createElement("label");
  nameLabel.textContent = "Nom du coffre";
  const name = document.createElement("input");
  name.required = true;
  name.maxLength = 80;
  name.value = "Mon coffre";
  nameLabel.append(name);

  const place = document.createElement("button");
  place.type = "button";
  place.className = "ghost";
  place.textContent = "Choisir l'emplacement";
  const placeText = document.createElement("p");
  placeText.className = "muted path";
  let directory = "";

  const passwordLabel = document.createElement("label");
  passwordLabel.textContent = "Mot de passe";
  const password = document.createElement("input");
  password.type = "password";
  password.autocomplete = "new-password";
  password.required = true;
  password.minLength = 8;
  passwordLabel.append(password);

  const confirmLabel = document.createElement("label");
  confirmLabel.textContent = "Confirmer le mot de passe";
  const confirm = document.createElement("input");
  confirm.type = "password";
  confirm.autocomplete = "new-password";
  confirm.required = true;
  confirmLabel.append(confirm);

  const hint = document.createElement("p");
  hint.className = "hint";
  hint.textContent = "Choisissez une phrase longue. Elle ne sera pas enregistrée.";

  const error = document.createElement("p");
  error.className = "error";
  error.hidden = true;

  const actions = document.createElement("div");
  actions.className = "row";
  const back = document.createElement("button");
  back.type = "button";
  back.className = "ghost";
  back.textContent = "Retour";
  const submit = document.createElement("button");
  submit.type = "submit";
  submit.textContent = "Créer le coffre";
  actions.append(back, submit);

  card.append(mark, title, intro, place, placeText, nameLabel, passwordLabel, confirmLabel, hint, error, actions);
  screen.append(card);
  root.append(screen);

  back.addEventListener("click", () => update({ screen: "welcome", error: null }));
  place.addEventListener("click", async () => {
    if (!inTauri()) {
      error.hidden = false;
      error.textContent = "Le choix de dossier fonctionne dans l'application SecureZip.";
      return;
    }
    const picked = await pickDirectory();
    if (!picked) return;
    directory = joinVault(picked);
    placeText.textContent = directory;
  });
  card.addEventListener("submit", async (event) => {
    event.preventDefault();
    error.hidden = true;
    if (!directory) {
      error.hidden = false;
      error.textContent = "Choisissez d'abord un emplacement.";
      return;
    }
    submit.disabled = true;
    update({ busy: "Préparation du coffre…" });
    try {
      const info = await api.createVault(directory, name.value.trim(), password.value, confirm.value);
      password.value = "";
      confirm.value = "";
      update({
        screen: "ready",
        info,
        busy: null,
        error: null,
        readyMessage: "Votre coffre est prêt.",
      });
    } catch (caught) {
      password.value = "";
      confirm.value = "";
      error.hidden = false;
      error.textContent = String(caught);
      update({ busy: null });
    } finally {
      submit.disabled = false;
    }
  });
}
