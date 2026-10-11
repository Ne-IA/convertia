// PLANTED-POSITIVE armed canary for the T2 DOM/eval sink ban in eslint.config.mjs (build-gates G5,
// security-concept section 5 T2). It DELIBERATELY names every banned sink and MUST fail eslint: do not
// "fix" it. check-ts-gate lints it as src/main.tsx (stdin) before the real run, and every line ending in
// an eslint-must-fire marker must report each named rule at error severity. Each banned property, each
// selector, each member of a selector's name list, each spelling of a called method's member name, each
// wrapper around a callee that a .tsx file can hold, each case-insensitive spelling, each template-literal
// argument form, each timer callee and handler form the docs name, and each javascript: URL spelling a URL
// parser accepts (leading controls and spaces, tabs and newlines in the scheme) has a line of its own that
// only it makes fire, so a ban narrowed by one element leaves a pin silent. This file is L(-1), and
// eslint.config.mjs ignores scripts/, so the real run never sees it.
declare const el: HTMLElement;
declare const doc: Document;
declare const range: Range;
declare const parser: DOMParser;
declare const frame: HTMLIFrameElement;
declare const w: Window;
declare const s: string;
declare const markup: { __html: string };
declare function createElement(tag: string, props: object): unknown;

export function htmlSinks(): void {
  el.innerHTML = s; // eslint-must-fire: no-restricted-properties
  el.innerHTML += s; // eslint-must-fire: no-restricted-properties
  el["innerHTML"] = s; // eslint-must-fire: no-restricted-properties
  el.outerHTML = s; // eslint-must-fire: no-restricted-properties
  el[`outerHTML`] = s; // eslint-must-fire: no-restricted-properties
  el.insertAdjacentHTML("beforeend", s); // eslint-must-fire: no-restricted-properties
  document.write(s); // eslint-must-fire: no-restricted-properties
  document.writeln(s); // eslint-must-fire: no-restricted-properties
  doc.write(s); // eslint-must-fire: no-restricted-properties
  window.document.write(s); // eslint-must-fire: no-restricted-properties
  range.createContextualFragment(s); // eslint-must-fire: no-restricted-properties
  el.setHTMLUnsafe(s); // eslint-must-fire: no-restricted-properties
  Document.parseHTMLUnsafe(s); // eslint-must-fire: no-restricted-properties
  parser.parseFromString(s, "text/html"); // eslint-must-fire: no-restricted-properties
  document.execCommand("insertHTML", false, s); // eslint-must-fire: no-restricted-properties
  frame.srcdoc = s; // eslint-must-fire: no-restricted-properties
  (el?.insertAdjacentHTML)("beforeend", s); // eslint-must-fire: no-restricted-properties
  (document?.write)(s); // eslint-must-fire: no-restricted-properties
  (document.writeln as Document["writeln"])(s); // eslint-must-fire: no-restricted-properties
  const { insertAdjacentHTML: insert } = el; // eslint-must-fire: no-restricted-properties
  void insert;
}

export function keySinks(): unknown[] {
  return [
    Object.assign(el, { innerHTML: s }), // eslint-must-fire: no-restricted-syntax
    Object.assign(el, { outerHTML: s }), // eslint-must-fire: no-restricted-syntax
    Object.assign(frame, { srcdoc: s }), // eslint-must-fire: no-restricted-syntax
    createElement("div", { dangerouslySetInnerHTML: markup }), // eslint-must-fire: no-restricted-syntax
    { __html: s }, // eslint-must-fire: no-restricted-syntax
    Object.assign(el, { "innerHTML": s }), // eslint-must-fire: no-restricted-syntax
    Object.assign(el, { "outerHTML": s }), // eslint-must-fire: no-restricted-syntax
    Object.assign(frame, { "srcdoc": s }), // eslint-must-fire: no-restricted-syntax
    createElement("div", { "dangerouslySetInnerHTML": markup }), // eslint-must-fire: no-restricted-syntax
    { "__html": s }, // eslint-must-fire: no-restricted-syntax
    createElement("iframe", { srcDoc: s }), // eslint-must-fire: no-restricted-syntax
    createElement("iframe", { "srcDoc": s }), // eslint-must-fire: no-restricted-syntax
  ];
}

export function reactSinks(): unknown[] {
  return [
    <div dangerouslySetInnerHTML={{ __html: s }} />, // eslint-must-fire: react/no-danger, no-restricted-syntax
    <section dangerouslySetInnerHTML={{ __html: s }}>{s}</section>, // eslint-must-fire: react/no-danger-with-children
    <Custom dangerouslySetInnerHTML={markup} />, // eslint-must-fire: react/no-danger
    <iframe srcDoc={s} />, // eslint-must-fire: no-restricted-syntax
  ];
}

