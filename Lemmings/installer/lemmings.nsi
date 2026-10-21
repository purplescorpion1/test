; lemmings.nsi — NSIS installer script for Lemmings Windows 11 port
; Build with: makensis lemmings.nsi
; Requires NSIS 3.x (https://nsis.sourceforge.io)

Unicode True

!define APP_NAME        "Lemmings"
!define APP_VERSION     "1.0.0"
!define APP_PUBLISHER   "Spirit Port"
!define APP_URL         "https://github.com/purplescorpion1/test"
!define APP_EXE         "lemmings.exe"
!define INSTALL_DIR     "$PROGRAMFILES64\Lemmings"
!define UNINSTALLER     "uninstall.exe"
!define REG_KEY         "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lemmings"

Name             "${APP_NAME} ${APP_VERSION}"
OutFile          "LemmingsSetup.exe"
InstallDir       "${INSTALL_DIR}"
InstallDirRegKey HKLM "${REG_KEY}" "InstallLocation"
RequestExecutionLevel admin
SetCompressor    /SOLID lzma
SetCompressorDictSize 32

!include "MUI2.nsh"
!include "LogicLib.nsh"

; ── MUI Pages ────────────────────────────────────────────────────────────────
!define MUI_ABORTWARNING
!define MUI_ICON   "..\assets\lemmings.ico"
!define MUI_UNICON "..\assets\lemmings.ico"

!define MUI_WELCOMEPAGE_TITLE  "Welcome to the Lemmings Installer"
!define MUI_WELCOMEPAGE_TEXT   "This will install the Lemmings ADF port on your computer.$\n$\nYou will need to provide your own Lemmings.1of2.adf and Lemmings.2of2.adf disk images.$\nPlace them in the game folder after installation."

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN            "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT       "Launch Lemmings now"
!define MUI_FINISHPAGE_SHOWREADME     "$INSTDIR\README.txt"
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Show README"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; ── Install ───────────────────────────────────────────────────────────────────
Section "Lemmings (required)" SecMain
    SectionIn RO
    SetOutPath "$INSTDIR"

    ; Core executable
    File "..\target\x86_64-pc-windows-msvc\release\${APP_EXE}"

    ; SDL2 runtime DLLs (ship alongside .exe for portability)
    File "..\sdl2_dlls\SDL2.dll"
    File "..\sdl2_dlls\SDL2_mixer.dll"

    ; Readme
    File "..\README.md"
    Rename "$INSTDIR\README.md" "$INSTDIR\README.txt"

    ; Uninstaller
    WriteUninstaller "$INSTDIR\${UNINSTALLER}"

    ; Registry entries (Add/Remove Programs)
    WriteRegStr   HKLM "${REG_KEY}" "DisplayName"      "${APP_NAME}"
    WriteRegStr   HKLM "${REG_KEY}" "DisplayVersion"   "${APP_VERSION}"
    WriteRegStr   HKLM "${REG_KEY}" "Publisher"        "${APP_PUBLISHER}"
    WriteRegStr   HKLM "${REG_KEY}" "URLInfoAbout"     "${APP_URL}"
    WriteRegStr   HKLM "${REG_KEY}" "InstallLocation"  "$INSTDIR"
    WriteRegStr   HKLM "${REG_KEY}" "UninstallString"  '"$INSTDIR\${UNINSTALLER}"'
    WriteRegDWORD HKLM "${REG_KEY}" "NoModify"         1
    WriteRegDWORD HKLM "${REG_KEY}" "NoRepair"         1

    ; Start Menu shortcut
    CreateDirectory "$SMPROGRAMS\${APP_NAME}"
    CreateShortcut  "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" \
                    "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
    CreateShortcut  "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk" \
                    "$INSTDIR\${UNINSTALLER}"

    ; Desktop shortcut (optional)
    CreateShortcut  "$DESKTOP\${APP_NAME}.lnk" \
                    "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
SectionEnd

; ── Uninstall ─────────────────────────────────────────────────────────────────
Section "Uninstall"
    Delete "$INSTDIR\${APP_EXE}"
    Delete "$INSTDIR\SDL2.dll"
    Delete "$INSTDIR\SDL2_mixer.dll"
    Delete "$INSTDIR\README.txt"
    Delete "$INSTDIR\${UNINSTALLER}"

    ; Keep ADF files — user's own data
    RMDir  "$INSTDIR"

    Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
    Delete "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk"
    RMDir  "$SMPROGRAMS\${APP_NAME}"
    Delete "$DESKTOP\${APP_NAME}.lnk"

    DeleteRegKey HKLM "${REG_KEY}"
SectionEnd
