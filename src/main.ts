import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { applyTheme, resolveTheme } from "./theme";

interface AppConfig {
  theme: string;
  hotkey_main: string;
  hotkey_paste: string;
  max_clipboard: number;
}

async function init(): Promise<void> {
  try {
    const cfg = await invoke<AppConfig>("get_config");
    applyTheme(resolveTheme(cfg.theme));
  } catch {
    applyTheme("warm");
  }
}

void init();
