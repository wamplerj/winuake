//! `cargo xtask package [--arch x64|arm64|all] [--version X.Y.Z]`: release builds, portable zips, Inno Setup installers, MSIX packages,
//! SHA-256 checksums and a winget manifest, for x64 and ARM64. Runs on a Windows host; ARM64 is cross-compiled with the MSVC ARM64 tools.

use crate::winappsdk;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const PRODUCT: &str = "Winuake";
/// Where release assets live unless `--repo` says otherwise.
const DEFAULT_REPOSITORY: &str = "wamplerj/winuake";
const PUBLISHER: &str = "Winuake contributors";
const INNO_APP_ID: &str = "3B1B6D2E-7A4F-4C0E-9C1A-5E8F2D7B6A11";
const MSIX_PUBLISHER: &str = "CN=Winuake Dev";
const RUNTIME_PACKAGES: [&str; 4] = [
    "Microsoft.WindowsAppRuntime.2.msix",
    "Microsoft.WindowsAppRuntime.Main.2.msix",
    "Microsoft.WindowsAppRuntime.Singleton.2.msix",
    "Microsoft.WindowsAppRuntime.DDLM.2.msix",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Architecture {
    X64,
    Arm64,
}

impl Architecture {
    fn label(self) -> &'static str {
        match self {
            Architecture::X64 => "x64",
            Architecture::Arm64 => "arm64",
        }
    }

    fn triple(self) -> &'static str {
        match self {
            Architecture::X64 => "x86_64-pc-windows-msvc",
            Architecture::Arm64 => "aarch64-pc-windows-msvc",
        }
    }

    fn runtime_directory(self) -> &'static str {
        match self {
            Architecture::X64 => "win10-x64",
            Architecture::Arm64 => "win10-arm64",
        }
    }

    fn inno_allowed(self) -> &'static str {
        match self {
            Architecture::X64 => "x64compatible",
            Architecture::Arm64 => "arm64",
        }
    }

    fn pe_machine(self) -> u16 {
        match self {
            Architecture::X64 => 0x8664,
            Architecture::Arm64 => 0xAA64,
        }
    }
}

struct Options {
    architectures: Vec<Architecture>,
    version: String,
    /// `OWNER/NAME` of the GitHub repository the release assets are published in (winget manifest URLs).
    repository: Option<String>,
}

fn parse_options(arguments: &[String], default_version: &str) -> Result<Options, Box<dyn Error>> {
    let mut architectures = vec![Architecture::X64, Architecture::Arm64];
    let mut version = default_version.to_owned();
    let mut repository = std::env::var("WINUAKE_REPOSITORY").ok();
    let mut iterator = arguments.iter();
    while let Some(argument) = iterator.next() {
        match argument.as_str() {
            "--arch" => {
                architectures = match iterator.next().map(String::as_str) {
                    Some("x64") => vec![Architecture::X64],
                    Some("arm64") => vec![Architecture::Arm64],
                    Some("all") => vec![Architecture::X64, Architecture::Arm64],
                    other => return Err(format!("--arch takes x64, arm64 or all, not {other:?}").into()),
                }
            }
            "--version" => version = iterator.next().ok_or("--version needs a value")?.clone(),
            "--repo" => repository = Some(iterator.next().ok_or("--repo needs OWNER/NAME")?.clone()),
            other => return Err(format!("unknown option {other}").into()),
        }
    }
    if let Some(repository) = &repository
        && repository.split('/').filter(|part| !part.is_empty()).count() != 2
    {
        return Err(format!("--repo must look like OWNER/NAME, not {repository}").into());
    }
    Ok(Options { architectures, version, repository })
}

