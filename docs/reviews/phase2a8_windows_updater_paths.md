# Windows updater path review

Windows CI at 8f1e027 exposed rejection of native separators in the updater's
portable-file whitelist. The fix at 25abd86 distinguishes native filesystem
components from raw ZIP names. Known nested stage directories use the same
portable component representation during owned cleanup.

An independent read-only reviewer found no blocker. Traversal, prefixes,
Windows devices, colon/ADS names and symlinks remain refused; raw ZIP backslashes
are rejected before enclosed-path processing. Stage cleanup still verifies
ownership before any removal. Positive native-join/nested-cleanup tests and raw
archive rejection cases were added to existing tests. Eleven Linux updater tests
and seven Windows GNU updater tests under Wine passed; targeted Clippy passed.
