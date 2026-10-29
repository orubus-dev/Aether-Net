// AetherNet sites — serve .ae site packages from the registry.
//
// A site on AetherNet is a content-addressed package published under
//   registry/sites/<handle>/index.html, assets, ...
// hosted by GitHub (the "VPS of sorts") and mirrored by mesh relays that
// cache the packages. This endpoint is the internet-assisted view: any
// browser can open https://<assist>/api/sites?name=genesis.me.ae and read
// the site, while AetherNet-native clients fetch the same bytes over mesh
// paths. The domain is cryptographic: resolve the name to its registry
// record first, and the pubkey fingerprint pins what the site must be
// signed with once site signing lands (roadmap M2).
//
// GET  /api/sites?name=<handle or domain>&path=/      → {content(base64), type}
// GET  /api/sites?name=<handle or domain>&mode=list   → {files:[...]}
// POST /api/sites {name, files:[{path, content(base64)}]} → atomic publish
//   (single git commit; requires the handle to be claimed, so only the
//    owner of the identity can publish to their domain once site signing
//    lands — for now the claim itself gates the namespace)

import {
  rawGet,
  ghList,
  ghRef,
  ghCreateBlob,
  ghCreateTree,
  ghCreateCommit,
  ghUpdateRef,
  resolveHandleRecord,
  requireToken,
} from "./_lib/gh.js";
import { route, send, readJson } from "./_lib/util.js";
import { HANDLE_RE } from "./_lib/derive.js";

const TYPES = {
  html: "text/html; charset=utf-8",
  css: "text/css; charset=utf-8",
  js: "text/javascript; charset=utf-8",
  mjs: "text/javascript; charset=utf-8",
  json: "application/json",
  svg: "image/svg+xml",
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  ico: "image/x-icon",
  woff2: "font/woff2",
  txt: "text/plain; charset=utf-8",
  wasm: "application/wasm",
};

export default route(async (req, res) => {
  const url = new URL(req.url, "http://x");
  const name = (url.searchParams.get("name") || "").trim().toLowerCase();
  let path = (url.searchParams.get("path") || "/").trim();
  const mode = url.searchParams.get("mode") || "get";

  // ---- POST: publish a site package as ONE atomic git commit ----
  if (req.method === "POST") {
    requireToken();
    const body = await readJson(req, 6 * 1024 * 1024);
    let handle = String(body.name || "").trim().toLowerCase();
    for (const suffix of [".me.ae", ".ae"]) {
      if (handle.endsWith(suffix)) handle = handle.slice(0, -suffix.length);
    }
    if (!HANDLE_RE.test(handle)) return send(res, 400, { error: "invalid site name" });
    const hrec = await resolveHandleRecord(handle);
    if (!hrec)
      return send(res, 403, {
        error: "handle not claimed — claim your .ae name before publishing",
      });
    const files = Array.isArray(body.files) ? body.files : [];
    if (!files.length) return send(res, 400, { error: "files[] required" });
    if (files.length > 64) return send(res, 413, { error: "max 64 files per publish" });
    let total = 0;
    for (const f of files) {
      if (typeof f.path !== "string" || f.path.includes("..") || f.path.startsWith("/"))
        return send(res, 400, { error: `bad path: ${String(f.path).slice(0, 40)}` });
      total += (f.content || "").length;
    }
    if (total > 4_500_000) return send(res, 413, { error: "site exceeds 4.5MB total" });

    const entries = [];
    for (const f of files) {
      const sha = await ghCreateBlob(String(f.content || ""));
      entries.push({ path: `registry/sites/${handle}/${f.path}`, sha });
    }
    if (!entries.some((e) => e.path.endsWith("index.html")))
      return send(res, 400, { error: "site needs an index.html" });
    const base = await ghRef("main");
    const tree = await ghCreateTree(base, entries);
    const commit = await ghCreateCommit(
      base,
      tree,
      `AetherNet site publish: ${handle} (${files.length} files)`
    );
    await ghUpdateRef("main", commit);
    return send(res, 201, {
      ok: true,
      domain: `${handle}${hrec && hrec.number && String(hrec.number).startsWith("500") ? ".me.ae" : ".ae"}`,
      files: files.length,
      commit,
      view: `/api/sites?name=${handle}&path=/`,
    });
  }

  if (req.method !== "GET") return send(res, 405, { error: "GET or POST" });

  let handle = name;
  for (const suffix of [".me.ae", ".ae"]) {
    if (handle.endsWith(suffix)) { handle = handle.slice(0, -suffix.length); break; }
  }
  if (!HANDLE_RE.test(handle)) return send(res, 400, { error: "invalid site name" });

  const base = `registry/sites/${handle}`;

  if (mode === "list") {
    const files = await ghList(base);
    if (!files) return send(res, 404, { error: "site not published", name: handle });
    return send(res, 200, {
      name: handle,
      files: files.map((f) => ({ path: "/" + f.name, size: f.size, sha: f.sha })),
    });
  }

  if (path === "/" || path === "") path = "/index.html";
  path = path.replace(/\\/g, "/").replace(/\/+/g, "/");
  if (path.includes("..")) return send(res, 400, { error: "bad path" });
  const rel = path.replace(/^\//, "");
  if (!rel) return send(res, 400, { error: "bad path" });

  const buf = await rawGet(`${base}/${rel}`);
  if (!buf) {
    // directory-style link "/about" → "/about/index.html"
    if (!/\.[a-z0-9]+$/i.test(rel)) {
      const alt = await rawGet(`${base}/${rel}/index.html`);
      if (alt) return site(res, `${rel}/index.html`, alt);
    }
    return send(res, 404, { error: "not found on this aether site", name: handle, path });
  }
  return site(res, rel, buf);
});

function site(res, rel, buf) {
  const ext = rel.split(".").pop().toLowerCase();
  send(res, 200, {
    ok: true,
    path: rel,
    type: TYPES[ext] || "application/octet-stream",
    bytes: buf.length,
    content: buf.toString("base64"),
  });
}
