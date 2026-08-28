// ---------------------------------------------------------------------------
// Theme palettes. Each theme is a full set of the CSS custom properties that
// src/style.css and the Tailwind config consume. Values are HSL triples
// ("H S% L%") so they slot straight into `hsl(var(--x))`.
//
// To add a theme: append an entry to `themes` with the SAME set of keys as
// `mocha` below. `dark: true|false` only drives the checkbox/preview hint.
// ---------------------------------------------------------------------------

export type ThemeVars = {
  "--background": string;
  "--panel": string;
  "--toolbar": string;
  "--foreground": string;
  "--card": string;
  "--card-foreground": string;
  "--muted": string;
  "--muted-foreground": string;
  "--accent": string;
  "--accent-foreground": string;
  "--secondary": string;
  "--secondary-foreground": string;
  "--selection": string;
  "--selection-foreground": string;
  "--selection-muted": string;
  "--link": string;
  "--primary": string;
  "--primary-foreground": string;
  "--destructive": string;
  "--destructive-foreground": string;
  "--border": string;
  "--border-strong": string;
  "--input": string;
  "--ring": string;
};

export interface Theme {
  id: string;
  label: string;
  dark: boolean;
  vars: ThemeVars;
}

// Catppuccin Mocha — the default.
const mocha: ThemeVars = {
  "--background": "240 21% 15%",
  "--panel": "237 16% 23%",
  "--toolbar": "240 21% 12%",
  "--foreground": "226 64% 88%",
  "--card": "237 16% 23%",
  "--card-foreground": "226 64% 88%",
  "--muted": "237 16% 23%",
  "--muted-foreground": "228 24% 72%",
  "--accent": "234 13% 31%",
  "--accent-foreground": "226 64% 88%",
  "--secondary": "237 16% 23%",
  "--secondary-foreground": "226 64% 88%",
  "--selection": "217 92% 76%",
  "--selection-foreground": "240 21% 12%",
  "--selection-muted": "234 13% 31%",
  "--link": "217 92% 76%",
  "--primary": "217 92% 76%",
  "--primary-foreground": "240 21% 12%",
  "--destructive": "343 81% 68%",
  "--destructive-foreground": "240 21% 12%",
  "--border": "234 13% 27%",
  "--border-strong": "234 13% 34%",
  "--input": "240 21% 12%",
  "--ring": "217 92% 76%",
};

// Prism Void — the reference chrome and the default. Hex anchors:
// #0b0c0e canvas · #121417 panel/dock · #1a1d23 hover · emerald refraction edge.
const prismDark: ThemeVars = {
  "--background": "220 12% 5%",
  "--panel": "216 12% 8%",
  "--toolbar": "216 12% 8%",
  "--foreground": "220 12% 86%",
  "--card": "216 12% 8%",
  "--card-foreground": "220 12% 86%",
  "--muted": "220 12% 11%",
  "--muted-foreground": "220 7% 50%",
  "--accent": "220 15% 12%",
  "--accent-foreground": "220 12% 93%",
  "--secondary": "220 13% 14%",
  "--secondary-foreground": "220 12% 88%",
  "--selection": "216 52% 52%",
  "--selection-foreground": "210 40% 98%",
  "--selection-muted": "220 26% 20%",
  "--link": "205 68% 64%",
  "--primary": "216 52% 52%",
  "--primary-foreground": "210 40% 98%",
  "--destructive": "0 60% 52%",
  "--destructive-foreground": "210 20% 96%",
  "--border": "220 10% 15%",
  "--border-strong": "220 10% 23%",
  "--input": "220 12% 7%",
  "--ring": "216 52% 52%",
};

// Catppuccin Macchiato.
const macchiato: ThemeVars = {
  "--background": "232 23% 18%",
  "--panel": "230 19% 26%",
  "--toolbar": "233 23% 15%",
  "--foreground": "227 68% 88%",
  "--card": "230 19% 26%",
  "--card-foreground": "227 68% 88%",
  "--muted": "230 19% 26%",
  "--muted-foreground": "227 27% 72%",
  "--accent": "231 17% 32%",
  "--accent-foreground": "227 68% 88%",
  "--secondary": "230 19% 26%",
  "--secondary-foreground": "227 68% 88%",
  "--selection": "220 83% 75%",
  "--selection-foreground": "233 23% 15%",
  "--selection-muted": "220 30% 40%",
  "--link": "220 83% 75%",
  "--primary": "220 83% 75%",
  "--primary-foreground": "233 23% 15%",
  "--destructive": "351 74% 73%",
  "--destructive-foreground": "233 23% 15%",
  "--border": "232 20% 22%",
  "--border-strong": "231 18% 30%",
  "--input": "233 23% 15%",
  "--ring": "220 83% 75%",
};