pub fn run(workspace_root: &Path, arguments: &[String]) -> Result<(), Box<dyn Error>> {
    if !cfg!(windows) {
        return Err("packaging needs a Windows host (MSVC, makeappx, Inno Setup)".into());
    }
    let default_version = env!("CARGO_PKG_VERSION");
    let options = parse_options(arguments, default_version)?;
    let distribution = workspace_root.join("dist").join(&options.version);
    if distribution.exists() {
        fs::remove_dir_all(&distribution)?;
    }
    fs::create_dir_all(&distribution)?;
    let runtime = winappsdk::ensure_package(workspace_root, winappsdk::PACKAGE_RUNTIME)?;
    let signing =
        prepare_signing(&distribution).map_err(|error| eprintln!("warning: no signing certificate: {error}")).ok();

    let mut artifacts: Vec<PathBuf> = Vec::new();
    for architecture in &options.architectures {
        let architecture = *architecture;
        eprintln!("== {} ==", architecture.label());
        build_release(workspace_root, architecture)?;
        let stage = stage_files(workspace_root, &distribution, architecture, &options.version)?;
        check_binaries(&stage, architecture)?;
        if sign_binaries(signing.as_ref(), &[stage.join("winuake.exe"), stage.join("winuake-cli.exe")])? {
            eprintln!("   signed executables");
        }
        artifacts.push(make_zip(&distribution, &stage, architecture, &options.version)?);
        match make_installer(workspace_root, &distribution, &stage, &runtime, architecture, &options.version) {
            Ok(path) => {
                if sign_binaries(signing.as_ref(), std::slice::from_ref(&path))? {
                    eprintln!("   signed installer");
                }
                artifacts.push(path);
            }
            Err(error) => eprintln!("warning: no installer for {}: {error}", architecture.label()),
        }
        match signing.as_ref().ok_or_else(|| "no signing certificate".into()).and_then(|signing| {
            make_msix(workspace_root, &distribution, &stage, architecture, &options.version, signing)
        }) {
            Ok(path) => artifacts.push(path),
            Err(error) => eprintln!("warning: no MSIX for {}: {error}", architecture.label()),
        }
    }
    write_checksums(&distribution, &artifacts)?;
    write_winget_manifest(&distribution, &artifacts, &options.version, options.repository.as_deref())?;
    eprintln!("done: {}", distribution.display());
    for artifact in &artifacts {
        eprintln!("  {}", artifact.display());
    }
    Ok(())
}

fn build_release(workspace_root: &Path, architecture: Architecture) -> Result<(), Box<dyn Error>> {
    let status = Command::new("cargo")
        .current_dir(workspace_root)
        .args(["build", "--release", "--target", architecture.triple(), "-p", "winuake", "-p", "winuake-cli"])
        .status()?;
    if !status.success() {
        return Err(format!("cargo build failed for {}", architecture.triple()).into());
    }
    Ok(())
}

fn copy_directory(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Lays out the files of one architecture: executables, bootstrap DLL, skins, licence and docs.
fn stage_files(
    workspace_root: &Path,
    distribution: &Path,
    architecture: Architecture,
    version: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let build = workspace_root.join("target").join(architecture.triple()).join("release");
    let stage = distribution
        .join("stage")
        .join(architecture.label())
        .join(format!("winuake-{version}-win-{}", architecture.label()));
    fs::create_dir_all(&stage)?;
    for file in ["winuake.exe", "winuake-cli.exe", "Microsoft.WindowsAppRuntime.Bootstrap.dll"] {
        fs::copy(build.join(file), stage.join(file)).map_err(|error| format!("{file}: {error}"))?;
    }
    copy_directory(&workspace_root.join("assets").join("skins"), &stage.join("skins"))?;
    fs::copy(workspace_root.join("LICENSE"), stage.join("LICENSE"))?;
    fs::copy(workspace_root.join("README.md"), stage.join("README.md"))?;
    fs::copy(workspace_root.join("docs").join("scripting.md"), stage.join("scripting.md"))?;
    fs::copy(workspace_root.join("assets").join("icon").join("winuake.ico"), stage.join("winuake.ico"))?;
    Ok(stage)
}

/// Reads the PE header: the executables must be for the architecture they are packaged as.
fn pe_machine(path: &Path) -> Result<u16, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let offset = u32::from_le_bytes(bytes.get(0x3C..0x40).ok_or("not a PE file")?.try_into()?) as usize;
    let machine = bytes.get(offset + 4..offset + 6).ok_or("not a PE file")?;
    Ok(u16::from_le_bytes([machine[0], machine[1]]))
}

fn check_binaries(stage: &Path, architecture: Architecture) -> Result<(), Box<dyn Error>> {
    for file in ["winuake.exe", "winuake-cli.exe", "Microsoft.WindowsAppRuntime.Bootstrap.dll"] {
        let machine = pe_machine(&stage.join(file))?;
        if machine != architecture.pe_machine() {
            return Err(format!(
                "{file} is machine type 0x{machine:X}, expected 0x{:X} for {}",
                architecture.pe_machine(),
                architecture.label()
            )
            .into());
        }
    }
    eprintln!("   binaries are {}", architecture.label());
    Ok(())
}

