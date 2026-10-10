# System crash during Phase 2A6 validation — 2026-10-10

The owner reported a whole-system crash. The machine rebooted at 12:11:29 CEST;
the previous ordinary journal ends at 12:09:50. EFI crash records named with
Unix timestamp 1791627013 (12:10:13 CEST) were archived under
`/var/lib/systemd/pstore/1791627013/{001,002}/dmesg.txt`. Those files require root;
their contents have not yet been read. A read-only copy was requested from the
owner. This is the key outstanding diagnostic evidence.

The accessible previous-boot kernel journal contains no recent OOM, NVIDIA Xid
or panic trace. No application core dump was found. Current NVIDIA inspection
reports RTX 2060, driver 580.173.02, 47°C, 425/6144 MiB occupied. This snapshot
cannot explain the earlier crash. The boot logs report unrecovered reads on
`/dev/sdc` (WDC WD10EARS, unmounted), also present at the previous boot startup.
The repository is on `/dev/sdb3` (Crucial MX500), a different disk. SMART needs
root and was unavailable. These storage errors are real but do not establish
that they caused this incident.

Implementation commit `80f7014` was already pushed. Git object verification found
no missing/corrupt objects. Final Linux/Windows binary hashes, 38 native UI checks
and six Windows Wine/decoder checks survived. The final three-client attempt has
16 passing checks on disk, but its receipt and two client reports became empty;
`/tmp` logs disappeared across reboot. That attempt is incomplete evidence and
will not replace the earlier valid receipt.

Recovery verifies retained JSON/binary artifacts and repeats concurrent checks
on explicitly selected software Vulkan. No repeated simultaneous hardware-GPU
clients are launched while the persisted trace is unavailable. The 2A6 exact
implementation commit passed Linux, Windows and dependency CI independently.
Physical artist acceptance remains pending.

Cause: **undetermined**. Do not assert an application, graphics-driver, disk or
memory cause without reading the persisted crash trace. No system setting,
driver, filesystem repair or user artwork was modified as a diagnostic action.
Compact facts are retained in `benchmarks/phase2a6/incident.json`.
