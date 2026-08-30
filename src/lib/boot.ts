// The launch screen's moving parts: advance the progress bar, then hand the window
// over to the mounted app.
//
// Deliberately dependency-free — it reads localStorage directly rather than
// importing i18n or the theme, because it runs before the config that both of those
// hang off has been hydrated. The markup and the styles it drives live in
// index.html / edit.html; the note there explains why they cannot be a component.

/** How long the launch screen stays up, measured from navigation start.
 *
 *  Not padding for its own sake: on this machine the two startup calls land in well
 *  under 100 ms, and a bar that flicks 0→100 in one frame reads as a glitch, not as
 *  a launch. Long enough to be a deliberate animation, short enough not to be a
 *  wait. Lower it and the bar starts disappearing before it is seen. */
const MIN_MS = 420;
/** A beat on a full bar before the fade, so the fill visibly completes. */
const HOLD_MS = 110;

/** Steps, in order: [fill fraction, zh, en]. Each names the phase being *entered*,
 *  so a step is announced before its work, not after. */
const STEPS = {
  config: [0.42, "正在读取启动器配置…", "Reading launcher config…"],
  ui: [0.86, "正在准备界面…", "Preparing the interface…"],
  done: [1, "", ""],
} as const satisfies Record<string, readonly [number, string, string]>;

export type BootStep = keyof typeof STEPS;

/** The locale i18n will settle on, from the same first-paint cache it keeps. */
function isEnglish(): boolean {
  try {
    return localStorage.getItem("agentlauncher.locale") === "en";
  } catch {
    return false;
  }
}

/** Push the bar to a step. Cheap and idempotent; safe before the DOM is complete. */
export function bootStep(step: BootStep): void {
  const [fraction, zh, en] = STEPS[step];
  const fill = document.getElementById("boot-fill");
  if (fill) fill.style.width = `${Math.round(fraction * 100)}%`;
  const msg = document.getElementById("boot-msg");
  if (msg) msg.textContent = isEnglish() ? en : zh;
}

/** Fill the bar, then cross-fade the launch screen out. Safe to call twice.
 *
 *  Call it from a `finally`: a render that throws still has to hand the window
 *  over, or one mistake in one component leaves a bar sitting at 86% forever with
 *  the real error only in the console. */
export function reveal(): void {
  bootStep("done");
  // performance.now() is measured from navigation start, so this is the remaining
  // part of the minimum — not an unconditional delay.
  setTimeout(finish, Math.max(0, MIN_MS - performance.now()) + HOLD_MS);
}

function finish(): void {
  document.getElementById("app")?.classList.add("ready");
  const boot = document.getElementById("boot");
  if (!boot) return;
  boot.classList.add("done");
  boot.addEventListener("transitionend", () => boot.remove(), { once: true });
  // Backstop: transitionend never fires if the element is not actually animating
  // (a webview that skips the transition, an interrupted paint). The overlay must
  // come off either way — it covers the whole window.
  setTimeout(() => boot.remove(), 600);
}
