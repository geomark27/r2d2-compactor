; Instalador de R2D2 Compactor (NSIS 3, interfaz MUI2 en español).
;
; Se compila desde el Makefile con:
;   makensis -DVERSION=x.y.z -DPKG_DIR=dist/pkg -DOUTFILE=dist/r2d2-compactor-setup.exe \
;            [-DAPP_ICO=ruta/icon.ico] installer/installer.nsi
;
; Instala POR USUARIO (sin permisos de administrador) en
; $LOCALAPPDATA\Programs\R2D2 Compactor — igual que VS Code/Chrome/Discord.
; Esto es deliberado: la auto-actualización de la app reemplaza el .exe en su
; sitio, y en Program Files eso fallaría por permisos.

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!define APP_NAME "R2D2 Compactor"
!define APP_EXE "r2d2-compactor.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\R2D2Compactor"

Name "${APP_NAME} v${VERSION}"
OutFile "${OUTFILE}"
; Default en la raíz de C: (decisión del proyecto). Nota: Windows permite a los
; usuarios crear carpetas ahí sin admin; si una política corporativa lo bloquea,
; la validación DirLeave lo detecta y sugiere otra carpeta.
InstallDir "C:\r2d2-compactor"
; Recuerda la carpeta si se reinstala/actualiza.
InstallDirRegKey HKCU "Software\R2D2Compactor" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

; Metadatos visibles en las propiedades del setup.exe.
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1034 "ProductName" "${APP_NAME}"
VIAddVersionKey /LANG=1034 "FileDescription" "Instalador de ${APP_NAME}"
VIAddVersionKey /LANG=1034 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1034 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1034 "LegalCopyright" ""

!define MUI_ABORTWARNING
!ifdef APP_ICO
  !define MUI_ICON "${APP_ICO}"
  !define MUI_UNICON "${APP_ICO}"
!endif

; ---- Páginas del asistente ----
!define MUI_WELCOMEPAGE_TITLE "Bienvenido a ${APP_NAME}"
!define MUI_WELCOMEPAGE_TEXT "Este asistente instalará ${APP_NAME} v${VERSION} en tu equipo.$\r$\n$\r$\nIncluye todo lo necesario (FFmpeg viene integrado); no hace falta descargar nada más.$\r$\n$\r$\nHaz clic en Siguiente para continuar."
!insertmacro MUI_PAGE_WELCOME
; Valida que la carpeta elegida sea escribible ANTES de instalar; si no lo es
; (p. ej. Program Files, que requiere administrador), avisa claro y pide otra.
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE DirLeave
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Ejecutar ${APP_NAME}"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Spanish"

; Comprueba con un archivo de prueba que la carpeta elegida admite escritura.
; Si no, no deja avanzar y explica el porqué (evita el error críptico
; "error abriendo archivo para escritura" a mitad de la instalación).
Function DirLeave
  ClearErrors
  CreateDirectory "$INSTDIR"
  FileOpen $0 "$INSTDIR\.escritura-test" w
  IfErrors 0 dir_ok
    RMDir "$INSTDIR" ; limpia la carpeta solo si quedó vacía
    MessageBox MB_OK|MB_ICONEXCLAMATION \
      "No se puede escribir en:$\r$\n$INSTDIR$\r$\n$\r$\nEsa carpeta requiere permisos de administrador (como Program Files) o está protegida.$\r$\n$\r$\nElige otra carpeta — se recomienda la sugerida por defecto — para que la instalación y las actualizaciones automáticas funcionen."
    Abort
  dir_ok:
  FileClose $0
  Delete "$INSTDIR\.escritura-test"
FunctionEnd

; ---- Instalación ----
Section "Instalar"
  SetOutPath "$INSTDIR"
  File "${PKG_DIR}/${APP_EXE}"
  SetOutPath "$INSTDIR\ffmpeg"
  File /r "${PKG_DIR}/ffmpeg/"

  ; Acceso directo del menú Inicio (buscable escribiendo "r2d2").
  CreateShortCut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0

  ; Desinstalador + entrada en "Agregar o quitar programas".
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\R2D2Compactor" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${APP_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" $0
SectionEnd

; ---- Desinstalación ----
Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  RMDir /r "$INSTDIR\ffmpeg"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\R2D2Compactor"
SectionEnd
