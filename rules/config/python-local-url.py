# Test cases for sloptopop.config.python-local-url (semgrep --test format).
import os

# ruleid: sloptopop.config.python-local-url
API_URL = "http://localhost:8000/api"

# ruleid: sloptopop.config.python-local-url
REDIS = "http://127.0.0.1:6379"

# ok: sloptopop.config.python-local-url
FROM_ENV = os.environ.get("API_URL", "http://localhost:8000/api")

# ok: sloptopop.config.python-local-url
FROM_GETENV = os.getenv("API_URL") or "http://localhost:8000/api"

# ok: sloptopop.config.python-local-url
PUBLIC = "https://api.example.com"
