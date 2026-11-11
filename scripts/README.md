# Windows VM dev scripts

Winuake is developed on Linux and verified on a Windows 11 KVM guest reached over SSH. These scripts sync the working tree, build, and run
tests/the app there.

Environment:

| Variable | Meaning |
|---|---|
| `VM_SSH` | `user@host` of the Windows guest (OpenSSH server, key auth) |
| `VM_DOMAIN` | libvirt domain name, for `virsh` screenshots and input injection |

Guest requirements: Rust (MSVC), Visual Studio Build Tools (C++), Git, Windows App SDK 2.6 runtime (installed by the Runtime NuGet MSIX or winget).

| Script | Purpose |
|---|---|
| `vm-test.sh <cargo test args>` | Sync, regenerate bindings, `cargo test` over SSH (non-interactive session: no DirectComposition) |
| `vm-test-interactive.sh <args>` | Same, but runs the tests in the logged-in console session through a scheduled task (needed for GPU tests) |
| `vm-run-app.sh` | Sync, build, launch the app in the console session, print its log |
| `vm-type.sh "text" [enter]` | Type into the guest with `virsh send-key` |
| `vm-mouse.py ...` | Absolute pointer moves, clicks, drags, wheel through QMP (needs a tablet device) |

Screenshots: `virsh screenshot $VM_DOMAIN out.png`.
