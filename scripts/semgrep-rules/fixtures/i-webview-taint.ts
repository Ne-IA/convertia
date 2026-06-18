// PLANTED-POSITIVE armed canary for G29 rule (i) — DELIBERATELY violates the WebView taint rule
// and MUST be flagged. DO NOT "fix" it. This dir is L(-1).
// rule (i): convertia-webview-taint-to-dom-sink (invoke result -> innerHTML)
import { invoke } from "@tauri-apps/api/core";

export async function render(el: HTMLElement): Promise<void> {
  const html = (await invoke("get_html")) as string;
  el.innerHTML = html;
}
