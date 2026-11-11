# Phase 8 output — Scripting IPC + CLI

## Done

- **8a** `winuake-core`: id-addressed workspace operations (`split_by_id`, `remove_terminal`, `grow_terminal`, `set_switch`, `set_tab_title`) with tests. `winuake-ipc` (Linux-tested, 33 tests): JSON-RPC protocol and error codes, `ScriptSession` mapping every Yakuake scripting method onto a real `Workspace` (all 48 methods, `-1` conventions, loose argument types), client helper and argument/result formatting.
- **8b** App: pipe server on `\\.\pipe\winuake-<user>` (user and SYSTEM only via SDDL built from the user's SID, exclusive first instance, one thread per connection, requests run on the UI thread through the dispatcher queue and use the same effect executor as shortcuts). `winuake-cli` with `toggle`, `add-session`, `run` shortcuts. `docs/scripting.md` maps `qdbus` calls and shows PowerShell use.

## Verified on the Win11 VM

From `winuake-cli` over SSH: `sessionIdList`, `addSession`, `splitSessionLeftRight`, `terminalIdList` (ids as expected), an unknown method (error message, exit 1), `toggle` showing the window, `setTabTitle`/`tabTitle`, `addSessionQuad`, monitor switch set and read back, `removeSession`, and `run echo scripted-hello` typing into the active pane and printing its output.

## Not done / known gaps

- Pipe name uses the user name, not the SID (the ACL uses the SID).
- `removeSession` on a locked session returns an error instead of asking. Scripted splits do not inherit the source terminal's directory (same gap as interactive ones).
- No timeout on a stuck client other than closing; the UI-thread wait is 15 s. One request line limit 1 MB.
- No tests of the pipe server itself (Windows only, exercised by hand); `winuake-cli` is covered by argument tests only.
