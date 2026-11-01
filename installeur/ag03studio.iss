; Installeur Windows d'AG03 MK2 Studio (Inno Setup 6).
; Construit par construire.ps1 ; ne pas lancer seul sans avoir compilé en --release.

#define AppName "AG03 MK2 Studio"
#define AppVersion "0.1.0"
#define AppPublisher "Lopjoy"

[Setup]
AppId={{B09687E5-6F64-47E8-84DD-41590D532105}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppCopyright=Copyright 2026 {#AppPublisher}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription=Installeur de {#AppName}
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; Installation sans droits administrateur (dans le profil), au choix de l'utilisateur
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\LICENSE
SetupIconFile=..\assets\icone.ico
UninstallDisplayIcon={app}\ag03studio.exe
UninstallDisplayName={#AppName}
OutputDir=..\dist
OutputBaseFilename=AG03-MK2-Studio-{#AppVersion}-installeur
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "bureau"; Description: "Créer un raccourci sur le Bureau"; GroupDescription: "Raccourcis :"
Name: "ligne"; Description: "Installer aussi l'outil en ligne de commande (ag03ctl)"; GroupDescription: "Options :"; Flags: unchecked

[Files]
Source: "..\target\release\ag03studio.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\ag03ctl.exe"; DestDir: "{app}"; Flags: ignoreversion; Tasks: ligne
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\NOTICE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\PROTOCOLE.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\ag03studio.exe"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\ag03studio.exe"; Tasks: bureau

[Run]
Filename: "{app}\ag03studio.exe"; Description: "Lancer {#AppName}"; Flags: nowait postinstall skipifsilent

[Messages]
french.WelcomeLabel2=Cet assistant va installer [name/ver] sur votre ordinateur.%n%nAG03 MK2 Studio pilote la Yamaha AG03 MK2 par USB. Le pilote « Yamaha Steinberg USB Driver » doit être installé.%n%nLogiciel indépendant, non affilié à Yamaha.