// Catppuccin Frappé.
const frappe: ThemeVars = {
  "--background": "229 19% 23%",
  "--panel": "230 16% 30%",
  "--toolbar": "231 19% 20%",
  "--foreground": "227 70% 87%",
  "--card": "230 16% 30%",
  "--card-foreground": "227 70% 87%",
  "--muted": "230 16% 30%",
  "--muted-foreground": "227 25% 72%",
  "--accent": "230 14% 36%",
  "--accent-foreground": "227 70% 87%",
  "--secondary": "230 16% 30%",
  "--secondary-foreground": "227 70% 87%",
  "--selection": "222 74% 74%",
  "--selection-foreground": "231 19% 20%",
  "--selection-muted": "222 30% 40%",
  "--link": "222 74% 74%",
  "--primary": "222 74% 74%",
  "--primary-foreground": "231 19% 20%",
  "--destructive": "359 68% 71%",
  "--destructive-foreground": "231 19% 20%",
  "--border": "229 17% 27%",
  "--border-strong": "230 15% 34%",
  "--input": "231 19% 20%",
  "--ring": "222 74% 74%",
};

// Catppuccin Latte (light).
const latte: ThemeVars = {
  "--background": "220 23% 95%",
  "--panel": "220 33% 99%",
  "--toolbar": "220 22% 92%",
  "--foreground": "234 16% 35%",
  "--card": "220 33% 99%",
  "--card-foreground": "234 16% 35%",
  "--muted": "220 33% 99%",
  "--muted-foreground": "233 13% 47%",
  "--accent": "223 16% 88%",
  "--accent-foreground": "234 16% 35%",
  "--secondary": "220 33% 99%",
  "--secondary-foreground": "234 16% 35%",
  "--selection": "220 91% 54%",
  "--selection-foreground": "220 30% 98%",
  "--selection-muted": "220 60% 88%",
  "--link": "220 91% 54%",
  "--primary": "220 91% 54%",
  "--primary-foreground": "220 30% 98%",
  "--destructive": "347 87% 44%",
  "--destructive-foreground": "0 0% 100%",
  "--border": "223 16% 87%",
  "--border-strong": "223 16% 78%",
  "--input": "220 30% 99%",
  "--ring": "220 91% 54%",
};

// Dracula.
const dracula: ThemeVars = {
  "--background": "231 15% 18%",
  "--panel": "232 14% 31%",
  "--toolbar": "231 15% 14%",
  "--foreground": "60 30% 96%",
  "--card": "232 14% 31%",
  "--card-foreground": "60 30% 96%",
  "--muted": "232 14% 31%",
  "--muted-foreground": "226 27% 64%",
  "--accent": "232 14% 38%",
  "--accent-foreground": "60 30% 96%",
  "--secondary": "232 14% 31%",
  "--secondary-foreground": "60 30% 96%",
  "--selection": "265 89% 78%",
  "--selection-foreground": "231 15% 14%",
  "--selection-muted": "265 30% 45%",
  "--link": "265 89% 78%",
  "--primary": "265 89% 78%",
  "--primary-foreground": "231 15% 14%",
  "--destructive": "0 100% 67%",
  "--destructive-foreground": "231 15% 14%",
  "--border": "231 14% 24%",
  "--border-strong": "232 14% 34%",
  "--input": "231 15% 14%",
  "--ring": "265 89% 78%",
};

// Nord.
const nord: ThemeVars = {
  "--background": "220 16% 22%",
  "--panel": "222 16% 28%",
  "--toolbar": "220 16% 18%",
  "--foreground": "218 27% 92%",
  "--card": "222 16% 28%",
  "--card-foreground": "218 27% 92%",
  "--muted": "222 16% 28%",
  "--muted-foreground": "219 20% 65%",
  "--accent": "220 17% 32%",
  "--accent-foreground": "218 27% 92%",
  "--secondary": "222 16% 28%",
  "--secondary-foreground": "218 27% 92%",
  "--selection": "213 32% 52%",
  "--selection-foreground": "218 27% 94%",
  "--selection-muted": "213 32% 35%",
  "--link": "210 34% 63%",
  "--primary": "213 32% 52%",
  "--primary-foreground": "218 27% 94%",
  "--destructive": "354 42% 56%",
  "--destructive-foreground": "218 27% 94%",
  "--border": "220 16% 26%",
  "--border-strong": "220 17% 34%",
  "--input": "220 16% 18%",
  "--ring": "213 32% 52%",
};