declare function Custom(props: object): null;

export function attributeSinks(): void {
  el.setAttribute("onclick", s); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("ONCLICK", s); // eslint-must-fire: no-restricted-syntax
  el.setAttribute(`onclick`, s); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("srcdoc", s); // eslint-must-fire: no-restricted-syntax
  el.setAttributeNS(null, "onload", s); // eslint-must-fire: no-restricted-syntax
  el.setAttributeNS(null, "ONLOAD", s); // eslint-must-fire: no-restricted-syntax
  el.setAttributeNS(null, `onload`, s); // eslint-must-fire: no-restricted-syntax
  document.createAttribute("onclick"); // eslint-must-fire: no-restricted-syntax
  document.createAttributeNS(null, "srcdoc"); // eslint-must-fire: no-restricted-syntax
  document.createElement("script"); // eslint-must-fire: no-restricted-syntax
  document.createElement("SCRIPT"); // eslint-must-fire: no-restricted-syntax
  document.createElement(`script`); // eslint-must-fire: no-restricted-syntax
  document.createElementNS("http://www.w3.org/2000/svg", "script"); // eslint-must-fire: no-restricted-syntax
  document.createElementNS("http://www.w3.org/2000/svg", "SCRIPT"); // eslint-must-fire: no-restricted-syntax
  document.createElementNS("http://www.w3.org/2000/svg", `script`); // eslint-must-fire: no-restricted-syntax
  document.createElementNS("http://www.w3.org/1999/xhtml", "h:script"); // eslint-must-fire: no-restricted-syntax
  el["setAttribute"]("onclick", s); // eslint-must-fire: no-restricted-syntax
  el[`setAttribute`]("onclick", s); // eslint-must-fire: no-restricted-syntax
  el?.["setAttribute"]("onclick", s); // eslint-must-fire: no-restricted-syntax
  el["setAttributeNS"](null, "onload", s); // eslint-must-fire: no-restricted-syntax
  el[`setAttributeNS`](null, "onload", s); // eslint-must-fire: no-restricted-syntax
  document["createElement"]("script"); // eslint-must-fire: no-restricted-syntax
  document[`createElement`]("script"); // eslint-must-fire: no-restricted-syntax
  document["createElementNS"]("http://www.w3.org/2000/svg", "script"); // eslint-must-fire: no-restricted-syntax
  document[`createElementNS`]("http://www.w3.org/2000/svg", "script"); // eslint-must-fire: no-restricted-syntax
  (el?.setAttribute)("onclick", s); // eslint-must-fire: no-restricted-syntax
  (el?.["setAttribute"])("onclick", s); // eslint-must-fire: no-restricted-syntax
  (el?.[`setAttribute`])("onclick", s); // eslint-must-fire: no-restricted-syntax
  (el.setAttribute as HTMLElement["setAttribute"])("onclick", s); // eslint-must-fire: no-restricted-syntax
  (el.setAttribute satisfies unknown)("onclick", s); // eslint-must-fire: no-restricted-syntax
  el.setAttribute!("onclick", s); // eslint-must-fire: no-restricted-syntax
  (document?.createAttribute)("onclick"); // eslint-must-fire: no-restricted-syntax
  (el?.setAttributeNS)(null, "onload", s); // eslint-must-fire: no-restricted-syntax
  el.setAttributeNS!(null, "onload", s); // eslint-must-fire: no-restricted-syntax
  (document.createAttributeNS as Document["createAttributeNS"])(null, "srcdoc"); // eslint-must-fire: no-restricted-syntax
  (document?.createElement)("script"); // eslint-must-fire: no-restricted-syntax
  (document.createElement<"script">)("script"); // eslint-must-fire: no-restricted-syntax
  (document["createElement"] satisfies unknown)("script"); // eslint-must-fire: no-restricted-syntax
  (document?.createElementNS)("http://www.w3.org/2000/svg", "script"); // eslint-must-fire: no-restricted-syntax
  (document.createElementNS<"script">)("http://www.w3.org/2000/svg", "script"); // eslint-must-fire: no-restricted-syntax
  (document[`createElementNS`] as Document["createElementNS"])("http://www.w3.org/2000/svg", "svg:script"); // eslint-must-fire: no-restricted-syntax
}

