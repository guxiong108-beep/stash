export type Theme = "light" | "warm";

export function resolveTheme(input: string | undefined): Theme {
  return input === "light" ? "light" : "warm";
}

export function applyTheme(
  theme: Theme,
  root: HTMLElement = document.documentElement,
): void {
  root.setAttribute("data-theme", theme);
}
