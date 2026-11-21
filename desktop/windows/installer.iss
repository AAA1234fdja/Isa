; Unsigned experimental per-user installer. This does not install WebView2,
; configure a workspace, enable local actions, or modify a source checkout.
#define AppName "ISA Desktop Prototype"
#define AppVersion "0.1.0"
#define AppExe "Isa.Desktop.exe"

[Setup]
AppId={{EEC08B77-7160-4D8D-962E-3B429C55509B}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=ISA
DefaultDirName={localappdata}\Programs\ISA Desktop Prototype
DefaultGroupName={#AppName}
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir=..\..\desktop-installer
OutputBaseFilename=ISA-Desktop-Prototype-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=isa.ico
UninstallDisplayIcon={app}\{#AppExe}

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "..\..\desktop-prototype\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

; No Run entry: installation never launches the app, backend, or arbitrary code.
; The WebView profile in LocalAppData is intentionally retained on uninstall.
