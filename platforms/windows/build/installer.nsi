; Mesh Town installer for Windows x64 (NSIS 3, Unicode, MUI2).
; Build: makensis -DVERSION=<x.y.z> -DEXE=<path to the gui exe> -DOUTFILE=<path to the installer> installer.nsi
; (platforms/windows/build/installer.sh passes all three).
; The installer puts the exe, the icon and the uninstaller under Program Files, adds shortcuts, an
; Uninstall entry and a firewall rule. It installs no service and does not enable autostart: the program
; does that itself from its settings page. User data (%APPDATA%\meshtown) is never touched.

!ifndef VERSION
  !error "define VERSION: makensis -DVERSION=<x.y.z> ..."
!endif
!ifndef EXE
  !error "define EXE: makensis -DEXE=<path to the gui exe> ..."
!endif
!ifndef OUTFILE
  !error "define OUTFILE: makensis -DOUTFILE=<path to the installer> ..."
!endif

!define APP "Mesh Town"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\MeshTown"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define APPROVED_KEY "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
!define HOME_URL "https://github.com/mesh-town/meshtown"

Unicode true
Name "${APP}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\${APP}"
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallLocation"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "${APP} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APP}"
VIAddVersionKey "CompanyName" "${APP}"
VIAddVersionKey "FileDescription" "${APP} installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "AGPL-3.0-only"

!include "MUI2.nsh"
!include "x64.nsh"
!include "FileFunc.nsh"

!define MUI_ICON "meshtown.ico"
!define MUI_UNICON "meshtown.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Run ${APP}"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchApp

!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Var WasRunning

; Stops every meshtown.exe whose path is inside $INSTDIR (copies elsewhere stay alive) and waits up to
; 10 s for each to exit. The exit code of powershell is 1 when something was running, 0 otherwise.
; The installer is a 32-bit process, so the powershell it starts is 32-bit too, and Get-Process there
; leaves Path empty for 64-bit processes; Win32_Process reports ExecutablePath for any bitness.
!macro StopApp un
Function ${un}StopApp
  nsExec::Exec `powershell -NoProfile -NonInteractive -Command "$$ids = @(Get-CimInstance Win32_Process | Where-Object { $$_.Name -eq 'meshtown.exe' -and $$_.ExecutablePath -and $$_.ExecutablePath.StartsWith('$INSTDIR\', [StringComparison]::OrdinalIgnoreCase) } | ForEach-Object { $$_.ProcessId }); if ($$ids.Count -eq 0) { exit 0 }; Stop-Process -Id $$ids -Force -ErrorAction SilentlyContinue; Wait-Process -Id $$ids -Timeout 10 -ErrorAction SilentlyContinue; exit 1"`
  Pop $0
FunctionEnd
!macroend
!insertmacro StopApp ""
!insertmacro StopApp "un."

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "${APP} needs 64-bit Windows." /SD IDOK
    Abort
  ${EndIf}
  SetRegView 64
  ReadRegStr $0 HKLM "${UNINST_KEY}" "InstallLocation"
  ${If} $0 != ""
    StrCpy $INSTDIR $0
  ${EndIf}
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

Function LaunchApp
  ; Already restarted after an update: do not start a second tray.
  ${If} $WasRunning != 1
    Exec '"$WINDIR\explorer.exe" "$INSTDIR\meshtown.exe"'
  ${EndIf}
FunctionEnd

Section "${APP}" SecMain
  SectionIn RO
  SetRegView 64
  SetShellVarContext all
  Call StopApp
  StrCpy $WasRunning 0
  ${If} $0 == 1
    StrCpy $WasRunning 1
  ${EndIf}

  SetOutPath "$INSTDIR"
  File "/oname=meshtown.exe" "${EXE}"
  File "meshtown.ico"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\meshtown.exe" "tray" "$INSTDIR\meshtown.ico"

  ; Re-added on every install so an update never leaves duplicates.
  nsExec::Exec `netsh advfirewall firewall delete rule name="${APP}"`
  Pop $0
  nsExec::Exec `netsh advfirewall firewall add rule name="${APP}" dir=in action=allow program="$INSTDIR\meshtown.exe" enable=yes profile=any`
  Pop $0

  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "${APP}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\meshtown.ico"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegStr HKLM "${UNINST_KEY}" "URLInfoAbout" "${HOME_URL}"
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKLM "${UNINST_KEY}" "EstimatedSize" $0
SectionEnd

Section "Desktop shortcut" SecDesktop
  SetShellVarContext all
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\meshtown.exe" "tray" "$INSTDIR\meshtown.ico"
SectionEnd

Function .onInstSuccess
  ; The tray was running before the update: bring it back, not elevated. explorer.exe drops the
  ; elevation but passes no arguments, so this relies on the GUI exe starting the tray when run bare.
  ${If} $WasRunning == 1
    Exec '"$WINDIR\explorer.exe" "$INSTDIR\meshtown.exe"'
  ${EndIf}
FunctionEnd

Section "Uninstall"
  SetRegView 64
  SetShellVarContext all
  Call un.StopApp

  Delete "$INSTDIR\meshtown.exe"
  Delete "$INSTDIR\meshtown.ico"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP}.lnk"
  Delete "$DESKTOP\${APP}.lnk"

  nsExec::Exec `netsh advfirewall firewall delete rule name="${APP}"`
  Pop $0

  ; Autostart written by the program points at the removed exe: drop it only if it is ours.
  ReadRegStr $0 HKCU "${RUN_KEY}" "${APP}"
  ${If} $0 == '"$INSTDIR\meshtown.exe" tray'
    DeleteRegValue HKCU "${RUN_KEY}" "${APP}"
    DeleteRegValue HKCU "${APPROVED_KEY}" "${APP}"
  ${EndIf}

  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
