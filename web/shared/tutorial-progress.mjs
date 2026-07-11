const RECORD_SCHEMA_VERSION = 1;
const DEFAULT_STORAGE_KEY = "burst-tutorial-progress-v1";
const VALID_OUTCOMES = new Set(["completed", "skipped"]);
const COOKIE_MAX_AGE_SECONDS = 60 * 60 * 24 * 365 * 10;

function defaultStorage() {
  try { return globalThis.localStorage; } catch { return null; }
}

function defaultDocument() {
  try { return globalThis.document; } catch { return null; }
}

function parseRecord(value) {
  if (!value) return null;
  try {
    const record = JSON.parse(value);
    if (
      Number(record?.schemaVersion) >= 1
      && VALID_OUTCOMES.has(record?.outcome)
      && typeof record?.finishedAt === "string"
    ) {
      return record;
    }
  } catch {}
  return null;
}

function readStorage(storage, key) {
  try { return storage?.getItem(key) ?? null; } catch { return null; }
}

function writeStorage(storage, key, value) {
  try { storage?.setItem(key, value); } catch {}
}

function readCookie(cookieDocument, name) {
  if (!cookieDocument || !name) return null;
  try {
    const prefix = `${encodeURIComponent(name)}=`;
    const entry = String(cookieDocument.cookie || "")
      .split(";")
      .map(value => value.trim())
      .find(value => value.startsWith(prefix));
    return entry ? decodeURIComponent(entry.slice(prefix.length)) : null;
  } catch {
    return null;
  }
}

function writeCookie(cookieDocument, name, value) {
  if (!cookieDocument || !name) return;
  try {
    cookieDocument.cookie = `${encodeURIComponent(name)}=${encodeURIComponent(value)}; Max-Age=${COOKIE_MAX_AGE_SECONDS}; Path=/; SameSite=Lax`;
  } catch {}
}

export function createTutorialProgressStore(options = {}) {
  const hasOwn = key => Object.prototype.hasOwnProperty.call(options, key);
  const storage = hasOwn("storage") ? options.storage : defaultStorage();
  const cookieDocument = hasOwn("cookieDocument")
    ? options.cookieDocument
    : defaultDocument();
  const storageKey = options.storageKey || DEFAULT_STORAGE_KEY;
  const cookieName = options.cookieName || null;
  const legacyKeys = options.legacyKeys || [];
  const now = options.now || (() => new Date());

  function persist(record) {
    const encoded = JSON.stringify(record);
    writeStorage(storage, storageKey, encoded);
    writeCookie(cookieDocument, cookieName, encoded);
    return record;
  }

  function finish(outcome) {
    if (!VALID_OUTCOMES.has(outcome)) throw new TypeError(`Unsupported tutorial outcome: ${outcome}`);
    const finishedAt = now();
    return persist({
      schemaVersion: RECORD_SCHEMA_VERSION,
      outcome,
      finishedAt: finishedAt instanceof Date ? finishedAt.toISOString() : String(finishedAt),
    });
  }

  function currentRecord() {
    const stored = parseRecord(readStorage(storage, storageKey));
    if (stored) {
      writeCookie(cookieDocument, cookieName, JSON.stringify(stored));
      return stored;
    }

    const cookieRecord = parseRecord(readCookie(cookieDocument, cookieName));
    if (cookieRecord) return persist(cookieRecord);

    const migrated = legacyKeys.some(key => readStorage(storage, key) === "complete");
    return migrated ? finish("completed") : null;
  }

  return Object.freeze({
    currentRecord,
    finish,
    hasFinished: () => currentRecord() !== null,
  });
}
