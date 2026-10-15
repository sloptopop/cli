# Test cases for sloptopop.sql.python-built-query (semgrep --test format).


def find(cursor, user_id, name):
    # ruleid: sloptopop.sql.python-built-query
    cursor.execute("SELECT * FROM users WHERE id = " + user_id)

    # ruleid: sloptopop.sql.python-built-query
    cursor.execute("SELECT * FROM users WHERE name = '%s'" % name)

    # ruleid: sloptopop.sql.python-built-query
    cursor.execute("DELETE FROM sessions WHERE user_id = {}".format(user_id))

    # ruleid: sloptopop.sql.python-built-query
    cursor.execute(f"UPDATE users SET name = '{name}' WHERE id = 1")

    # ok: sloptopop.sql.python-built-query
    cursor.execute("SELECT * FROM users WHERE id = %s", (user_id,))

    # ok: sloptopop.sql.python-built-query
    cursor.execute("SELECT * FROM users " + "WHERE active")

    # ok: sloptopop.sql.python-built-query
    log.info(f"select {user_id}")
