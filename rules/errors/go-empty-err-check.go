// Test cases for sloptopop.errors.go-empty-err-check (semgrep --test format).
package orders

func save(o Order) error {
	err := db.Save(o)
	// ruleid: sloptopop.errors.go-empty-err-check
	if err != nil {
	}

	// ruleid: sloptopop.errors.go-empty-err-check
	if err := db.Flush(); err != nil {
		// ignored on purpose
	}

	// ruleid: sloptopop.errors.go-empty-err-check
	if _, err := db.Exec(q); err != nil {
	}

	writeErr := w.Close()
	// ruleid: sloptopop.errors.go-empty-err-check
	if writeErr != nil {
	}

	// ok: sloptopop.errors.go-empty-err-check
	if err != nil {
		return fmt.Errorf("save order: %w", err)
	}

	// ok: sloptopop.errors.go-empty-err-check
	if o.Next != nil {
	}
	return nil
}
