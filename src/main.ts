import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme, resolveTheme } from "./theme";
import { renderClipboard, bindClipboardActions } from "./clipboard_view";

interface AppConfig {
  theme: string;
  hotkey_main: string;
  hotkey_paste: string;
  max_clipboard: number;
}

let mode = "all";

function setupTabs(): void {
  const tabs = document.getElementById("tabs")!;
  tabs.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest(".tab") as HTMLElement | null;
    if (!btn) return;
    tabs.querySelectorAll(".tab").forEach((t) => t.classList.remove("is-active"));
    btn.classList.add("is-active");
    mode = btn.dataset.mode ?? "all";
    refresh();
  });
}

function refresh(): void {
  const q = (document.getElementById("search") as HTMLInputElement).value;
  if (mode === "clipboard" || mode === "all") {
    void renderClipboard(q);
  } else {
    (document.getElementById("clip-list") as HTMLElement).innerHTML = "";
    (document.getElementById("hint") as HTMLElement).style.display = "block";
  }
}

async function init(): Promise<void> {
  try {
    const cfg = await invoke<AppConfig>("get_config");
    applyTheme(resolveTheme(cfg.theme));
  } catch {
    applyTheme("warm");
  }
  setupTabs();
  bindClipboardActions();
  (document.getElementById("search") as HTMLInputElement).addEventListener("input", refresh);
  await listen("clip://changed", () => refresh());
  refresh();
}

void init();
