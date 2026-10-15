# Test cases for sloptopop.tls.python-verify-false (semgrep --test format).
import ssl

import httpx
import requests

# ruleid: sloptopop.tls.python-verify-false
response = requests.get(url, timeout=10, verify=False)

# ruleid: sloptopop.tls.python-verify-false
client = httpx.Client(verify=False)

# ruleid: sloptopop.tls.python-verify-false
context = ssl._create_unverified_context()

# ok: sloptopop.tls.python-verify-false
response = requests.get(url, timeout=10, verify="/etc/ssl/certs/internal-ca.pem")

# ok: sloptopop.tls.python-verify-false
response = requests.get(url, timeout=10)
