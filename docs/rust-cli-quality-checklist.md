# rust cli quality checklist

CLI projects should provide clear usage on invalid arguments and return non-zero on failure. Include file paths and operations in I/O errors. Avoid panics for expected user mistakes. Use temporary directories in filesystem tests and cover empty input, Unicode, malformed input, and large input. Run formatting, clippy, and tests. Keep dependencies minimal and review behavior before recommending elevated privileges.

## Stable errors and exit codes

Use a non-zero exit code for invalid arguments, missing required files, and failed operations. Keep usage errors distinguishable from runtime I/O failures where practical, and write diagnostics to stderr so stdout remains usable for machine-readable output. Error messages should include the operation and path without exposing environment variables, credentials, or full file contents. Regression tests should assert both the exit code and the presence of a useful diagnostic.

## Signals and interrupted operations

For commands that write state, test interruption before the final rename or commit step and verify the previous valid state remains readable. If the CLI handles termination signals, document which operations are safe to interrupt and whether cleanup is best-effort. Avoid claiming success when a write was interrupted, and return a non-zero exit code when the requested operation did not complete. Keep signal tests isolated from the developer's real configuration directory.
