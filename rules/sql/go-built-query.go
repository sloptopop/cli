// Test cases for sloptopop.sql.go-built-query (semgrep --test format).
package store

func find(ctx context.Context, db *sql.DB, id string, name string) {
	// ruleid: sloptopop.sql.go-built-query
	db.Query("SELECT * FROM users WHERE id = " + id)

	// ruleid: sloptopop.sql.go-built-query
	db.QueryRowContext(ctx, fmt.Sprintf("SELECT * FROM users WHERE name = '%s'", name))

	// ruleid: sloptopop.sql.go-built-query
	db.ExecContext(ctx, "DELETE FROM sessions WHERE user_id = "+id)

	// ok: sloptopop.sql.go-built-query
	db.QueryContext(ctx, "SELECT * FROM users WHERE id = $1", id)

	// ok: sloptopop.sql.go-built-query
	db.Query("SELECT * FROM users " + "WHERE active")

	// ok: sloptopop.sql.go-built-query
	cache.Get("user:" + id)
}
