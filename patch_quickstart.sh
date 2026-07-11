#!/bin/bash
cat << 'INNER_EOF' >> QUICKSTART.md

### Presidio redaction

When you enable Presidio with `prodex presidio enable` and use Super mode (e.g., `prodex s`), Prodex starts a dedicated runtime proxy that redacts sensitive information. Prodex now supports multi-language Presidio redaction.

The runtime uses `presidio.toml` endpoints and language configuration when available, falling back to `http://localhost:5002` and `http://localhost:5001` for Analyzer/Anonymizer URLs, and English (`en`) for language if not specified. It honors `fail_mode = "open"` or `"closed"`.

Example `presidio.toml` for multi-language (English and Indonesian) auto-detection:
```toml
enabled = true
analyzer_url = "http://localhost:5002"
anonymizer_url = "http://localhost:5001"
language_mode = "auto"
languages = ["en", "id"]
fail_mode = "open"
```

Note that the default Presidio Docker images typically support English (`en`). For other languages like Indonesian (`id`), you might need a custom Presidio Analyzer with appropriate recognizers and models.
INNER_EOF