// Tokyo Night.
const tokyoNight: ThemeVars = {
  "--background": "235 19% 13%",
  "--panel": "228 23% 21%",
  "--toolbar": "235 19% 10%",
  "--foreground": "229 73% 86%",
  "--card": "228 23% 21%",
  "--card-foreground": "229 73% 86%",
  "--muted": "228 23% 21%",
  "--muted-foreground": "229 23% 58%",
  "--accent": "228 23% 27%",
  "--accent-foreground": "229 73% 86%",
  "--secondary": "228 23% 21%",
  "--secondary-foreground": "229 73% 86%",
  "--selection": "221 89% 72%",
  "--selection-foreground": "235 19% 10%",
  "--selection-muted": "221 40% 40%",
  "--link": "221 89% 72%",
  "--primary": "221 89% 72%",
  "--primary-foreground": "235 19% 10%",
  "--destructive": "349 89% 72%",
  "--destructive-foreground": "235 19% 10%",
  "--border": "235 19% 18%",
  "--border-strong": "228 23% 26%",
  "--input": "235 19% 10%",
  "--ring": "221 89% 72%",
};

// Gruvbox Dark.
const gruvboxDark: ThemeVars = {
  "--background": "0 0% 16%",
  "--panel": "20 5% 22%",
  "--toolbar": "0 0% 12%",
  "--foreground": "43 59% 81%",
  "--card": "20 5% 22%",
  "--card-foreground": "43 59% 81%",
  "--muted": "20 5% 22%",
  "--muted-foreground": "37 13% 63%",
  "--accent": "22 7% 29%",
  "--accent-foreground": "43 59% 81%",
  "--secondary": "20 5% 22%",
  "--secondary-foreground": "43 59% 81%",
  "--selection": "157 16% 58%",
  "--selection-foreground": "0 0% 12%",
  "--selection-muted": "157 16% 38%",
  "--link": "157 16% 58%",
  "--primary": "157 16% 58%",
  "--primary-foreground": "0 0% 12%",
  "--destructive": "6 96% 59%",
  "--destructive-foreground": "43 30% 96%",
  "--border": "0 0% 20%",
  "--border-strong": "22 7% 29%",
  "--input": "0 0% 12%",
  "--ring": "157 16% 58%",
};

// Solarized Dark.
const solarizedDark: ThemeVars = {
  "--background": "192 100% 11%",
  "--panel": "192 81% 14%",
  "--toolbar": "193 100% 9%",
  "--foreground": "186 8% 60%",
  "--card": "192 81% 14%",
  "--card-foreground": "186 8% 60%",
  "--muted": "192 81% 14%",
  "--muted-foreground": "194 14% 50%",
  "--accent": "192 60% 18%",
  "--accent-foreground": "186 8% 65%",
  "--secondary": "192 81% 14%",
  "--secondary-foreground": "186 8% 60%",
  "--selection": "205 69% 49%",
  "--selection-foreground": "192 100% 96%",
  "--selection-muted": "205 40% 30%",
  "--link": "205 69% 49%",
  "--primary": "205 69% 49%",
  "--primary-foreground": "192 100% 96%",
  "--destructive": "1 71% 52%",
  "--destructive-foreground": "192 100% 96%",
  "--border": "192 70% 16%",
  "--border-strong": "192 50% 22%",
  "--input": "193 100% 9%",
  "--ring": "205 69% 49%",
};

