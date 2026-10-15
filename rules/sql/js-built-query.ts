// Test cases for sloptopop.sql.js-built-query (semgrep --test format).

export async function find(db: Pool, id: string, name: string) {
  // ruleid: sloptopop.sql.js-built-query
  await db.query("SELECT * FROM users WHERE id = " + id);

  // ruleid: sloptopop.sql.js-built-query
  await db.query(`SELECT * FROM users WHERE name = '${name}'`);

  // ruleid: sloptopop.sql.js-built-query
  await prisma.$queryRawUnsafe(`DELETE FROM sessions WHERE user_id = ${id}`);

  // ruleid: sloptopop.sql.js-built-query
  sqlite.prepare("update users set name = '" + name + "' where id = 1").run();

  // ok: sloptopop.sql.js-built-query
  await db.query("SELECT * FROM users WHERE id = $1", [id]);

  // ok: sloptopop.sql.js-built-query
  await db.query("SELECT * FROM users " + "WHERE active");

  // ok: sloptopop.sql.js-built-query
  await prisma.$queryRaw`SELECT * FROM users WHERE id = ${id}`;

  // ok: sloptopop.sql.js-built-query
  cache.get("user:" + id);
}
