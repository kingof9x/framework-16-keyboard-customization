; Installer for Framework 16 Keyboard Control (Inno Setup 6).
; Built in CI (.github/workflows/app.yml):
;   iscc /DAppVersion=0.2.0 app\installer\installer.iss
; Installs for the current user only, so no administrator prompt is needed.

#define AppName "Framework 16 Keyboard Control"
#define AppExe "framework-16-keyboard-control.exe"
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{49DC55DF-26AA-4912-98D2-497D748E6BBE}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=kingof9x
AppPublisherURL=https://github.com/kingof9x/framework-16-keyboard-customization
AppSupportURL=https://github.com/kingof9x/framework-16-keyboard-customization/issues
AppUpdatesURL=https://github.com/kingof9x/framework-16-keyboard-customization/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\target\installer
OutputBaseFilename=framework-16-keyboard-control-{#AppVersion}-setup
SetupIconFile=..\gui\assets\app.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
; An update closes the running app first (it may be waiting in the notification area)
CloseApplications=force
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; The app's own "Start with Windows" setting writes this; remove it on uninstall
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "{#AppName}"; Flags: uninsdeletevalue dontcreatekey

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM {#AppExe} /F"; Flags: runhidden; RunOnceId: "StopApp"