fn add_directory_to_zip(
    writer: &mut zip::ZipWriter<fs::File>,
    root: &Path,
    directory: &Path,
    prefix: &str,
) -> Result<(), Box<dyn Error>> {
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
        let name = format!("{prefix}/{relative}");
        if entry.file_type()?.is_dir() {
            add_directory_to_zip(writer, root, &path, prefix)?;
        } else {
            writer.start_file(name, options)?;
            writer.write_all(&fs::read(&path)?)?;
        }
    }
    Ok(())
}

fn make_zip(
    distribution: &Path,
    stage: &Path,
    architecture: Architecture,
    version: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let target = distribution.join(format!("winuake-{version}-win-{}.zip", architecture.label()));
    let mut writer = zip::ZipWriter::new(fs::File::create(&target)?);
    let prefix = stage.file_name().expect("stage has a name").to_string_lossy().into_owned();
    add_directory_to_zip(&mut writer, stage, stage, &prefix)?;
    writer.finish()?;
    Ok(target)
}

fn find_inno() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(explicit) = std::env::var_os("WINUAKE_ISCC") {
        candidates.push(PathBuf::from(explicit));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("Programs").join("Inno Setup 6").join("ISCC.exe"));
    }
    for variable in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(base) = std::env::var_os(variable) {
            candidates.push(PathBuf::from(base).join("Inno Setup 6").join("ISCC.exe"));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn inno_script(
    stage: &Path,
    runtime: &Path,
    output: &Path,
    architecture: Architecture,
    version: &str,
    icon: &Path,
) -> String {
    let runtime_files: String = RUNTIME_PACKAGES
        .iter()
        .map(|package| {
            format!(
                "Source: \"{}\"; DestDir: \"{{tmp}}\\runtime\"; Flags: deleteafterinstall\n",
                runtime.join("tools").join("MSIX").join(architecture.runtime_directory()).join(package).display()
            )
        })
        .collect();
    let install_runtime: String = RUNTIME_PACKAGES
        .iter()
        .map(|package| format!("Add-AppxPackage -Path '{{tmp}}\\runtime\\{package}' -ErrorAction SilentlyContinue; "))
        .collect();
    format!(
        r#"; Generated by `cargo xtask package`.
[Setup]
AppId={{{{{INNO_APP_ID}}}
AppName={PRODUCT}
AppVersion={version}
AppPublisher={PUBLISHER}
DefaultDirName={{autopf}}\{PRODUCT}
DefaultGroupName={PRODUCT}
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed={allowed}
ArchitecturesInstallIn64BitMode={allowed}
OutputDir={output}
OutputBaseFilename=winuake-{version}-win-{arch}-setup
SetupIconFile={icon}
UninstallDisplayIcon={{app}}\winuake.exe
LicenseFile={stage}\LICENSE
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes

[Tasks]
Name: "addtopath"; Description: "Add Winuake to PATH (for winuake-cli)"; GroupDescription: "Command line:"
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{stage}\*"; DestDir: "{{app}}"; Flags: recursesubdirs createallsubdirs ignoreversion
{runtime_files}
[Icons]
Name: "{{group}}\{PRODUCT}"; Filename: "{{app}}\winuake.exe"
Name: "{{autodesktop}}\{PRODUCT}"; Filename: "{{app}}\winuake.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{{olddata}};{{app}}"; Tasks: addtopath; Check: NeedsAddPath(ExpandConstant('{{app}}'))
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "{PRODUCT}"; Flags: uninsdeletevalue dontcreatekey

[Run]
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -Command ""{install_runtime}"""; StatusMsg: "Installing the Windows App SDK runtime..."; Flags: runhidden waituntilterminated
Filename: "{{app}}\winuake.exe"; Description: "Start {PRODUCT}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "taskkill.exe"; Parameters: "/f /im winuake.exe"; Flags: runhidden; RunOnceId: "StopWinuake"

[Code]
function NeedsAddPath(Param: string): Boolean;
var
  Existing: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Existing) then
  begin
    Result := True;
    Exit;
  end;
  Result := Pos(';' + Uppercase(Param) + ';', ';' + Uppercase(Existing) + ';') = 0;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Existing, Cleaned, AppDir: string;
begin
  if (CurUninstallStep = usPostUninstall) and RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Existing) then
  begin
    AppDir := ExpandConstant('{{app}}');
    Cleaned := ';' + Existing + ';';
    StringChangeEx(Cleaned, ';' + AppDir + ';', ';', True);
    while (Length(Cleaned) > 0) and (Cleaned[1] = ';') do Delete(Cleaned, 1, 1);
    while (Length(Cleaned) > 0) and (Cleaned[Length(Cleaned)] = ';') do Delete(Cleaned, Length(Cleaned), 1);
    RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Cleaned);
  end;
end;
"#,
        allowed = architecture.inno_allowed(),
        arch = architecture.label(),
        output = output.display(),
        icon = icon.display(),
        stage = stage.display(),
    )
}

fn make_installer(
    workspace_root: &Path,
    distribution: &Path,
    stage: &Path,
    runtime: &Path,
    architecture: Architecture,
    version: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let compiler =
        find_inno().ok_or("Inno Setup 6 not found (winget install JRSoftware.InnoSetup, or set WINUAKE_ISCC)")?;
    let script_path = distribution.join(format!("winuake-{}.iss", architecture.label()));
    let icon = workspace_root.join("assets").join("icon").join("winuake.ico");
    fs::write(&script_path, inno_script(stage, runtime, distribution, architecture, version, &icon))?;
    let status = Command::new(compiler).arg("/Q").arg(&script_path).status()?;
    if !status.success() {
        return Err("Inno Setup failed".into());
    }
    Ok(distribution.join(format!("winuake-{version}-win-{}-setup.exe", architecture.label())))
}

fn windows_kit_tool(name: &str) -> Option<PathBuf> {
    let base = PathBuf::from(std::env::var_os("ProgramFiles(x86)")?).join("Windows Kits").join("10").join("bin");
    let mut versions: Vec<PathBuf> = fs::read_dir(base).ok()?.flatten().map(|entry| entry.path()).collect();
    versions.sort();
    versions.into_iter().rev().map(|version| version.join("x64").join(name)).find(|path| path.is_file())
}

fn msix_manifest(architecture: Architecture, version: &str) -> String {
    let numeric: Vec<&str> = version.split(['.', '-', '+']).take(3).collect();
    let package_version = format!(
        "{}.{}.{}.0",
        numeric.first().unwrap_or(&"0"),
        numeric.get(1).unwrap_or(&"0"),
        numeric.get(2).unwrap_or(&"0")
    );
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:uap5="http://schemas.microsoft.com/appx/manifest/uap/windows10/5"
         xmlns:desktop="http://schemas.microsoft.com/appx/manifest/desktop/windows10"
         xmlns:desktop6="http://schemas.microsoft.com/appx/manifest/desktop/windows10/6"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap uap5 desktop desktop6 rescap">
  <Identity Name="Winuake.Winuake" Publisher="{MSIX_PUBLISHER}" Version="{package_version}" ProcessorArchitecture="{arch}"/>
  <Properties>
    <DisplayName>{PRODUCT}</DisplayName>
    <PublisherDisplayName>{PUBLISHER}</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.22000.0" MaxVersionTested="10.0.26100.0"/>
    <PackageDependency Name="Microsoft.WindowsAppRuntime.2" MinVersion="2.6.0.0" Publisher="CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US"/>
  </Dependencies>
  <Resources><Resource Language="en-us"/></Resources>
  <Applications>
    <Application Id="Winuake" Executable="winuake.exe" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="{PRODUCT}" Description="Drop-down terminal (port of KDE Yakuake)"
                          BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png"/>
      <Extensions>
        <desktop:Extension Category="windows.startupTask" Executable="winuake.exe" EntryPoint="Windows.FullTrustApplication">
          <desktop:StartupTask TaskId="WinuakeStartup" Enabled="false" DisplayName="{PRODUCT}"/>
        </desktop:Extension>
        <uap5:Extension Category="windows.appExecutionAlias" Executable="winuake-cli.exe" EntryPoint="Windows.FullTrustApplication">
          <uap5:AppExecutionAlias><uap5:ExecutionAlias Alias="winuake-cli.exe"/></uap5:AppExecutionAlias>
        </uap5:Extension>
      </Extensions>
    </Application>
  </Applications>
  <Capabilities><rescap:Capability Name="runFullTrust"/></Capabilities>
</Package>
"#,
        arch = architecture.label()
    )
}

fn powershell(script: &str) -> Result<String, Box<dyn Error>> {
    let output =
        Command::new("powershell").args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script]).output()?;
    if !output.status.success() {
        return Err(format!("powershell: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

struct Signing {
    pfx: PathBuf,
    password: String,
    /// A self-signed development certificate: fine for the MSIX test install, but binaries are not signed with it.
    throwaway: bool,
    /// RFC 3161 timestamp server, so signatures outlive the certificate.
    timestamp_url: Option<String>,
}

/// With `WINUAKE_SIGN_PFX` (and `WINUAKE_SIGN_PASSWORD`) signs with that certificate, whose subject must equal the manifest publisher.
/// Otherwise makes one throwaway self-signed certificate for the whole run (`winuake-dev.cer` is written next to the packages so it can
/// be trusted for local testing only).
fn prepare_signing(distribution: &Path) -> Result<Signing, Box<dyn Error>> {
    if let Ok(pfx) = std::env::var("WINUAKE_SIGN_PFX") {
        let timestamp_url =
            Some(std::env::var("WINUAKE_TIMESTAMP_URL").unwrap_or_else(|_| "http://timestamp.digicert.com".to_owned()));
        return Ok(Signing {
            pfx: PathBuf::from(pfx),
            password: std::env::var("WINUAKE_SIGN_PASSWORD").unwrap_or_default(),
            throwaway: false,
            timestamp_url,
        });
    }
    let pfx = distribution.join("winuake-dev.pfx");
    let cer = distribution.join("winuake-dev.cer");
    let script = format!(
        "$p = ConvertTo-SecureString 'winuake-dev' -AsPlainText -Force; \
         $c = New-SelfSignedCertificate -Type Custom -Subject '{MSIX_PUBLISHER}' -KeyUsage DigitalSignature -FriendlyName 'Winuake dev' \
         -CertStoreLocation 'Cert:\\CurrentUser\\My' -TextExtension @('2.5.29.37={{text}}1.3.6.1.5.5.7.3.3', '2.5.29.19={{text}}'); \
         Export-PfxCertificate -Cert $c -FilePath '{}' -Password $p | Out-Null; Export-Certificate -Cert $c -FilePath '{}' | Out-Null",
        pfx.display(),
        cer.display()
    );
    powershell(&script)?;
    Ok(Signing { pfx, password: "winuake-dev".to_owned(), throwaway: true, timestamp_url: None })
}

/// Signs one file with `signtool`.
fn sign_file(signtool: &Path, signing: &Signing, file: &Path) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new(signtool);
    command.args(["sign", "/fd", "SHA256", "/f"]).arg(&signing.pfx).args(["/p", &signing.password]);
    if let Some(url) = &signing.timestamp_url {
        command.args(["/tr", url, "/td", "SHA256"]);
    }
    let status = command.arg(file).status()?;
    if !status.success() {
        return Err(format!("signtool failed for {}", file.display()).into());
    }
    Ok(())
}

/// Signs the executables and, later, the installer, when a real certificate was given. Returns whether it signed.
fn sign_binaries(signing: Option<&Signing>, files: &[PathBuf]) -> Result<bool, Box<dyn Error>> {
    let Some(signing) = signing.filter(|signing| !signing.throwaway) else { return Ok(false) };
    let signtool = windows_kit_tool("signtool.exe").ok_or("signtool.exe (Windows SDK) not found")?;
    for file in files {
        sign_file(&signtool, signing, file)?;
    }
    Ok(true)
}

/// Builds and signs an MSIX. With `WINUAKE_SIGN_PFX` (and `WINUAKE_SIGN_PASSWORD`) it signs with that certificate, whose subject must equal
/// the manifest publisher; otherwise it makes a throwaway self-signed certificate (`winuake-dev.cer` is written next to the package so it can be
/// trusted for local testing only).
fn make_msix(
    workspace_root: &Path,
    distribution: &Path,
    stage: &Path,
    architecture: Architecture,
    version: &str,
    signing: &Signing,
) -> Result<PathBuf, Box<dyn Error>> {
    let makeappx = windows_kit_tool("makeappx.exe").ok_or("makeappx.exe (Windows SDK) not found")?;
    let signtool = windows_kit_tool("signtool.exe").ok_or("signtool.exe (Windows SDK) not found")?;
    let layout = distribution.join("msix").join(architecture.label());
    if layout.exists() {
        fs::remove_dir_all(&layout)?;
    }
    copy_directory(stage, &layout)?;
    let assets = layout.join("Assets");
    fs::create_dir_all(&assets)?;
    let svg = fs::read(workspace_root.join("assets").join("icon").join("winuake.svg"))?;
    for (name, pixels) in [("Square44x44Logo.png", 44), ("Square150x150Logo.png", 150), ("StoreLogo.png", 50)] {
        fs::write(assets.join(name), winuake_skin::raster::svg_to_square_png(&svg, pixels)?)?;
    }
    fs::write(layout.join("AppxManifest.xml"), msix_manifest(architecture, version))?;
    let package = distribution.join(format!("winuake-{version}-win-{}.msix", architecture.label()));
    let status = Command::new(&makeappx).args(["pack", "/o", "/d"]).arg(&layout).arg("/p").arg(&package).status()?;
    if !status.success() {
        return Err("makeappx failed".into());
    }

    sign_file(&signtool, signing, &package)?;
    Ok(package)
}

fn sha256_of(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut hasher = Sha256::new();
    hasher.update(fs::read(path)?);
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02X}")).collect())
}

fn write_checksums(distribution: &Path, artifacts: &[PathBuf]) -> Result<(), Box<dyn Error>> {
    let mut lines = String::new();
    for artifact in artifacts {
        lines += &format!("{}  {}\n", sha256_of(artifact)?, artifact.file_name().expect("file name").to_string_lossy());
    }
    fs::write(distribution.join("SHA256SUMS.txt"), lines)?;
    Ok(())
}

/// winget manifests (singleton-free multi-file form) for the two installers. The URLs assume GitHub release assets under
/// `WINUAKE_RELEASE_BASE_URL`; fill in the real repository before submitting to winget-pkgs.
fn write_winget_manifest(
    distribution: &Path,
    artifacts: &[PathBuf],
    version: &str,
    repository: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let repository = repository.unwrap_or(DEFAULT_REPOSITORY);
    let base = std::env::var("WINUAKE_RELEASE_BASE_URL")
        .unwrap_or_else(|_| format!("https://github.com/{repository}/releases/download/v{version}"));
    let directory = distribution.join("winget");
    fs::create_dir_all(&directory)?;
    let mut installers = String::new();
    for architecture in [Architecture::X64, Architecture::Arm64] {
        let name = format!("winuake-{version}-win-{}-setup.exe", architecture.label());
        let Some(path) =
            artifacts.iter().find(|path| path.file_name().is_some_and(|file| file.to_string_lossy() == name))
        else {
            continue;
        };
        installers += &format!(
            "- Architecture: {}\n  InstallerUrl: {base}/{name}\n  InstallerSha256: {}\n",
            architecture.label(),
            sha256_of(path)?
        );
    }
    fs::write(
        directory.join("Winuake.Winuake.yaml"),
        format!(
            "PackageIdentifier: Winuake.Winuake\nPackageVersion: {version}\nDefaultLocale: en-US\nManifestType: version\nManifestVersion: 1.6.0\n"
        ),
    )?;
    fs::write(
        directory.join("Winuake.Winuake.installer.yaml"),
        format!(
            "PackageIdentifier: Winuake.Winuake\nPackageVersion: {version}\nInstallerType: inno\nScope: user\nInstallModes:\n- interactive\n- silent\n- silentWithProgress\nMinimumOSVersion: 10.0.22000.0\nUpgradeBehavior: install\nCommands:\n- winuake-cli\nInstallers:\n{installers}ManifestType: installer\nManifestVersion: 1.6.0\n"
        ),
    )?;
    fs::write(
        directory.join("Winuake.Winuake.locale.en-US.yaml"),
        format!(
            "PackageIdentifier: Winuake.Winuake\nPackageVersion: {version}\nPackageLocale: en-US\nPublisher: {PUBLISHER}\nPackageName: {PRODUCT}\nShortDescription: Drop-down terminal for Windows 11, a port of KDE Yakuake.\nLicense: GPL-2.0-or-later\nLicenseUrl: https://github.com/{repository}/blob/main/LICENSE\nPublisherUrl: https://github.com/{repository}\nTags:\n- terminal\n- dropdown\n- quake\n- yakuake\nManifestType: defaultLocale\nManifestVersion: 1.6.0\n"
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_no_options_when_parsing_then_both_architectures_and_the_crate_version_are_used() {
        let options = parse_options(&[], "1.2.3").expect("parses");
        assert_eq!(options.architectures.len(), 2);
        assert_eq!(options.version, "1.2.3");
    }

    #[test]
    fn given_arch_and_version_when_parsing_then_they_are_honoured() {
        let arguments: Vec<String> =
            ["--arch", "arm64", "--version", "9.9.9"].iter().map(|text| (*text).to_owned()).collect();
        let options = parse_options(&arguments, "1.0.0").expect("parses");
        assert_eq!(options.architectures.len(), 1);
        assert_eq!(options.architectures[0].label(), "arm64");
        assert_eq!(options.version, "9.9.9");
    }

    #[test]
    fn given_a_repository_when_parsing_then_it_must_look_like_owner_slash_name() {
        let good: Vec<String> = ["--repo", "someone/winuake"].iter().map(|text| (*text).to_owned()).collect();
        assert_eq!(parse_options(&good, "1.0.0").expect("parses").repository.as_deref(), Some("someone/winuake"));
        for bad in ["winuake", "a/b/c", "/"] {
            let arguments = vec!["--repo".to_owned(), bad.to_owned()];
            assert!(parse_options(&arguments, "1.0.0").is_err(), "{bad}");
        }
    }

    #[test]
    fn given_bad_options_when_parsing_then_an_error_explains() {
        let bad: Vec<String> = ["--arch", "mips"].iter().map(|text| (*text).to_owned()).collect();
        assert!(parse_options(&bad, "1.0.0").is_err());
        assert!(parse_options(&["--nope".to_owned()], "1.0.0").is_err());
    }

    #[test]
    fn given_each_architecture_then_triple_machine_and_runtime_directory_agree() {
        assert_eq!(Architecture::X64.triple(), "x86_64-pc-windows-msvc");
        assert_eq!(Architecture::Arm64.triple(), "aarch64-pc-windows-msvc");
        assert_eq!(Architecture::X64.pe_machine(), 0x8664);
        assert_eq!(Architecture::Arm64.pe_machine(), 0xAA64);
        assert_eq!(Architecture::Arm64.runtime_directory(), "win10-arm64");
    }

    #[test]
    fn given_a_version_when_building_the_msix_manifest_then_the_identity_has_a_four_part_version_and_the_architecture()
    {
        let manifest = msix_manifest(Architecture::Arm64, "1.2.3");
        assert!(manifest.contains("Version=\"1.2.3.0\""));
        assert!(manifest.contains("ProcessorArchitecture=\"arm64\""));
        assert!(manifest.contains("Microsoft.WindowsAppRuntime.2"));
        assert!(manifest.contains("windows.startupTask"));
    }

    #[test]
    fn given_an_installer_script_then_it_installs_the_matching_runtime_and_targets_the_architecture() {
        let script = inno_script(
            Path::new("C:/stage"),
            Path::new("C:/rt"),
            Path::new("C:/out"),
            Architecture::Arm64,
            "1.0.0",
            Path::new("C:/i.ico"),
        );
        assert!(script.contains("ArchitecturesAllowed=arm64"));
        assert!(script.contains("win10-arm64"));
        assert!(script.contains("Microsoft.WindowsAppRuntime.DDLM.2.msix"));
        assert!(script.contains("OutputBaseFilename=winuake-1.0.0-win-arm64-setup"));
    }

    #[test]
    fn given_bytes_when_hashing_then_the_digest_is_uppercase_hex() {
        let path = std::env::temp_dir().join(format!("winuake-xtask-hash-{}", std::process::id()));
        fs::write(&path, b"abc").expect("writes");
        let digest = sha256_of(&path).expect("hashes");
        let _ = fs::remove_file(&path);
        assert_eq!(digest, "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD");
    }
}
