import assert from "node:assert/strict";
import test from "node:test";

import { createTutorialProgressStore } from "./tutorial-progress.mjs";

function memoryStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, String(value)),
  };
}

function cookieDocument() {
  const values = new Map();
  return {
    get cookie() {
      return [...values].map(([key, value]) => `${key}=${value}`).join("; ");
    },
    set cookie(header) {
      const [pair] = String(header).split(";", 1);
      const separator = pair.indexOf("=");
      values.set(pair.slice(0, separator), pair.slice(separator + 1));
    },
  };
}

test("records completion and skip outcomes", () => {
  const completedStorage = memoryStorage();
  const completed = createTutorialProgressStore({
    storage: completedStorage,
    cookieDocument: null,
    now: () => new Date("2026-07-12T00:00:00Z"),
  });
  completed.finish("completed");
  assert.deepEqual(completed.currentRecord(), {
    schemaVersion: 1,
    outcome: "completed",
    finishedAt: "2026-07-12T00:00:00.000Z",
  });

  const skipped = createTutorialProgressStore({ storage: memoryStorage(), cookieDocument: null });
  skipped.finish("skipped");
  assert.equal(skipped.currentRecord().outcome, "skipped");
});

test("migrates legacy completion flags", () => {
  const storage = memoryStorage({ "burst-tutorial-local-v1": "complete" });
  const progress = createTutorialProgressStore({
    storage,
    cookieDocument: null,
    legacyKeys: ["burst-tutorial-local-v1"],
  });
  assert.equal(progress.hasFinished(), true);
  assert.equal(progress.currentRecord().outcome, "completed");
});

test("local review cookie survives a port-specific storage change", () => {
  const cookies = cookieDocument();
  const firstPort = createTutorialProgressStore({
    storage: memoryStorage(),
    cookieDocument: cookies,
    cookieName: "burst_tutorial_progress_v1",
  });
  firstPort.finish("skipped");

  const secondPort = createTutorialProgressStore({
    storage: memoryStorage(),
    cookieDocument: cookies,
    cookieName: "burst_tutorial_progress_v1",
  });
  assert.equal(secondPort.hasFinished(), true);
  assert.equal(secondPort.currentRecord().outcome, "skipped");
});

test("local review backfills a cookie from an existing structured record", () => {
  const storage = memoryStorage();
  createTutorialProgressStore({ storage, cookieDocument: null }).finish("completed");
  const cookies = cookieDocument();

  const currentPort = createTutorialProgressStore({
    storage,
    cookieDocument: cookies,
    cookieName: "burst_tutorial_progress_v1",
  });
  assert.equal(currentPort.hasFinished(), true);

  const nextPort = createTutorialProgressStore({
    storage: memoryStorage(),
    cookieDocument: cookies,
    cookieName: "burst_tutorial_progress_v1",
  });
  assert.equal(nextPort.hasFinished(), true);
});

test("ignores malformed records", () => {
  const progress = createTutorialProgressStore({
    storage: memoryStorage({ "burst-tutorial-progress-v1": "not-json" }),
    cookieDocument: null,
  });
  assert.equal(progress.hasFinished(), false);
});
