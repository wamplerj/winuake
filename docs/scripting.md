# Scripting Winuake

Winuake exposes Yakuake's scripting methods (the `Q_SCRIPTABLE` D-Bus API) over a named pipe, and `winuake-cli` talks to it.

## Command line

```
winuake-cli <method> [argument ...]
winuake-cli toggle                    # toggleWindowState
winuake-cli add-session               # addSession
winuake-cli run echo hello            # runCommand "echo hello"
```

Results print like `qdbus`: bare text or numbers, nothing for methods without a result. Exit code 1 with a message on errors or when Winuake is not running, 2 for usage mistakes. Quote arguments that contain spaces when a method has more than one argument:

```
winuake-cli runCommandInTerminal 3 "git status"
winuake-cli setTabTitle 0 "build server"
```

## From `qdbus` to `winuake-cli`

| Yakuake | Winuake |
|---|---|
| `qdbus org.kde.yakuake /yakuake/sessions org.kde.yakuake.addSession` | `winuake-cli addSession` |
| `qdbus org.kde.yakuake /yakuake/sessions org.kde.yakuake.splitSessionLeftRight 0` | `winuake-cli splitSessionLeftRight 0` |
| `qdbus org.kde.yakuake /yakuake/sessions org.kde.yakuake.runCommandInTerminal 1 "ls"` | `winuake-cli runCommandInTerminal 1 ls` |
| `qdbus org.kde.yakuake /yakuake/tabs org.kde.yakuake.setTabTitle 0 build` | `winuake-cli setTabTitle 0 build` |
| `qdbus org.kde.yakuake /yakuake/MainWindow org.kde.yakuake.toggleWindowState` | `winuake-cli toggleWindowState` |

Method names, arguments and results are the same. Differences: `sessionIdList` and `terminalIdList` come back sorted; `runCommand` sends Enter as a carriage return; `removeSession` on a session locked with `setSessionClosable false` is an error (Yakuake would show a dialog); `-1` as a session id means the active session wherever a session id is accepted.

## PowerShell

```powershell
$cli = "winuake-cli"
$session = & $cli addSession                     # new tab, prints its id
$pane = & $cli splitSessionLeftRight $session    # id of the new terminal
& $cli runCommandInTerminal $pane "npm run watch"
& $cli setTabTitle $session "watch"
& $cli setSessionMonitorSilenceEnabled $session true
```

## Protocol

JSON-RPC 2.0, one request per line, one reply per line, positional `params`, over `\\.\pipe\winuake-<user name>`. Only the current user and SYSTEM can open the pipe; the first pipe instance is created exclusively so another process cannot take the name.

```
{"jsonrpc":"2.0","id":1,"method":"splitSessionLeftRight","params":[0]}
{"jsonrpc":"2.0","id":1,"result":1}
```

Errors use JSON-RPC codes: -32700 not JSON, -32600 not a request, -32601 unknown method, -32602 bad parameters, -32000 refused (locked session, UI busy). Requests without an `id` are notifications and get no reply.

Scripting can run commands in your shells; the log records only the length of scripted text.
