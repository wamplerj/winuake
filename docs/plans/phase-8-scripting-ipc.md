# Phase 8 — Scripting IPC + CLI

Parent: `winuake-port.md`. Ports the `Q_SCRIPTABLE` D-Bus interface of `sessionstack.h`, `tabbar.h`, `mainwindow.h` (`org.kde.yakuake` `/yakuake/sessions`, `/yakuake/tabs`, `/yakuake/MainWindow`).

## Behavior to port

Every scriptable method keeps its Yakuake name, arguments and result so scripts translate one to one: `addSession`, `addSessionTwoHorizontal/Vertical/Quad`, `raiseSession`, `removeSession`, `removeTerminal`, `splitSession*`, `splitTerminal*`, `tryGrowTerminal*` (default 10 px), `activeSessionId`, `activeTerminalId`, `sessionIdList`, `terminalIdList`, `terminalIdsForSessionId`, `sessionIdForTerminalId`, `runCommand`, `runCommandInTerminal`, the closable / keyboard-input / monitor-activity / monitor-silence families (session and terminal level, `hasTerminalsWith...`), `tabTitle`, `setTabTitle`, `sessionAtTab`, `toggleWindowState`. Failed lookups give `-1`, `false`, `""` like Yakuake. A session id of `-1` means the active session where Yakuake accepts it.

## Design

- Transport: named pipe `\\.\pipe\winuake-<user name>` (byte mode, one request line, one response line), JSON-RPC 2.0 with positional `params`. Access limited to the creating user and SYSTEM by SDDL. Deviation: the plan said the user SID; the user name is used in the pipe name and the ACL does the protection.
- `winuake-ipc` (pure, Linux-tested): protocol types and error codes, argument decoding, `ScriptSession` that maps method calls onto `winuake_core::Workspace` and returns the workspace effects, `ScriptHost` trait for window toggle, text input and pane geometry, client helper over any `Read + Write`. Tests run the dispatcher against a real `Workspace` with a fake host.
- App: pipe server thread; each request is handed to the UI thread (DispatcherQueue), applied with the same effect executor as shortcuts, and the reply returned to the connection. Scripted `removeSession` on a locked session is refused with an error instead of a dialog.
- `winuake-cli`: `winuake-cli <method> [args]` prints the result like `qdbus`; shortcuts `toggle`, `add-session`, `run <command>`; exit code 1 and a message on errors or when Winuake is not running. Docs map `qdbus org.kde.yakuake ...` calls to the CLI and show PowerShell examples.
- `runCommand` sends the text plus Enter (CR, the console's Enter) to the terminal.
- Security: scripting can run commands in the user's shells, so the pipe is user-only, and `runCommand*` logs lengths only, never the text.

## Chunks

- **8a** Core id-addressed operations; `winuake-ipc` protocol, dispatcher, client, tests.
- **8b** App pipe server and UI-thread dispatch; `winuake-cli`; docs and parity.

## Verification

Unit tests for protocol errors, every method family, `-1` conventions. On the VM: start the app, drive it from `winuake-cli` over SSH/scheduled task: add sessions, split, run a command and see its output, rename a tab, list ids, toggle the window.
