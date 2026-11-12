# Releasing Winuake

Everything is built by `cargo xtask package` on a Windows machine (also what `.github/workflows/release.yml` runs on a version tag).

## One-time setup

- Rust with `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` (`rustup target add aarch64-pc-windows-msvc`), Visual Studio Build Tools with the **ARM64** component, the Windows SDK (`makeappx`, `signtool`), Inno Setup 6 (`winget install JRSoftware.InnoSetup`).
- `cargo xtask gen-bindings` once (downloads the Windows App SDK packages).

## Signing

Without a certificate, packages are built but only the MSIX is signed, with a throwaway self-signed certificate (`dist/<version>/winuake-dev.cer`, import it into *Trusted People* to test the MSIX). That is for testing only.

For a public release use a code-signing certificate whose subject equals the MSIX publisher in `xtask/src/package.rs` (`MSIX_PUBLISHER`; change that constant to your certificate's subject first):

```
set WINUAKE_SIGN_PFX=C:\path\to\certificate.pfx
set WINUAKE_SIGN_PASSWORD=...
cargo xtask package --version 1.0.0 --repo OWNER/NAME
```

The executables, the installer and the MSIX are then signed with SHA-256 and an RFC 3161 timestamp (`WINUAKE_TIMESTAMP_URL`, default DigiCert). In CI store the PFX as a secret, write it to a file in a step and pass its path.

## Steps

1. Bump `version` in the root `Cargo.toml` and the manifest version in `crates/winuake/winuake.manifest`.
2. Update `docs/release-notes-1.0.md` (rename per version), `docs/parity.md`, `UPSTREAM.md` (`cargo xtask upstream-diff` shows unreviewed Yakuake commits).
3. `cargo xtask package --version X.Y.Z --repo OWNER/NAME`. Output in `dist/X.Y.Z/`: zips, installers, MSIX, `SHA256SUMS.txt`, `winget/` manifests, for x64 and ARM64.
4. Test an installer and the MSIX on x64 (and on ARM64 hardware if you have it).
5. Tag `vX.Y.Z` and push: the release workflow attaches the files to a GitHub release.
6. winget: copy `dist/X.Y.Z/winget/*` into a fork of `microsoft/winget-pkgs` under `manifests/w/Winuake/Winuake/X.Y.Z/` and open a pull request (or use `wingetcreate`). The installer manifest points at the release assets and carries their hashes.
