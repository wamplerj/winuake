#!/bin/bash
# usage: vm-type.sh "text" [enter]   -- types lowercase letters, digits, space, '-' into the VM, one key per call
text="$1"
for ((i=0; i<${#text}; i++)); do
  c="${text:$i:1}"
  case "$c" in
    [a-z]) key="KEY_${c^^}";;
    [A-Z]) key="KEY_${c}"; virsh send-key "${VM_DOMAIN:?set VM_DOMAIN}" KEY_LEFTSHIFT "$key" >/dev/null; continue;;
    [0-9]) key="KEY_$c";;
    ' ') key=KEY_SPACE;;
    '-') key=KEY_MINUS;;
    '.') key=KEY_DOT;;
    *) echo "unsupported char $c" >&2; continue;;
  esac
  virsh send-key "${VM_DOMAIN:?set VM_DOMAIN}" "$key" >/dev/null
done
[ "$2" = "enter" ] && virsh send-key "${VM_DOMAIN:?set VM_DOMAIN}" KEY_ENTER >/dev/null
true
