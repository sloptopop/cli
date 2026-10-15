// Test cases for sloptopop.config.js-local-url (semgrep --test format).

// ruleid: sloptopop.config.js-local-url
const API_URL = "http://localhost:3000/api";

// ruleid: sloptopop.config.js-local-url
const ADMIN = 'http://127.0.0.1:8080';

// ruleid: sloptopop.config.js-local-url
const GATEWAY = "https://192.168.1.20/v1";

// ruleid: sloptopop.config.js-local-url
const QUEUE = "http://10.0.0.5:5672";

// ruleid: sloptopop.config.js-local-url
const INTERNAL = "http://172.16.4.2/health";

export async function ping() {
  // ruleid: sloptopop.config.js-local-url
  return fetch("http://localhost:54321/rest/v1/orders");
}

// ok: sloptopop.config.js-local-url
const FROM_ENV = process.env.API_URL ?? "http://localhost:3000/api";

// ok: sloptopop.config.js-local-url
const FROM_ENV_OR = process.env.API_URL || "http://localhost:3000/api";

// ok: sloptopop.config.js-local-url
const PUBLIC = "https://api.example.com/v1";

// ok: sloptopop.config.js-local-url
const NOT_PRIVATE = "http://172.32.0.1/";

// ok: sloptopop.config.js-local-url
const LOOKALIKE = "http://localhost.example.com/";

// ok: sloptopop.config.js-local-url
const TEXT = "start the server on localhost first";
