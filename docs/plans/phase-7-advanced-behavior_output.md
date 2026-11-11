# Phase 7 output — Advanced behavior

## Done

- **7a** Monitor for activity/silence: toggles reach each terminal's session; output raises one "Activity detected in monitored terminal in session "<tab>"." notification until the terminal gets focus again; silence after 10 s. Delivered as a tray balloon (a toast on Windows 11) and suppressed while the user is looking at that terminal in an open, focused window (`winuake-core::notifications`, tested). Blocked-input indicator: typing without modifiers into a locked terminal flashes `KeyboardInputBlockIndicatorColor/Opacity/Duration` over its pane (shared `flashes` timer).
- **7b** Terminal highlight on manual activation in split sessions (same flash mechanism); focus follows mouse; auto open (`winuake-core::drop_down::pointer_at_open_edge`, tested; timer at `PollInterval` while hidden); display layout changes noticed while open (checked about once a second) re-place the window.
- **7c** Translucency: Acrylic system backdrop plus a premultiplied-alpha terminal background in `BackgroundColor` at `BackgroundColorOpacity`; the window root is transparent. Settings changes apply live.

## Verified on the Win11 VM

Activity toast appeared with the right text after hiding the window (and was suppressed while visible); locked terminal flashed red and swallowed the key; highlight flash on clicking the other pane; typing after hovering the other pane went to it; pointer pushed to the top edge opened the hidden window; translucent window showed the blurred desktop tinted by the chosen color.

## Not done / known gaps

- Display hot-plug and reordering were not exercised (the VM has one display); the detection compares monitor layouts once a second while open and is untested live. Not a `WM_DISPLAYCHANGE` handler (message-only windows do not receive it).
- Notifications need the tray icon shown; with it hidden a warning is logged. Silence notifications fire only if output arrived earlier. Toast appearance is Windows' balloon style, not a branded toast (no AppUserModelID in the unpackaged app).
- Highlight color is fixed (Breeze blue). Translucency tints with Acrylic (blur), not a plain see-through window; the title and tab bars stay opaque.
- Flashes cover the pane at the time they start; a layout change during the flash leaves the rectangle where it was until it expires.
