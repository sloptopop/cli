// Test cases for sloptopop.tls.js-reject-unauthorized (semgrep --test format).
import https from "node:https";

// ruleid: sloptopop.tls.js-reject-unauthorized
const agent = new https.Agent({ keepAlive: true, rejectUnauthorized: false });

// ruleid: sloptopop.tls.js-reject-unauthorized
const pool = new Pool({ connectionString, ssl: { rejectUnauthorized: false } });

const options: https.RequestOptions = {};
// ruleid: sloptopop.tls.js-reject-unauthorized
options.rejectUnauthorized = false;

// ruleid: sloptopop.tls.js-reject-unauthorized
process.env.NODE_TLS_REJECT_UNAUTHORIZED = "0";

// ok: sloptopop.tls.js-reject-unauthorized
const strict = new https.Agent({ rejectUnauthorized: true, ca });
