# Shared setup for the VM scripts. Source it; do not run it.
: "${VM_SSH:?set VM_SSH to user@host of the Windows guest}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VM_USER_HOME_WIN="C:\\Users\\${VM_SSH%@*}"
SSH_OPTIONS=(-o BatchMode=yes)
ssh_quiet() { ssh "${SSH_OPTIONS[@]}" "$VM_SSH" "$@" 2>&1 | grep -v -E 'WARNING|vulnerable|upgraded' || true; }

sync_tree() {
  local tarball
  tarball="$(mktemp)"
  (cd "$REPO_ROOT" && git ls-files -co --exclude-standard -z | tar --null -T - -cf "$tarball")
  scp -q "${SSH_OPTIONS[@]}" "$tarball" "$VM_SSH:winuake-src.tar" 2>/dev/null
  rm -f "$tarball"
  ssh_quiet 'if not exist src\winuake mkdir src\winuake & cd src\winuake & tar -xf ..\..\winuake-src.tar'
}
