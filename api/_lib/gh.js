// AetherNet assist configuration.
//
// The assist layer is stateless: every durable byte lives in the GitHub
// repository (the registry = storage of truth, mail/ = temporary encrypted
// envelopes). These env vars are the only configuration:
//
//   GITHUB_TOKEN  fine-grained token, contents:write on GITHUB_REPO
//                 (needed for writes; reads of public paths work without)
//   GITHUB_REPO   default "orubus-dev/Aether-Net"
//   MAIL_TTL_MAX  max envelope lifetime in seconds (default 7 days)

export const REPO = process.env.GITHUB_REPO || "orubus-dev/Aether-Net";
export const [OWNER, REPO_NAME] = REPO.split("/");
export const TOKEN = process.env.GITHUB_TOKEN || "";
export const MAIL_TTL_MAX = Number(process.env.MAIL_TTL_MAX || 7 * 24 * 3600);
export const MAIL_BOX_MAX = 48; // envelopes per box before new drops are refused
export const MAIL_BLOB_MAX = 1_500_000; // bytes of base64 ciphertext per envelope
export const API = "https://api.github.com";

const GH_HEADERS = () => ({
  Accept: "application/vnd.github+json",
  "X-GitHub-Api-Version": "2022-11-28",
  "User-Agent": "aethernet-assist/1.0",
  ...(TOKEN ? { Authorization: `Bearer ${TOKEN}` } : {}),
});

/** GET a file from the repo via the Contents API. Returns {json?, raw?, sha?} or null on 404. */
export async function ghGet(path) {
  const r = await fetch(`${API}/repos/${REPO}/contents/${encodePath(path)}`, {
    headers: GH_HEADERS(),
  });
  if (r.status === 404) return null;
  if (r.status === 403 && r.headers.get("x-ratelimit-remaining") === "0") {
    throw new Error("GitHub rate limit exhausted — retry later");
  }
  if (!r.ok) throw new Error(`ghGet ${path}: HTTP ${r.status}`);
  const j = await r.json();
  return { sha: j.sha, raw: Buffer.from(j.content || "", "base64") };
}

/** List a directory. Returns [{name, path, sha, size}] or null when missing. */
export async function ghList(dir) {
  const r = await fetch(`${API}/repos/${REPO}/contents/${encodePath(dir)}`, {
    headers: GH_HEADERS(),
  });
  if (r.status === 404) return null;
  if (!r.ok) throw new Error(`ghList ${dir}: HTTP ${r.status}`);
  const j = await r.json();
  return Array.isArray(j) ? j : null;
}

/** Create a file (atomic). Fails with {taken:true} when the path already exists. */
export async function ghCreate(path, contentB64, message) {
  const r = await fetch(`${API}/repos/${REPO}/contents/${encodePath(path)}`, {
    method: "PUT",
    headers: { ...GH_HEADERS(), "Content-Type": "application/json" },
    body: JSON.stringify({ message, content: contentB64 }),
  });
  if (r.status === 201) {
    const j = await r.json();
    return { ok: true, commit: j.commit && j.commit.sha, sha: j.content && j.content.sha };
  }
  if (r.status === 422) return { ok: false, taken: true };
  const text = await r.text();
  throw new Error(`ghCreate ${path}: HTTP ${r.status} ${text.slice(0, 200)}`);
}

/** Delete a file by its blob sha. */
export async function ghDelete(path, sha, message) {
  const r = await fetch(`${API}/repos/${REPO}/contents/${encodePath(path)}`, {
    method: "DELETE",
    headers: { ...GH_HEADERS(), "Content-Type": "application/json" },
    body: JSON.stringify({ message, sha }),
  });
  if (r.status === 200) return { ok: true };
  if (r.status === 404 || r.status === 422) return { ok: false, gone: true };
  throw new Error(`ghDelete ${path}: HTTP ${r.status}`);
}

