; Inno Setup 6 installer script for Lekh (lk + lekh)
; Installs BOTH lk.exe and lekh.exe per-user into %LOCALAPPDATA%\Programs\Lekh
; and adds that folder to the Windows User PATH so `lk` and `lekh` work in
; PowerShell, Windows Terminal, and cmd.exe immediately without Cargo.

#define MyAppName "Lekh"
#define MyAppVersion "0.1.1"
#define MyAppPublisher "khajumsanjog"
#define MyAppURL "https://github.com/khajumsanjog/Lekh"

[Setup]
AppId={{8D6B6E4A-5C73-4F29-9A41-2E8C01B0E110}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\..\dist-installer
OutputBaseFilename=lekh-windows-x86_64-setup
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
VersionInfoVersion={#MyAppVersion}.0
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription=Lekh (lk) Setup - Fast, safe terminal text editor
VersionInfoCopyright=Copyright (c) 2026 {#MyAppPublisher} (MIT License)
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppVersion}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\..\dist\lk.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dist\lekh.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dist\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dist\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath(ExpandConstant('{app}'))

[Code]
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath)
  then begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(Param) + ';', ';' + Uppercase(OrigPath) + ';') = 0;
end;

procedure RemovePath(PathToRemove: string);
var
  OrigPath, NewPath, P: string;
  StartPos: Integer;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
    exit;
  P := ';' + OrigPath + ';';
  StartPos := Pos(';' + Uppercase(PathToRemove) + ';', Uppercase(P));
  if StartPos = 0 then
    exit;
  NewPath := Copy(P, 1, StartPos - 1) + Copy(P, StartPos + Length(PathToRemove) + 1, MaxInt);
  if (Length(NewPath) > 0) and (NewPath[1] = ';') then
    Delete(NewPath, 1, 1);
  if (Length(NewPath) > 0) and (NewPath[Length(NewPath)] = ';') then
    Delete(NewPath, Length(NewPath), 1);
  RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', NewPath);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    // ChangesEnvironment=yes broadcasts WM_SETTINGCHANGE automatically
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemovePath(ExpandConstant('{app}'));
end;
