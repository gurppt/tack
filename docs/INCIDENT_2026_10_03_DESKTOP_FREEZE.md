# Desktop freeze during Phase 1C — 2026-10-03

## Confirmed observations

The user reported a complete workstation freeze and restarted it. Previous boot:
`787bb6d13e3648569d919ada73f781f9`; last journal entry 11:18:52 CEST.
New boot: `397bcacaea1c43cbb9ca7b2365e974c9`, began 11:20:18 CEST.

At 11:13–11:15 native testing of two generated PNG images, a Tack prototype
reissued `Window::set_title` on every event-loop turn. A process snapshot recorded
70.3% CPU. This is an actual application defect: title changes can generate X11
property traffic and needless event wakeups. The implementation now caches the
last title and updates the window only when status changes; source warnings and
save status share one title path rather than competing writers.

Subsequent launches were not mapped/managed normally by Xfwm4: the X window existed,
was `IsUnMapped`, lacked `WM_STATE`, and map requests did not complete. Mouse
checks at that point were inconclusive. The owned PureRef probe and Tack processes
were terminated before the final attempted launch around 11:18:53. The pending
next diagnostic/tool call was interrupted by the user/restart. No successful
interaction benchmarks from that interval should be claimed as evidence.

## System evidence

Read-only checks after restart: previous kernel journal has no entries after
10:30; warning-priority journal has no entries from 11:05 to 11:20. No OOM,
hung-task/lockup, NVIDIA Xid or corresponding Tack core dump was recorded around
the incident. The historical libXi startup crash at October 2 03:17 is a different,
already documented incident. No implication that missing logs exclude a driver
or system lockup.

The old Xorg log has a zero-filled tail; it supplies no recorded fatal error at
the time of the freeze. Raw retained evidence is ignored local diagnostic data
under `benchmark-results/phase1c-incident/`; no full desktop/session logs are
published. The working tree survived the restart and passes `git diff --check`.

The kernel reports unrecoverable sector-208 reads on `/dev/sdc` at both previous
and current boot. This is a pre-existing secondary, unmounted NTFS disk; repository
and benchmark files are on `/dev/sdb3` (ext4). No evidence connects that disk fault
to this freeze. No disk repair or global system/driver change was attempted.

At 11:22 CEST the restarted host had about 27 GiB available RAM, zero swap use,
RTX 2060/NVIDIA 580.173.02 responding, and no surviving Tack/PureRef test process.

## Attribution and follow-up

Root cause **unconfirmed**. The title-event flood is a plausible contributor to
an unresponsive graphical session; it is not proof of the complete machine
freeze. X11/WM/driver involvement remains possible. Do not mark the incident
resolved merely because the host restarted or tests compile.

Continue with deterministic noninteractive tests, then one short native process
at a time. Check idle CPU/wakeup behavior and window mapping before interaction
stress. Keep bounded GPU submissions/work queues, close owned windows through
normal WM requests, retain any additional evidence, and carry this incident into
the Phase 1C report and recommendation.

## Post-restart verification

The corrected build completed a three-second owned two-image launch normally.
After startup, sampled process CPU ticks stayed at 53 through the idle interval
and RSS remained about 318 MiB: the title wakeup loop was not reproduced.
A subsequent native gesture/save pass, explicit GPU readback tests and seven
12-second manipulation scenarios all exited normally, one GPU process at a time.
The machine has not frozen again during these checks. This establishes recovery
of the tested application path, **not** the root cause or resolution of the full
workstation freeze. Local artifacts: `phase1c-incident/post-reboot-short.json`,
`phase1c-native-checks/` and `phase1c-interaction-final/` under ignored
`benchmark-results/`.

A final kernel-log check after all native tests again found zero NVIDIA Xid, OOM
or lockup/hung-task signals. The retained `/dev/sdc` read errors in this boot are
timestamped 11:20:18–11:20:29, during boot, rather than the later test interval.
Raw check: `benchmark-results/phase1c-incident/post-tests-kernel.log`.

## Phase 1D follow-up — 2026-10-04

Serialized native spatial/manipulation/persistence/regression runs completed
without another freeze. Ten-second ordinary idle observations show zero redraw
and source/file I/O, zero main/worker CPU ticks, and 0.3–0.5% aggregate CPU
(comparable 1C baseline 0.4%). Separate GDB stacks locate the residual periodic
threads in the NVIDIA driver; Tack's main loop waits in epoll and overview
workers wait on their channels. This does not establish the historical freeze
cause. No global driver/disk/desktop changes were made. Final kernel check
contains no new Xid/OOM/hung-task/lockup signal. See MISSION_1D_REPORT.md and
benchmarks/phase1d-idle.json for measured evidence and its limits.
