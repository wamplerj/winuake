# Phase 0 — output

## Done

- Scaffold (workspace, CI, parity matrix, upstream tracking) committed as `a9495d7`.
- `cargo xtask gen-bindings`: downloads Windows App SDK **2.6.5** NuGet packages (Foundation, InteractiveExperiences, WinUI) into `target/winappsdk/`, runs `windows-bindgen` 0.100 and writes two gitignored files into `crates/winuake/src/`:
  - `bindings.rs`: ergonomic WinUI 3 bindings (`Application::Start`, `Window`, `Grid`, `TabView`, `Button`, ...), member-level filters.
  - `composition.rs`: minimal-mode bindings, only to subclass `Application` (`IApplicationOverrides`, `IXamlMetadataProvider`).
- `cargo xtask fetch-runtime`: downloads the runtime MSIX packages (not needed on the dev VM; 2.6.5 was already installed).
- Spike (`crates/winuake/src/{bootstrap,spike}.rs`): unpackaged bootstrap (`MddBootstrapInitialize2`), composed `Application` subclass, `OnLaunched` creates a `Window` with `TabView`, `TextBlock`, `Button`, `SwapChainPanel`. `build.rs` copies the bootstrap DLL next to the exe.
- `swap_chain_clear.rs`: hardware D3D11 device, `CreateSwapChainForComposition`, hand-declared `ISwapChainPanelNative::SetSwapChain`, clear + `Present`. The `SwapChainPanel` inside the tab renders solid teal on the Win11 VM.
- Third generated file `graphics.rs` (D3D11/DXGI, full ergonomic bindings from the default Win32 metadata; bare type names work, `Namespace.Type` paths for Win32 do not).
- Verified on Win11 VM (build 26300): window renders with Fluent styling, process responsive.

## Measurements (debug build, idle, Win11 KVM VM)

| Metric | Value |
|---|---|
| Working set | 74-75 MB (no swap chain), 78 MB (with D3D11 swap chain) |
| Private bytes | 20-21 MB |

Release build not measured yet.

## Gate: GO for WinUI 3 from Rust

Findings that shape later phases:

1. **Metadata provider is mandatory.** XAML asks the app's `IXamlMetadataProvider` for built-in control types; the app must delegate to `Microsoft.UI.Xaml.XamlTypeInfo.XamlControlsXamlMetaDataProvider`.
2. **Merge `XamlControlsResources` inside `OnLaunched`** via `Application::Current()`. Touching `Application.Resources` inside the `Start` callback raises a fatal E_UNEXPECTED (`0xC000027B`). Without the merge, `Window::Activate` crashes too.
3. **Bindgen quirks (0.100):** member-level filters (`Type::{a, b}`, raw names like `put_Content`) are required to get methods; factory/statics interfaces referenced by class wrappers must be listed explicitly or the output does not compile; field names are PascalCase in ergonomic mode, snake_case in minimal mode. Composition only exists in minimal mode, hence the two output files.
4. **`SwapChainPanel` has no intrinsic size** and `TabViewItem` content does not stretch it: it rendered as zero-size (white) until given an explicit `Width`/`Height`. Real layout must set size or alignment.
5. D3D11/DXGI base interfaces (`IDXGIObject`, `IDXGISwapChain`, `IDXGIDeviceSubObject`) must be listed in the filter or their methods are missing from derived interfaces.
6. `windows` crate is no longer used; generated code depends on `windows-core`, `windows-link`, `windows-collections` 0.100.

## Not done

- Release-build memory, startup time.
- Swap chain does not resize with the panel (fixed 1280x720); resize handling belongs to Phase 2.
- Window is a normal resizable window. Drop-down behavior is Phase 3.

## Dev loop (Linux host, Win11 KVM guest)

Edit on Linux, `cargo check -p winuake --target x86_64-pc-windows-msvc` for fast feedback, then tar the tree to the VM over SSH, run `cargo xtask gen-bindings && cargo build`, launch via scheduled task with `/it` in the console session, screenshot with `virsh screenshot`.
