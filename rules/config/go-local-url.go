// Test cases for sloptopop.config.go-local-url (semgrep --test format).
package config

// ruleid: sloptopop.config.go-local-url
const apiURL = "http://localhost:8080/api"

func dial() {
	// ruleid: sloptopop.config.go-local-url
	connect("http://192.168.0.10:9000")

	// ok: sloptopop.config.go-local-url
	connect("https://api.example.com")
}
