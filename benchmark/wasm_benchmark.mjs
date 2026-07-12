#!/usr/bin/env node

import { createServer } from "node:http";
import { readdir, readFile, stat, writeFile } from "node:fs/promises";
import { extname, isAbsolute, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const repositoryRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const argumentsMap = parseArguments(process.argv.slice(2));
const source = requireDirectory(argumentsMap.get("--source"), "--source");
const output = argumentsMap.get("--out");
const timeoutMs = Number(argumentsMap.get("--timeout-ms") || 10 * 60 * 1000);
const decodeConcurrency = argumentsMap.get("--decode-concurrency");
const decodeBackend = argumentsMap.get("--decode-backend");
const acceleration = argumentsMap.get("--acceleration");
const detector = argumentsMap.get("--detector");
const chrome = argumentsMap.get("--chrome")
  || process.env.CHROME_BIN
  || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
    if (pathname === "/favicon.ico") {
      response.writeHead(204).end();
      return;
    }
    const relative = pathname === "/" ? "web/dist/index.html" : pathname.replace(/^\/+/, "");
    const target = resolve(repositoryRoot, normalize(relative));
    if (!target.startsWith(`${repositoryRoot}/`)) throw new Error("path outside repository");
    const details = await stat(target);
    const file = details.isDirectory() ? join(target, "index.html") : target;
    response.writeHead(200, {
      "Content-Type": contentType(file),
      "Cross-Origin-Embedder-Policy": "require-corp",
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cache-Control": "no-store",
    });
    response.end(await readFile(file));
  } catch {
    response.writeHead(404).end("Not found");
  }
});

await new Promise(resolveListen => server.listen(0, "127.0.0.1", resolveListen));
const port = server.address().port;
let browser;
try {
  browser = await chromium.launch({ executablePath: chrome, headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const diagnosticsSession = await page.context().newCDPSession(page);
  await diagnosticsSession.send("Runtime.enable");
  diagnosticsSession.on("Runtime.exceptionThrown", event => {
    const details = event.exceptionDetails;
    process.stderr.write(
      `Browser exception: ${details.url || "unknown"}:${details.lineNumber + 1}:${details.columnNumber + 1} ${details.text}\n`,
    );
  });
  page.on("pageerror", error => process.stderr.write(`Page error: ${error.stack || error.message}\n`));
  page.on("console", message => {
    if (["error", "warning"].includes(message.type())) {
      process.stderr.write(`Browser ${message.type()}: ${message.text()}\n`);
    }
  });
  const query = new URLSearchParams({ lang: "en" });
  if (decodeConcurrency) query.set("decode-concurrency", decodeConcurrency);
  if (decodeBackend) query.set("decode-backend", decodeBackend);
  if (acceleration) query.set("acceleration", acceleration);
  if (detector) query.set("detector", detector);
  await page.goto(`http://127.0.0.1:${port}/web/dist/index.html?${query}`, { waitUntil: "networkidle" });
  const isolated = await page.evaluate(() => crossOriginIsolated);
  if (!isolated) throw new Error("benchmark page is not cross-origin isolated");
  const folderInput = page.locator("#folderInput");
  await folderInput.evaluate(input => input.removeAttribute("webkitdirectory"));
  await folderInput.setInputFiles(await collectFiles(source));
  try {
    await page.waitForFunction(() => document.documentElement.dataset.benchmarkComplete === "true", null, {
      timeout: timeoutMs,
    });
  } catch (error) {
    const diagnostics = await page.evaluate(() => ({
      initialization: document.documentElement.dataset.initialization || "ok",
      selectedFiles: document.querySelector("#folderInput")?.files?.length || 0,
      stage: document.querySelector("#stageLabel")?.textContent || "",
      detail: document.querySelector("#progressDetail")?.textContent || "",
      toast: document.querySelector("#toast")?.textContent || "",
    }));
    throw new Error(`${error.message}; page state: ${JSON.stringify(diagnostics)}`);
  }
  const benchmark = await page.evaluate(() => window.__burstBenchmark);
  const serialized = `${JSON.stringify(benchmark, null, 2)}\n`;
  if (output) await writeFile(resolve(output), serialized);
  process.stdout.write(serialized);
} finally {
  if (browser) await browser.close();
  await new Promise(resolveClose => server.close(resolveClose));
}

function parseArguments(values) {
  const result = new Map();
  for (let index = 0; index < values.length; index += 2) {
    if (!values[index]?.startsWith("--") || values[index + 1] === undefined) usage();
    result.set(values[index], values[index + 1]);
  }
  return result;
}

function requireDirectory(value, name) {
  if (!value) usage();
  return isAbsolute(value) ? value : resolve(value);
}

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(entries.map(entry => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? collectFiles(path) : [path];
  }));
  return nested.flat().sort();
}

function usage() {
  process.stderr.write("Usage: node benchmark/wasm_benchmark.mjs --source <folder> [--out <json>] [--chrome <executable>] [--timeout-ms N] [--decode-concurrency N] [--decode-backend image-bitmap] [--acceleration auto|webgpu|portable] [--detector auto|heuristic|ml|off]\n");
  process.exit(2);
}

function contentType(path) {
  return ({
    ".css": "text/css; charset=utf-8",
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".mjs": "text/javascript; charset=utf-8",
    ".json": "application/json; charset=utf-8",
    ".wasm": "application/wasm",
  })[extname(path).toLowerCase()] || "application/octet-stream";
}
