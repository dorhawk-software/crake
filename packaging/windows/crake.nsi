; Crake's installer for Windows (scripts/build-windows-installer.sh builds
; it with NSIS): Program Files\Crake with openconnect and Wintun beside it
; (their licences and where their source is, too), craked as an automatic
; service, a Start menu entry, and an entry in Installed apps. Installing
; over an older Crake stops its service first. Uninstalling stops and
; removes the service and the programs; the user's tunnels and log stay
; in ProgramData\Crake (as apt's remove, not purge).
; Unsigned: signing is decided before a release (CLAUDE.md).
;
;   makensis -DVERSION=0.1.0 -DSRC=<staged files> -DOUT=<setup.exe> crake.nsi

Unicode true
!include "x64.nsh"

Name "Crake"
OutFile "${OUT}"
InstallDir "$PROGRAMFILES64\Crake"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
Icon "${SRC}\crake.ico"
UninstallIcon "${SRC}\crake.ico"
BrandingText "dorhawk.software"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Crake"
VIAddVersionKey "CompanyName" "dorhawk.software"
VIAddVersionKey "FileDescription" "Crake installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "GPL-3.0-or-later"

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Crake"

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "Crake needs 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

Section "Crake"
  ; An older Crake: its app and service stop before their files change.
  nsExec::Exec 'taskkill /IM crake.exe /F'
  nsExec::Exec 'sc.exe stop craked'
  Sleep 3000

  SetOutPath "$INSTDIR"
  File "${SRC}\crake.exe"
  File "${SRC}\craked.exe"
  File "${SRC}\crake-hook.exe"
  File "${SRC}\crake-hook.js"
  File "${SRC}\crake.ico"
  File "${SRC}\LICENSE.txt"
  SetOutPath "$INSTDIR\openconnect"
  File /r "${SRC}\openconnect\*"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; craked as a service (created once; an upgrade keeps it).
  nsExec::Exec 'sc.exe create craked binPath= "\"$INSTDIR\craked.exe\"" start= auto DisplayName= "Crake"'
  nsExec::Exec 'sc.exe description craked "Routes only what needs a VPN tunnel."'
  nsExec::Exec 'sc.exe failure craked reset= 86400 actions= restart/5000'
  nsExec::Exec 'sc.exe start craked'

  CreateShortcut "$SMPROGRAMS\Crake.lnk" "$INSTDIR\crake.exe" "" "$INSTDIR\crake.ico"

  WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayName" "Crake"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "Publisher" "dorhawk.software"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "URLInfoAbout" "https://dorhawk.software/crake/"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\crake.ico"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /IM crake.exe /F'
  ; Stopping craked switches every tunnel off.
  nsExec::Exec 'sc.exe stop craked'
  Sleep 3000
  nsExec::Exec 'sc.exe delete craked'
  Delete "$SMPROGRAMS\Crake.lnk"
  RMDir /r "$INSTDIR\openconnect"
  Delete "$INSTDIR\crake.exe"
  Delete "$INSTDIR\craked.exe"
  Delete "$INSTDIR\crake-hook.exe"
  Delete "$INSTDIR\crake-hook.js"
  Delete "$INSTDIR\crake.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "${UNINSTALL_KEY}"
SectionEnd
