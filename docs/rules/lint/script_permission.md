# Lint/ScriptPermission

Grant script file execute permission.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks if a file which has a shebang line as its first line is granted execute permission.

## Options

This rule has no options.

## Blind spots

A file's real executable permission bit can only be checked when the linted path is an actual file on disk; sources read from stdin or given a synthetic path are skipped, matching upstream's `@options.key?(:stdin)` guard.
