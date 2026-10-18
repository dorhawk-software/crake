; Crake's installer for Windows (scripts/build-windows-installer.sh builds
; it with NSIS): Program Files\Crake with openconnect and Wintun, and
; OpenVPN and its two drivers, beside it (their licences and where their
; source is, too; the drivers into Windows' driver store), craked as an automatic
; service, a Start menu entry, and an entry in Installed apps. Installing
; over an older Crake stops its service first. Uninstalling stops and
; removes the service and the programs; the user's tunnels and log stay
; in ProgramData\Crake (as apt's remove, not purge).
; Unsigned: signing is decided before a release (CLAUDE.md).
;
;   makensis -DVERSION=0.1.0 -DSRC=<staged files> -DOUT=<setup.exe> crake.nsi

Unicode true
!include "MUI2.nsh"
!include "x64.nsh"

Name "Crake"
OutFile "${OUT}"
InstallDir "$PROGRAMFILES64\Crake"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "dorhawk.software"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Crake"
VIAddVersionKey "CompanyName" "dorhawk.software"
VIAddVersionKey "FileDescription" "Crake installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "GPL-3.0-or-later"

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Crake"

; Modern UI (MUI2): a welcome page, the folder page, progress, and a finish
; page that can open Crake. Crake's own icon throughout.
!define MUI_ICON "${SRC}\crake.ico"
!define MUI_UNICON "${SRC}\crake.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Crake"
!define MUI_WELCOMEPAGE_TEXT "Crake connects to your VPNs but routes nothing by default. It learns, per host, what needs a tunnel and sends only that through, telling you each time.$\r$\n$\r$\nThis will install Crake, with openconnect, Wintun and OpenVPN beside it, and run it as a background service."
!define MUI_FINISHPAGE_RUN "$INSTDIR\crake.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open Crake"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!define MUI_FINISHPAGE_TEXT "Crake is installed and running in the notification area. Click the bird to open it and add a tunnel."

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

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
  SetOutPath "$INSTDIR\openvpn"
  File /r "${SRC}\openvpn\*"
  SetOutPath "$INSTDIR"

  ; OpenVPN's drivers (signed by Microsoft for OpenVPN Inc.): ovpn-dco, its
  ; build for Windows 11 or 10, and tap-windows6. craked makes each OpenVPN
  ; tunnel's adapter with them. pnputil through Sysnative: this installer is
  ; 32-bit, and System32's 64-bit pnputil is hidden from it otherwise.
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion" "CurrentBuildNumber"
  StrCpy $1 "Win10"
  IntCmp $0 22000 win11 win10 win11
  win11:
    StrCpy $1 "Win11"
  win10:
  nsExec::ExecToLog '"$WINDIR\Sysnative\pnputil.exe" /add-driver "$INSTDIR\openvpn\ovpn-dco\$1\ovpn-dco.inf" /install'
  nsExec::ExecToLog '"$WINDIR\Sysnative\pnputil.exe" /add-driver "$INSTDIR\openvpn\tap-windows6\OemVista.inf" /install'
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
  ; The drivers stay in Windows' driver store (with no adapter left, they
  ; do nothing; OpenVPN's own installer may share them).
  RMDir /r "$INSTDIR\openvpn"
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