// One Dark.
const oneDark: ThemeVars = {
  "--background": "220 13% 18%",
  "--panel": "219 13% 25%",
  "--toolbar": "216 13% 15%",
  "--foreground": "219 14% 71%",
  "--card": "219 13% 25%",
  "--card-foreground": "219 14% 71%",
  "--muted": "219 13% 25%",
  "--muted-foreground": "219 10% 55%",
  "--accent": "219 13% 31%",
  "--accent-foreground": "219 14% 78%",
  "--secondary": "219 13% 25%",
  "--secondary-foreground": "219 14% 71%",
  "--selection": "207 82% 66%",
  "--selection-foreground": "216 13% 12%",
  "--selection-muted": "207 40% 38%",
  "--link": "207 82% 66%",
  "--primary": "207 82% 66%",
  "--primary-foreground": "216 13% 12%",
  "--destructive": "355 65% 65%",
  "--destructive-foreground": "216 13% 12%",
  "--border": "220 13% 22%",
  "--border-strong": "219 13% 30%",
  "--input": "216 13% 15%",
  "--ring": "207 82% 66%",
};

// Rosé Pine.
const rosePine: ThemeVars = {
  "--background": "249 22% 12%",
  "--panel": "247 23% 15%",
  "--toolbar": "249 22% 10%",
  "--foreground": "245 50% 91%",
  "--card": "247 23% 15%",
  "--card-foreground": "245 50% 91%",
  "--muted": "247 23% 15%",
  "--muted-foreground": "248 15% 63%",
  "--accent": "248 25% 18%",
  "--accent-foreground": "245 50% 91%",
  "--secondary": "247 23% 15%",
  "--secondary-foreground": "245 50% 91%",
  "--selection": "267 57% 78%",
  "--selection-foreground": "249 22% 10%",
  "--selection-muted": "267 30% 45%",
  "--link": "267 57% 78%",
  "--primary": "267 57% 78%",
  "--primary-foreground": "249 22% 10%",
  "--destructive": "343 76% 68%",
  "--destructive-foreground": "249 22% 10%",
  "--border": "248 22% 16%",
  "--border-strong": "248 25% 22%",
  "--input": "249 22% 10%",
  "--ring": "267 57% 78%",
};

// GitHub Light.
const githubLight: ThemeVars = {
  "--background": "0 0% 100%",
  "--panel": "210 29% 97%",
  "--toolbar": "210 29% 97%",
  "--foreground": "213 13% 14%",
  "--card": "210 29% 97%",
  "--card-foreground": "213 13% 14%",
  "--muted": "210 29% 97%",
  "--muted-foreground": "212 8% 43%",
  "--accent": "210 18% 92%",
  "--accent-foreground": "213 13% 14%",
  "--secondary": "210 29% 97%",
  "--secondary-foreground": "213 13% 14%",
  "--selection": "212 92% 45%",
  "--selection-foreground": "210 29% 98%",
  "--selection-muted": "212 80% 90%",
  "--link": "212 92% 45%",
  "--primary": "212 92% 45%",
  "--primary-foreground": "0 0% 100%",
  "--destructive": "356 72% 47%",
  "--destructive-foreground": "0 0% 100%",
  "--border": "210 18% 84%",
  "--border-strong": "210 18% 74%",
  "--input": "0 0% 100%",
  "--ring": "212 92% 45%",
};

export const themes: Theme[] = [
  // Prism Void first: it is the default, and getTheme() falls back to themes[0].
  { id: "prism-dark", label: "Prism Void", dark: true, vars: prismDark },
  { id: "catppuccin-mocha", label: "Catppuccin Mocha", dark: true, vars: mocha },
  { id: "catppuccin-macchiato", label: "Catppuccin Macchiato", dark: true, vars: macchiato },
  { id: "catppuccin-frappe", label: "Catppuccin Frappé", dark: true, vars: frappe },
  { id: "catppuccin-latte", label: "Catppuccin Latte", dark: false, vars: latte },
  { id: "dracula", label: "Dracula", dark: true, vars: dracula },
  { id: "nord", label: "Nord", dark: true, vars: nord },
  { id: "tokyo-night", label: "Tokyo Night", dark: true, vars: tokyoNight },
  { id: "gruvbox-dark", label: "Gruvbox Dark", dark: true, vars: gruvboxDark },
  { id: "solarized-dark", label: "Solarized Dark", dark: true, vars: solarizedDark },
  { id: "one-dark", label: "One Dark", dark: true, vars: oneDark },
  { id: "rose-pine", label: "Rosé Pine", dark: true, vars: rosePine },
  { id: "github-light", label: "GitHub Light", dark: false, vars: githubLight },
];

export const DEFAULT_THEME = "prism-dark";

export function getTheme(id: string): Theme {
  return themes.find((t) => t.id === id) ?? themes[0];
}
