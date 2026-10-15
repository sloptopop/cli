// Test cases for sloptopop.tls.go-insecure-skip-verify (semgrep --test format).
package client

import "crypto/tls"

func configs() []*tls.Config {
	// ruleid: sloptopop.tls.go-insecure-skip-verify
	a := &tls.Config{MinVersion: tls.VersionTLS12, InsecureSkipVerify: true}

	b := &tls.Config{}
	// ruleid: sloptopop.tls.go-insecure-skip-verify
	b.InsecureSkipVerify = true

	// ok: sloptopop.tls.go-insecure-skip-verify
	c := &tls.Config{MinVersion: tls.VersionTLS12, InsecureSkipVerify: false}

	// ok: sloptopop.tls.go-insecure-skip-verify
	d := &tls.Config{MinVersion: tls.VersionTLS12, RootCAs: pool}
	return []*tls.Config{a, b, c, d}
}