export function codeSinks(): void {
  eval(s); // eslint-must-fire: no-eval
  window.eval(s); // eslint-must-fire: no-eval
  (0, eval)(s); // eslint-must-fire: no-eval
  self.eval(s); // eslint-must-fire: no-restricted-properties
  new Function(s); // eslint-must-fire: no-new-func, no-restricted-globals
  Function(s); // eslint-must-fire: no-new-func, no-restricted-globals
  Function.call(null, s); // eslint-must-fire: no-new-func, no-restricted-globals
  const F = Function; // eslint-must-fire: no-restricted-globals
  globalThis.Function(s); // eslint-must-fire: no-restricted-globals
  parent.Function(s); // eslint-must-fire: no-restricted-properties
  const ev = eval; // eslint-must-fire: no-eval
  top!.eval(s); // eslint-must-fire: no-restricted-properties
  frames.eval(s); // eslint-must-fire: no-restricted-properties
  document.defaultView!.Function(s); // eslint-must-fire: no-restricted-properties
  const code = "run()";
  setTimeout("run(" + s + ")", 0); // eslint-must-fire: no-implied-eval, @typescript-eslint/no-implied-eval
  setTimeout("run()", 0); // eslint-must-fire: no-implied-eval, @typescript-eslint/no-implied-eval
  setInterval(`run()`, 0); // eslint-must-fire: no-implied-eval, @typescript-eslint/no-implied-eval
  setTimeout(code, 0); // eslint-must-fire: no-implied-eval
  window.setTimeout("run()", 0); // eslint-must-fire: no-implied-eval
  globalThis["setInterval"]("run()", 0); // eslint-must-fire: no-implied-eval
  window?.setTimeout("run()", 0); // eslint-must-fire: no-implied-eval
  setTimeout(s, 0); // eslint-must-fire: @typescript-eslint/no-implied-eval
  setInterval(s, 0); // eslint-must-fire: @typescript-eslint/no-implied-eval
  window.setTimeout(s, 0); // eslint-must-fire: @typescript-eslint/no-implied-eval
  window["setTimeout"](s, 0); // eslint-must-fire: @typescript-eslint/no-implied-eval
  globalThis.setInterval(s, 0); // eslint-must-fire: @typescript-eslint/no-implied-eval
  window?.setInterval(s, 0); // eslint-must-fire: @typescript-eslint/no-implied-eval
  (window?.setTimeout)(s, 0); // eslint-must-fire: no-restricted-syntax
  (globalThis?.setInterval)(s, 0); // eslint-must-fire: no-restricted-syntax
  (window?.["setTimeout"])(s, 0); // eslint-must-fire: no-restricted-syntax
  (globalThis?.[`setInterval`])(s, 0); // eslint-must-fire: no-restricted-syntax
  (window.setTimeout satisfies unknown)(s, 0); // eslint-must-fire: no-restricted-syntax
  (globalThis.setInterval as typeof setInterval)(s, 0); // eslint-must-fire: no-restricted-syntax
  window.setTimeout!(s, 0); // eslint-must-fire: no-restricted-syntax
  (window.setInterval<[]>)(s, 0); // eslint-must-fire: no-restricted-syntax
  (window?.setTimeout)(() => undefined, 0); // eslint-must-fire: no-restricted-syntax
  (setTimeout satisfies unknown)(s, 0); // eslint-must-fire: no-restricted-syntax
  (setInterval as typeof setInterval)(s, 0); // eslint-must-fire: no-restricted-syntax
  setTimeout!(s, 0); // eslint-must-fire: no-restricted-syntax
  (setInterval<[]>)(s, 0); // eslint-must-fire: no-restricted-syntax
  window[`setTimeout`](s, 0); // eslint-must-fire: no-restricted-syntax
  globalThis?.[`setInterval`](s, 0); // eslint-must-fire: no-restricted-syntax
  self.setTimeout(s, 0); // eslint-must-fire: no-restricted-properties
  w.setInterval(s, 0); // eslint-must-fire: no-restricted-properties
  el.setAttribute("href", "javascript:void(0)"); // eslint-must-fire: no-script-url
  el.setAttribute("href", `javascript:${s}`); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", `JAVASCRIPT:${s}`); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", ` javascript:${s}`); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", `java\nscript:${s}`); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", " javascript:void(0)"); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", " JAVASCRIPT:void(0)"); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", "\x00javascript:void(0)"); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", "\x01javascript:void(0)"); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", "java\tscript:void(0)"); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", "java\nscript:void(0)"); // eslint-must-fire: no-restricted-syntax
  el.setAttribute("href", "javascript\r:void(0)"); // eslint-must-fire: no-restricted-syntax
  // eslint-disable-next-line no-eval -- an inline directive has no effect (noInlineConfig)
  eval(s); // eslint-must-fire: no-eval
  void F;
  void ev;
}

export function localTimerName(setInterval: (handler: string) => void): void {
  (setInterval as (handler: string) => void)(s); // eslint-must-fire: no-restricted-syntax
}
