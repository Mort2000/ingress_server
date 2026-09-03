// Dev-time headless check of an emitted viewer page (M10, D93 note in
// docs/progress.md). NOT part of the build or the test suite — cargo never
// invokes it and the artifact never depends on it; it exists so the claim
// "the concatenated viewer JS boots and the in-page selftest passes" is
// reproducible wherever a local JS engine exists:
//
//   node scripts/viewer-headless-check.mjs [viewer/sample.html]
//
// It extracts the second <script> block of the page (the concatenated
// viewer JS), runs it under a minimal DOM shim with ?selftest=1 forced,
// and reports the selftest verdict. The VISUAL half of D93 (canvas looks,
// keyboard feel, animation) still needs a human with a browser.

import { readFileSync } from "node:fs";

const path = process.argv[2] ?? "viewer/sample.html";
const html = readFileSync(path, "utf8");

const blocks = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].map((m) => m[1]);
if (blocks.length !== 2) {
  console.error(`expected 2 script blocks (data + viewer js), found ${blocks.length}`);
  process.exit(1);
}

// --- minimal DOM shim -------------------------------------------------------
function makeElement(id) {
  const sizes = { map: [920, 660], curve: [560, 210] };
  const element = {
    id,
    innerHTML: "",
    textContent: "",
    value: "0",
    max: "100",
    title: "",
    disabled: false,
    style: {},
    dataset: {},
    lastElementChild: null,
    children: [],
    classList: { toggle() {}, add() {}, remove() {} },
    addEventListener() {},
    appendChild(child) {
      this.children.push(child);
      this.lastElementChild = child;
    },
    scrollIntoView() {},
  };
  if (sizes[id]) {
    const [width, height] = sizes[id];
    element.width = width;
    element.height = height;
    element.getContext = () => new Proxy(
      {},
      {
        get: (_target, prop) => {
          if (typeof prop === "string" && /^(fillStyle|strokeStyle|lineWidth|font|textAlign|globalAlpha)$/.test(prop)) {
            return undefined;
          }
          return () => undefined;
        },
        set: () => true,
      }
    );
  }
  return element;
}

const elements = new Map();
const documentShim = {
  title: "shim",
  getElementById(id) {
    if (!elements.has(id)) elements.set(id, makeElement(id));
    return elements.get(id);
  },
  createElement(tag) {
    return makeElement(`<${tag}>`);
  },
  querySelectorAll() {
    return [];
  },
  addEventListener() {},
};
const locationShim = { search: "?selftest=1" };
const windowShim = { setInterval: () => 0, clearInterval: () => {} };

// --- run the concatenated viewer JS ----------------------------------------
// In a browser, a top-level `const` in one <script> is visible to the
// later scripts (shared global lexical scope), so concatenating the data
// block before the viewer block reproduces the page's execution order.
const source = `"use strict";\n${blocks[0]}\n${blocks[1]}`;
try {
  const runner = new Function("document", "location", "window", source);
  runner(documentShim, locationShim, windowShim);
} catch (error) {
  console.error(`BOOT FAIL: viewer js threw: ${error?.stack ?? error}`);
  process.exit(1);
}

const verdict = elements.get("selftest-result")?.innerHTML ?? "";
const pass = /SELFTEST (\d+)\/(\d+)/.exec(verdict);
if (!pass) {
  console.error("SELFTEST DID NOT RUN (no verdict rendered)");
  process.exit(1);
}
const [, got, total] = pass;
const failed = /FAIL/.test(verdict);
console.log(`selftest ${got}/${total} ${failed ? "FAIL" : "PASS"} (${path})`);
if (failed) {
  console.error(verdict.replace(/<[^>]+>/g, " | "));
  process.exit(1);
}
