import "@fontsource/press-start-2p/400.css";
import "@fontsource/vt323/400.css";
import "@fontsource/pixelify-sans/400.css";
import "./styles/main.css";

import { createBackend } from "./api";
import { messageDialog } from "./components/dialog";
import { createHud } from "./components/hud";
import { h, type Screen } from "./dom";
import { useDecimalUnits } from "./format";
import { detectLang, setLang, t } from "./i18n";
import { applyPrefs, loadPrefs, savePrefs } from "./prefs";
import { createResultsState, resultsScreen } from "./screens/results";
import { scanScreen, type ScanView } from "./screens/scan";
import { titleScreen, type TitleState } from "./screens/title";
import { setSoundEnabled, sfx } from "./sound";

const OLD_DOWNLOAD_DAYS = 180;

async function main() {
  const prefs = loadPrefs();
  setLang(prefs.lang ?? detectLang());
  applyPrefs(prefs);
  setSoundEnabled(prefs.sound);

  const backend = await createBackend();
  const info = await backend.systemInfo();
  useDecimalUnits(info.platform === "macos");

  // Every screen is rebuilt from a factory, so changing the language just re-runs it.
  const stage = h("main", { class: "stage" });
  let current: Screen | null = null;
  let factory: () => Screen = () => ({ el: h("div") });
  const show = (next: () => Screen) => {
    current?.destroy?.();
    factory = next;
    current = next();
    stage.replaceChildren(current.el);
  };

  const hud = createHud(prefs, () => show(factory));
  document.getElementById("app")!.append(hud.el, stage);

  const title: TitleState = { scope: "home", customPath: null, findDuplicates: prefs.findDuplicates };

  const goTitle = () => show(() => titleScreen({ info, backend, state: title, onStart: (roots, expected) => void startScan(roots, expected) }));

  async function startScan(roots: string[], expectedBytes: number | null) {
    prefs.findDuplicates = title.findDuplicates;
    savePrefs(prefs);
    let view: ScanView | null = null;
    show(() => (view = scanScreen({ expectedBytes, onAbort: () => void backend.cancelScan() })));
    sfx.start();
    try {
      const report = await backend.startScan(roots, { findDuplicates: title.findDuplicates, oldDownloadDays: OLD_DOWNLOAD_DAYS }, (p) => view?.update(p));
      sfx.coin();
      const state = createResultsState(report, OLD_DOWNLOAD_DAYS);
      show(() => resultsScreen({ state, info, backend, hud, onNewGame: goTitle }));
    } catch (error) {
      sfx.error();
      goTitle();
      await messageDialog({ title: t("scan.failed", { error: String(error) }), lines: [] });
    }
  }

  goTitle();
}

void main();
