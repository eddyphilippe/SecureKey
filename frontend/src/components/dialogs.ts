export function ask(title: string, label: string, value = "", password = false): Promise<string | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "overlay";
    const dialog = document.createElement("form");
    dialog.className = "dialog";
    const heading = document.createElement("h2");
    heading.textContent = title;
    const fieldLabel = document.createElement("label");
    fieldLabel.textContent = label;
    const input = document.createElement("input");
    input.type = password ? "password" : "text";
    input.value = value;
    input.autocomplete = "off";
    input.required = true;
    fieldLabel.append(input);
    const actions = document.createElement("div");
    actions.className = "row";
    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "ghost";
    cancel.textContent = "Annuler";
    const ok = document.createElement("button");
    ok.type = "submit";
    ok.textContent = "Valider";
    actions.append(cancel, ok);
    dialog.append(heading, fieldLabel, actions);
    overlay.append(dialog);
    document.body.append(overlay);
    input.focus();
    const close = (result: string | null) => {
      overlay.remove();
      resolve(result);
    };
    cancel.addEventListener("click", () => close(null));
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) close(null);
    });
    dialog.addEventListener("submit", (event) => {
      event.preventDefault();
      close(input.value);
    });
  });
}

export function confirmBox(title: string, message: string): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "overlay";
    const dialog = document.createElement("div");
    dialog.className = "dialog";
    const heading = document.createElement("h2");
    heading.textContent = title;
    const text = document.createElement("p");
    text.textContent = message;
    const actions = document.createElement("div");
    actions.className = "row";
    const cancel = document.createElement("button");
    cancel.type = "button";
    cancel.className = "ghost";
    cancel.textContent = "Annuler";
    const ok = document.createElement("button");
    ok.type = "button";
    ok.className = "danger";
    ok.textContent = "Confirmer";
    actions.append(cancel, ok);
    dialog.append(heading, text, actions);
    overlay.append(dialog);
    document.body.append(overlay);
    const close = (result: boolean) => {
      overlay.remove();
      resolve(result);
    };
    cancel.addEventListener("click", () => close(false));
    ok.addEventListener("click", () => close(true));
  });
}
