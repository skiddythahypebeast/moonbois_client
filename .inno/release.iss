[Setup]
AppName=Moonbois
AppVersion=0.1.0
DefaultDirName={userpf}\Moonbois
OutputDir=.\output
OutputBaseFilename=MoonboisSetup
UninstallDisplayName=Moonbois
ArchitecturesAllowed=x64compatible
PrivilegesRequired=lowest
ChangesEnvironment=true
AppPublisher=skiddythahypebeast
SetupIconFile="..\assets\moonbois.ico"
UninstallDisplayIcon="{app}\moonbois.ico"

[Files]
Source: "..\target\release\moonbois_cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\detect_enter.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\moonbois.ico"; DestDir: "{app}";

[Run]
Filename: "{app}\moonbois_cli.exe"; Description: "Run Moonbois CLI"; Flags: nowait postinstall skipifsilent

[Icons]
Name: "{group}\Moonbois Uninstaller"; Filename: "{uninstallexe}"; IconFilename: "{app}\moonbois.ico"
Name: "{group}\Moonbois"; Filename: "{app}\moonbois_cli.exe"; IconFilename: "{app}\moonbois.ico"
Name: "{userdesktop}\Moonbois"; Filename: "{app}\moonbois_cli.exe"; IconFilename: "{app}\moonbois.ico"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\{#SetupSetting("AppName")}"; ValueType: string; ValueName: "DisplayIcon"; ValueData: "{app}\moonbois.ico"; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Environment"; ValueType: string; ValueName: "Path"; ValueData: "{olddata};{app}";
Root: HKCU; Subkey: "Environment"; ValueType: string; ValueName: "MOONBOIS_ROOT"; ValueData: "{app}";