#!/usr/bin/env python3
"""usage: vm-mouse.py move X Y | down | up | click X Y | drag X1 Y1 X2 Y2 | wheel up|down N   (X/Y in 2560x1440 screen pixels)"""
import json, os, subprocess, sys

def qmp(events):
    cmd = json.dumps({"execute": "input-send-event", "arguments": {"events": events}})
    subprocess.run(["virsh", "qemu-monitor-command", os.environ["VM_DOMAIN"], cmd], check=True, capture_output=True)

def move(x, y):
    qmp([{"type": "abs", "data": {"axis": "x", "value": int(x / 2560 * 32767)}},
         {"type": "abs", "data": {"axis": "y", "value": int(y / 1440 * 32767)}}])

def button(name, down):
    qmp([{"type": "btn", "data": {"button": name, "down": down}}])

a = sys.argv[1:]
if a[0] == "move": move(float(a[1]), float(a[2]))
elif a[0] == "down": button("left", True)
elif a[0] == "up": button("left", False)
elif a[0] == "click":
    move(float(a[1]), float(a[2])); button("left", True); button("left", False)
elif a[0] == "rightclick":
    move(float(a[1]), float(a[2])); button("right", True); button("right", False)
elif a[0] == "drag":
    x1, y1, x2, y2 = map(float, a[1:5])
    move(x1, y1); button("left", True)
    steps = 8
    for i in range(1, steps + 1):
        move(x1 + (x2 - x1) * i / steps, y1 + (y2 - y1) * i / steps)
    button("left", False)
elif a[0] == "wheel":
    name = "wheel-up" if a[1] == "up" else "wheel-down"
    for _ in range(int(a[2])):
        button(name, True); button(name, False)
