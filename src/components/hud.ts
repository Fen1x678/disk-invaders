import { h } from "../dom";
import { scoreDigits } from "../format";
import { getLang, LANGUAGES, setLang, t, type Lang } from "../i18n";
import { applyPrefs, savePrefs, THEMES, type Prefs } from "../prefs";
import { setSoundEnabled, sfx } from "../sound";
import { sprite } from "../sprites";

export interface Hud {
  el: HTMLElement;
  /** Adds freed bytes to the score (and the hi-score when beaten). */
  addScore(bytes: number): void;
  render(): void;
}

/** The arcade header: logo, SCORE / HI-SCORE and the settings switches. */
export function createHud(prefs: Prefs, onLanguageChange: () => void): Hud {
  const el = h("header", { class: "hud" });
  let score = 0;

  const save = () => {
    savePrefs(prefs);
    applyPrefs(prefs);
    setSoundEnabled(prefs.sound);
  };

  const scoreBox = (label: string, bytes: number, extra = "") => {
    const { value, unit } = scoreDigits(bytes);
    return h("div", { class: `hud-score ${extra}` }, h("span", { class: "hud-label" }, label), h("span", { class: "hud-digits" }, value, h("small", {}, unit)));
  };

  const toggle = (label: string, value: string, onclick: () => void) =>
    h("button", { class: "hud-toggle", onclick }, h("span", { class: "hud-label" }, label), h("span", { class: "hud-value" }, value));

  function render() {
    el.replaceChildren(
      h("div", { class: "hud-logo" }, sprite("crab", "animate"), h("span", {}, "DISK INVADERS")),
      scoreBox(t("hud.score"), score),
      scoreBox(t("hud.hiscore"), prefs.hiScore, "hi"),
      h(
        "div",
        { class: "hud-controls" },
        toggle(t("hud.theme"), prefs.theme.toUpperCase(), () => {
          prefs.theme = THEMES[(THEMES.indexOf(prefs.theme) + 1) % THEMES.length];
          save();
          sfx.blip();
          render();
        }),
        toggle(t("hud.sound"), prefs.sound ? "ON" : "OFF", () => {
          prefs.sound = !prefs.sound;
          save();
          sfx.blip();
          render();
        }),
        toggle(t("hud.crt"), prefs.crt ? "ON" : "OFF", () => {
          prefs.crt = !prefs.crt;
          save();
          sfx.blip();
          render();
        }),
        toggle("LANG", getLang().toUpperCase(), () => {
          const langs = Object.keys(LANGUAGES) as Lang[];
          prefs.lang = langs[(langs.indexOf(getLang()) + 1) % langs.length];
          setLang(prefs.lang);
          save();
          sfx.blip();
          render();
          onLanguageChange();
        }),
      ),
    );
  }

  render();
  return {
    el,
    render,
    addScore(bytes) {
      score += bytes;
      if (score > prefs.hiScore) {
        prefs.hiScore = score;
        save();
      }
      render();
      el.querySelector(".hud-score")?.classList.add("bump");
    },
  };
}
