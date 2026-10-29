// Shared HTTP helpers for AetherNet assist functions.
// Framework-agnostic (plain Node req/res) so the same handlers run on
// Vercel and inside sandbox-server.js (your own VPS).

export function cors(res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET,POST,PUT,DELETE,OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization");
  res.setHeader("Cache-Control", "no-store");
}

export function send(res, code, obj) {
  const body = JSON.stringify(obj, null, 2);
  res.statusCode = code;
  res.setHeader("Content-Type", "application/json; charset=utf-8");
  res.end(body);
}

export async function readJson(req, limitBytes = 4 * 1024 * 1024) {
  const chunks = [];
  let size = 0;
  for await (const c of req) {
    size += c.length;
    if (size > limitBytes) throw new HttpError(413, "payload too large");
    chunks.push(c);
  }
  if (!chunks.length) return {};
  try {
    return JSON.parse(Buffer.concat(chunks).toString("utf8"));
  } catch {
    throw new HttpError(400, "invalid JSON body");
  }
}

export class HttpError extends Error {
  constructor(status, message) {
    super(message);
    this.status = status;
  }
}

/** Wrap a handler with CORS + OPTIONS + error mapping. */
export function route(handler) {
  return async (req, res) => {
    cors(res);
    if (req.method === "OPTIONS") {
      res.statusCode = 204;
      return res.end();
    }
    try {
      await handler(req, res);
    } catch (e) {
      if (e instanceof HttpError) return send(res, e.status, { error: e.message });
      if (e && typeof e.status === "number")
        return send(res, e.status, { error: e.message });
      console.error(e);
      send(res, 500, { error: "internal", detail: String(e && e.message) });
    }
  };
}