/** Create or update a file (PUT with sha when it exists). */
export async function ghUpsert(path, contentB64, message) {
  const existing = await ghGet(path);
  const body = { message, content: contentB64 };
  if (existing && existing.sha) body.sha = existing.sha;
  const r = await fetch(`${API}/repos/${REPO}/contents/${encodePath(path)}`, {
    method: "PUT",
    headers: { ...GH_HEADERS(), "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (r.status === 200 || r.status === 201) {
    const j = await r.json();
    return { ok: true, commit: j.commit && j.commit.sha, sha: j.content && j.content.sha };
  }
  const text = await r.text();
  throw new Error(`ghUpsert ${path}: HTTP ${r.status} ${text.slice(0, 200)}`);
}

/** Raw read of a public file — no token, CDN cached. Returns Buffer or null. */
export async function rawGet(path, ref = "main") {
  const r = await fetch(
    `https://raw.githubusercontent.com/${REPO}/${ref}/${encodePath(path)}`,
    { headers: { "User-Agent": "aethernet-assist/1.0" } }
  );
  if (r.status === 404) return null;
  if (!r.ok) throw new Error(`rawGet ${path}: HTTP ${r.status}`);
  return Buffer.from(await r.arrayBuffer());
}

function encodePath(p) {
  return p.split("/").map(encodeURIComponent).join("/");
}

/* --- Git Data API (used by site publishing: one atomic commit) --- */

export async function ghRef(branch = "main") {
  const r = await fetch(`${API}/repos/${REPO}/git/ref/heads/${branch}`, {
    headers: GH_HEADERS(),
  });
  if (!r.ok) throw new Error(`ghRef: HTTP ${r.status}`);
  const j = await r.json();
  return j.object.sha;
}

export async function ghCreateBlob(contentB64) {
  const r = await fetch(`${API}/repos/${REPO}/git/blobs`, {
    method: "POST",
    headers: { ...GH_HEADERS(), "Content-Type": "application/json" },
    body: JSON.stringify({ content: contentB64, encoding: "base64" }),
  });
  if (!r.ok) throw new Error(`ghCreateBlob: HTTP ${r.status}`);
  const j = await r.json();
  return j.sha;
}

export async function ghCreateTree(baseTreeSha, entries) {
  const r = await fetch(`${API}/repos/${REPO}/git/trees`, {
    method: "POST",
    headers: { ...GH_HEADERS(), "Content-Type": "application/json" },
    body: JSON.stringify({
      base_tree: baseTreeSha,
      tree: entries.map((e) => ({
        path: e.path,
        mode: "100644",
        type: "blob",
        sha: e.sha,
      })),
    }),
  });
  if (!r.ok) throw new Error(`ghCreateTree: HTTP ${r.status}`);
  const j = await r.json();
  return j.sha;
}

export async function ghCreateCommit(baseCommitSha, treeSha, message) {
  const r = await fetch(`${API}/repos/${REPO}/git/commits`, {
    method: "POST",
    headers: { ...GH_HEADERS(), "Content-Type": "application/json" },
    body: JSON.stringify({ message, tree: treeSha, parents: [baseCommitSha] }),
  });
  if (!r.ok) throw new Error(`ghCreateCommit: HTTP ${r.status}`);
  const j = await r.json();
  return j.sha;
}

export async function ghUpdateRef(branch, commitSha) {
  const r = await fetch(`${API}/repos/${REPO}/git/refs/heads/${branch}`, {
    method: "PATCH",
    headers: { ...GH_HEADERS(), "Content-Type": "application/json" },
    body: JSON.stringify({ sha: commitSha, force: false }),
  });
  if (!r.ok) throw new Error(`ghUpdateRef: HTTP ${r.status}`);
  return { ok: true };
}

/** Resolve a handle/domain to its registry handle record, or null. */
export async function resolveHandleRecord(handle) {
  const buf = await rawGet(`registry/handles/${handle}.json`);
  return buf ? JSON.parse(buf.toString("utf8")) : null;
}

export function requireToken() {
  if (!TOKEN) {
    const e = new Error(
      "GITHUB_TOKEN not configured on this assist endpoint — writes are disabled"
    );
    e.status = 503;
    throw e;
  }
}
