// A small floating context menu for the hub. VS Code webviews can't use the
// native editor context menu, so this renders our own, themed with VS Code
// variables. One menu at a time; dismissed on outside click, scroll or Escape.

export interface MenuAction {
  label: string;
  icon?: string;
  danger?: boolean;
  run: () => void;
}

let current: HTMLElement | undefined;

export function closeMenu(): void {
  current?.remove();
  current = undefined;
}

export function openMenu(x: number, y: number, actions: MenuAction[]): void {
  closeMenu();
  const menu = document.createElement("div");
  menu.className = "ctxmenu";

  for (const action of actions) {
    const item = document.createElement("div");
    item.className = "ctxmenu-item" + (action.danger ? " danger" : "");
    if (action.icon) {
      const ic = document.createElement("span");
      ic.className = "ctxmenu-icon";
      ic.innerHTML = action.icon;
      item.appendChild(ic);
    }
    const label = document.createElement("span");
    label.textContent = action.label;
    item.appendChild(label);
    item.addEventListener("click", (e) => {
      e.stopPropagation();
      closeMenu();
      action.run();
    });
    menu.appendChild(item);
  }

  document.body.appendChild(menu);
  current = menu;

  // Keep the menu within the viewport.
  const rect = menu.getBoundingClientRect();
  const left = Math.min(x, window.innerWidth - rect.width - 4);
  const top = Math.min(y, window.innerHeight - rect.height - 4);
  menu.style.left = `${Math.max(2, left)}px`;
  menu.style.top = `${Math.max(2, top)}px`;
}

// Global dismissers, registered once.
document.addEventListener("click", () => closeMenu());
document.addEventListener("contextmenu", (e) => {
  // Let row handlers open their own menu; close any stale one first.
  if (!(e.target as HTMLElement)?.closest(".row")) {
    closeMenu();
  }
});
window.addEventListener("scroll", () => closeMenu(), true);
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    closeMenu();
  }
});
