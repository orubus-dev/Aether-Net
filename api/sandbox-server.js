// sandbox-server.js — run the entire AetherNet assist layer on YOUR box.
//
// "We can turn your own sandbox into a VPS": this file mounts the exact
// same handlers the Vercel deployment uses, with the same env contract and
// the same REST surface. Point apps at http://your-box:8787 and they talk
// to your assist node; it still holds no keys and sees only ciphertext.
//
//   GITHUB_TOKEN=github_pat_… GITHUB_REPO=orubus-dev/Aether-Net \
//     node api/sandbox-server.js 8787
//
// Routes are resolved exactly like Vercel resolves files: /api/<name> →
// api/<name>.js default export.

import http from "node:http";
import { pathToFileURL } from "node:url"; // kept for parity

const PORT = Number(process.argv[2] || process.env.PORT || 8787);
const handlers = new Map();

for (const name of ["health", "resolve", "claim", "mail", "signal", "sites", "sweep"]) {
  const mod = await import(new URL(`./${name}.js`, import.meta.url).href);
  handlers.set(`/api/${name}`, mod.default);
}

const server = http.createServer(async (req, res) => {
  const path = new URL(req.url, "http://x").pathname.replace(/\/+$/, "") || "/";
  const handler = handlers.get(path);
  if (handler) return handler(req, res);

  // Static: serve the web client from ../web when present.
  if (req.method === "GET") {
    try {
      const fs = await import("node:fs/promises");
      const pathMod = await import("node:path");
      const rel = path === "/" ? "index.html" : path.replace(/^\/+/, "");
      const webRoot = pathMod.resolve(process.cwd(), "web");
      const file = pathMod.resolve(webRoot, pathMod.normalize(rel));
      if (!file.startsWith(webRoot)) throw new Error("nope");
      const data = await fs.readFile(file);
      const types = {
        html: "text/html; charset=utf-8",
        css: "text/css; charset=utf-8",
        js: "text/javascript; charset=utf-8",
        json: "application/json",
        svg: "image/svg+xml",
      };
      const ext = file.split(".").pop();
      res.writeHead(200, { "Content-Type": types[ext] || "application/octet-stream" });
      return res.end(data);
    } catch {
      res.writeHead(404, { "Content-Type": "application/json" });
      return res.end(JSON.stringify({ error: "not found", hint: "run from the repo root so web/ is served" }));
    }
  }
  res.writeHead(404, { "Content-Type": "application/json" });
  res.end(JSON.stringify({ error: "unknown route" }));
});

server.listen(PORT, () => {
  console.log(`AetherNet assist (sandbox VPS) listening on :${PORT}`);
  console.log(`  repo:   ${process.env.GITHUB_REPO || "orubus-dev/Aether-Net"}`);
  console.log(`  token:  ${process.env.GITHUB_TOKEN ? "configured" : "MISSING — writes will 503"}`);
  console.log(`  health: http://localhost:${PORT}/api/health`);
});
