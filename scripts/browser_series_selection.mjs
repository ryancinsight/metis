/** Real WASM/DOM acceptance. Playwright is a test-runner prerequisite only. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import http from "node:http";
import { createRequire } from "node:module";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const playwright = require(process.env.METIS_PLAYWRIGHT_MODULE || "playwright");
const engine = process.env.METIS_TEST_BROWSER || "chromium";
assert.ok(["chromium", "firefox", "webkit"].includes(engine), "supported test engine");
const output = path.join(root, "output", "series-selection-evidence", engine);
const routes = new Map([
  ["/gallery.html", "text/html"],
  ["/gallery.css", "text/css"],
  ["/gallery.js", "text/javascript"],
  ["/consumer/series_selection.js", "text/javascript"],
  ["/consumer/series_selection_bg.wasm", "application/wasm"],
]);

async function snapshot(page) {
  return page.evaluate(() => {
    const app = document.getElementById("selection-app");
    return {
      active: app.getAttribute("data-active-viewport"),
      selected: app.getAttribute("data-selected-series"),
      listeners: app.getAttribute("data-selection-listener-count"),
      series: [...app.querySelectorAll(".series-row")].map(button => ({
        id: button.id, pressed: button.getAttribute("aria-pressed"),
      })),
      viewports: [...app.querySelectorAll(".series-viewport")].map(button => ({
        id: button.id, pressed: button.getAttribute("aria-pressed"),
        series: button.getAttribute("data-series-id"),
        title: button.querySelector("strong").textContent,
        detail: button.querySelector("span:nth-of-type(2)").textContent,
      })),
      summary: document.getElementById("selection-summary")?.textContent,
    };
  });
}

const titles = {
  "fixture-ct": "Fixture CT series",
  "fixture-mr": "Fixture MR series",
  "fixture-secondary": "Fixture secondary series",
};
const details = {
  "fixture-ct": "CT-DEMO · selection metadata only",
  "fixture-mr": "MR-DEMO · selection metadata only",
  "fixture-secondary": "SECONDARY-DEMO · selection metadata only",
};

function checkState(actual, active, assignments) {
  const selected = assignments[active];
  assert.equal(actual.active, String(active));
  assert.equal(actual.selected, selected);
  assert.equal(actual.listeners, "1");
  assert.deepEqual(actual.series, Object.keys(titles).map(key => ({
    id: `series-${key}`, pressed: String(key === selected),
  })));
  assert.deepEqual(actual.viewports, assignments.map((series, index) => ({
    id: `viewport-${index}`, pressed: String(index === active), series,
    title: titles[series] || "No demonstration series assigned",
    detail: details[series] || "Select a series fixture for this viewport",
  })));
  assert.equal(actual.summary, `Active viewport ${active + 1}; selected series: ${titles[selected] || "none"}`);
}

test("Rust fixture selection updates only the active viewport", { timeout: 60_000 }, async () => {
  const server = http.createServer(async (request, response) => {
    try {
      const pathname = new URL(request.url, "http://127.0.0.1").pathname;
      if (!routes.has(pathname)) {
        response.writeHead(404).end();
        return;
      }
      const bytes = await readFile(path.join(root, "output", "browser", pathname));
      response.writeHead(200, { "Content-Type": routes.get(pathname) }).end(bytes);
    } catch (error) {
      response.writeHead(500).end(String(error));
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  let browser;
  try {
    browser = await playwright[engine].launch({
      headless: true,
      ...(process.env.METIS_TEST_BROWSER_EXECUTABLE
        ? { executablePath: process.env.METIS_TEST_BROWSER_EXECUTABLE } : {}),
    });
    const page = await browser.newPage({ viewport: { width: 1100, height: 820 } });
    page.setDefaultTimeout(10_000);
    const errors = [];
    page.on("pageerror", error => errors.push(String(error)));
    await page.goto(`http://127.0.0.1:${server.address().port}/gallery.html`);
    await page.locator('#selection-app[data-selection-listener-count="1"]').waitFor();
    assert.equal(await page.locator("#fixture-notice").textContent(),
      "Demonstration fixtures only. No clinical study is loaded.");
    assert.equal(await page.locator("img, canvas").count(), 0, "no image-rendering claim");
    const states = [];
    const verify = async (active, assignments) => {
      const actual = await snapshot(page);
      checkState(actual, active, assignments);
      assert.equal(await page.locator("#selection-error").textContent(), "");
      assert.equal(await page.locator("#selection-host-error").textContent(), "");
      states.push(actual);
    };
    await verify(0, ["", "", "", ""]);
    const desktopBoxes = await Promise.all([0, 1, 2, 3].map(index =>
      page.locator(`#viewport-${index}`).boundingBox()));
    assert.equal(desktopBoxes[0].x, desktopBoxes[2].x);
    assert.equal(desktopBoxes[1].x, desktopBoxes[3].x);
    assert.equal(desktopBoxes[0].y, desktopBoxes[1].y);
    assert.equal(desktopBoxes[2].y, desktopBoxes[3].y);
    assert.ok(desktopBoxes[0].x < desktopBoxes[1].x);
    assert.ok(desktopBoxes[0].y < desktopBoxes[2].y);
    await page.locator("#series-fixture-ct strong").click();
    await verify(0, ["fixture-ct", "", "", ""]);
    await page.locator("#viewport-2 > span:first-child").click();
    await verify(2, ["fixture-ct", "", "", ""]);
    await page.locator("#series-fixture-mr").click();
    await verify(2, ["fixture-ct", "", "fixture-mr", ""]);
    await page.locator("#series-fixture-secondary").focus();
    await page.keyboard.press("Enter");
    await verify(2, ["fixture-ct", "", "fixture-secondary", ""]);
    assert.equal(await page.evaluate(() => document.activeElement.id), "series-fixture-secondary");
    await page.locator("#viewport-0").focus();
    await page.keyboard.press("Space");
    await verify(0, ["fixture-ct", "", "fixture-secondary", ""]);
    assert.equal(await page.evaluate(() => document.activeElement.id), "viewport-0");
    await page.locator("#viewport-3").click();
    await verify(3, ["fixture-ct", "", "fixture-secondary", ""]);
    await page.locator("#series-fixture-mr").focus();
    await page.keyboard.press("Space");
    await verify(3, ["fixture-ct", "", "fixture-secondary", "fixture-mr"]);
    await page.locator("#series-fixture-mr").click();
    await verify(3, ["fixture-ct", "", "fixture-secondary", "fixture-mr"]);
    await page.locator("#viewport-1").click();
    await verify(1, ["fixture-ct", "", "fixture-secondary", "fixture-mr"]);

    // Adversarial DOM ids pass through the same delegated Rust listener.
    for (const [id, malformed, expected] of [
      ["viewport-1", "viewport-4", "Viewport index 4 is outside the 4-viewport layout"],
      ["viewport-1", "viewport--1", "Malformed viewport identifier"],
      ["series-fixture-ct", "series-unknown", "Unknown selection control"],
    ]) {
      const before = await snapshot(page);
      const control = page.locator(`#${id}`);
      await control.evaluate((element, changed) => { element.id = changed; }, malformed);
      await page.locator(`#${malformed}`).click();
      assert.equal(await page.locator("#selection-error").textContent(), `Selection rejected: ${expected}`);
      await page.locator(`#${malformed}`).evaluate((element, original) => { element.id = original; }, id);
      assert.deepEqual(await snapshot(page), before, "rejection preserves every assignment");
    }
    await page.locator("#series-fixture-ct").click();
    await verify(1, ["fixture-ct", "fixture-ct", "fixture-secondary", "fixture-mr"]);
    await mkdir(output, { recursive: true });
    await page.screenshot({ path: path.join(output, "assigned.png"), fullPage: true });

    await page.setViewportSize({ width: 360, height: 800 });
    await verify(1, ["fixture-ct", "fixture-ct", "fixture-secondary", "fixture-mr"]);
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
      "narrow viewport has no horizontal overflow");
    for (const button of await page.locator("#selection-app button").all()) {
      assert.ok((await button.boundingBox()).height >= 44, "control retains its declared hit target");
    }
    assert.equal(await page.locator("h1").count(), 1);
    const narrowBoxes = await Promise.all([0, 1, 2, 3].map(index =>
      page.locator(`#viewport-${index}`).boundingBox()));
    for (let index = 1; index < narrowBoxes.length; index++) {
      assert.equal(narrowBoxes[index].x, narrowBoxes[0].x);
      assert.equal(narrowBoxes[index].width, narrowBoxes[0].width);
      assert.ok(narrowBoxes[index].y > narrowBoxes[index - 1].y);
    }
    // Capture actual viewport-sized frames rather than a resized full-page surface.
    await page.screenshot({ path: path.join(output, "narrow.png") });
    await page.locator("#viewport-1").scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, "narrow-active.png") });
    await page.locator("#viewport-3").scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, "narrow-last.png") });

    for (let cycle = 0; cycle < 3; cycle++) {
      await page.evaluate(async () => {
        const { default: init } = await import("./consumer/series_selection.js");
        (await init()).series_selection_stop();
      });
      assert.equal(await page.locator("#selection-app").getAttribute("data-selection-listener-count"), "0");
      assert.equal(await page.locator("#selection-app button").count(), 0);
      assert.equal(await page.locator("#selection-app").getAttribute("data-selected-series"), "");
      await page.evaluate(() => {
        const button = document.createElement("button");
        button.id = "series-fixture-ct";
        button.setAttribute("data-selection-control", "");
        button.textContent = "Stopped control";
        document.getElementById("selection-app").append(button);
      });
      await page.locator("#series-fixture-ct").click();
      assert.equal(await page.locator("#selection-host-error").textContent(), "",
        "stop detaches the actual listener from the retained root");
      assert.equal(await page.locator("#selection-app").getAttribute("data-selected-series"), "");
      await page.evaluate(async () => {
        const { default: init } = await import("./consumer/series_selection.js");
        (await init()).series_selection_start();
        (await init()).series_selection_start();
      });
      await verify(0, ["", "", "", ""]);
      await page.evaluate(() => {
        globalThis.selectionMutations = 0;
        globalThis.selectionObserver = new MutationObserver(records => {
          globalThis.selectionMutations += records.length;
        });
        globalThis.selectionObserver.observe(document.getElementById("selection-summary"), { childList: true });
      });
      await page.locator("#series-fixture-secondary").click();
      await verify(0, ["fixture-secondary", "", "", ""]);
      assert.equal(await page.evaluate(() => {
        globalThis.selectionObserver.disconnect();
        const mutations = globalThis.selectionMutations;
        delete globalThis.selectionObserver;
        delete globalThis.selectionMutations;
        return mutations;
      }), 1, "repeated start retains exactly one active render callback");
    }
    assert.deepEqual(errors, [], "no uncaught browser error");
    const basis = {};
    for (const file of [
      "Cargo.toml", "Cargo.lock", "crates/metis-web/Cargo.toml", "crates/metis-web/examples/series_selection.rs",
      "crates/metis-web/examples/series_selection/model.rs",
      "crates/metis-web/examples/series_selection/browser.rs",
      "crates/metis-web/examples/series_selection/presentation.rs",
      "crates/metis-web/examples/series_selection/tests.rs",
      "crates/metis-web/examples/series_selection/frontend/gallery.html",
      "crates/metis-web/examples/series_selection/frontend/gallery.js",
      "crates/metis-web/examples/series_selection/frontend/gallery.css",
      "scripts/browser_series_selection.py", "scripts/browser_series_selection.mjs",
      "output/browser/consumer/series_selection_bg.wasm",
    ]) {
      basis[file] = createHash("sha256").update(await readFile(path.join(root, file))).digest("hex");
    }
    await writeFile(path.join(output, "report.json"), JSON.stringify({
      engine, version: browser.version(), states, source_sha256: basis,
      revision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8", timeout: 10_000 }).trim(),
      capabilities: { selection: true, dicom_loading: false, decoding: false, image_planes: false, mpr: false },
    }, null, 2) + "\n");
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
});
