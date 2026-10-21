# rust cli quality checklist

CLI projects should provide clear usage on invalid arguments and return non-zero on failure. Include file paths and operations in I/O errors. Avoid panics for expected user mistakes. Use temporary directories in filesystem tests and cover empty input, Unicode, malformed input, and large input. Run formatting, clippy, and tests. Keep dependencies minimal and review behavior before recommending elevated privileges.
