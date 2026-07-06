# Guía de usuario — R2D2 Compactor

Herramienta para **reducir el peso de videos e imágenes de evidencia** antes de subirlos, sin perder calidad de forma notoria. Es un programa de escritorio para Windows: se abre, arrastras los archivos y los comprime.

---

## 1. Instalación (una sola vez)

1. Entra a la página de descargas del proyecto:
   **https://github.com/geomark27/r2d2-compactor/releases**
2. En la versión más reciente, descarga el **instalador**: **`r2d2-compactor-setup.exe`**.
3. Ábrelo con doble clic y sigue el asistente: **Siguiente → elige la carpeta (o deja la sugerida) → Instalar → Finalizar**. El instalador ya trae todo lo necesario (FFmpeg incluido); no hay que descargar nada más.
4. ¡Listo! Al finalizar puedes dejar marcada la casilla "Ejecutar R2D2 Compactor" para abrirlo de una vez.

> La primera vez, Windows puede mostrar un aviso de seguridad ("Windows protegió tu PC"). Es normal en programas nuevos: haz clic en **"Más información" → "Ejecutar de todas formas"**.

### Abrirlo desde el buscador de Windows

La instalación crea el acceso en el menú Inicio: pulsa la tecla **Windows** y escribe **`r2d2`** — aparecerá "R2D2 Compactor" para abrirlo directo.

### Desinstalar

Como cualquier programa: **Configuración → Aplicaciones → R2D2 Compactor → Desinstalar**.

### Alternativa portable (sin instalador)

Si prefieres no instalar nada, en la misma página hay un **`r2d2-compactor-windows-amd64.zip`**: descomprímelo completo en cualquier carpeta y abre `r2d2-compactor.exe`. En ese caso, **no separes** el `.exe` de la carpeta `ffmpeg` que viene junto a él.

---

## 2. La ventana explicada

Cuando abres el programa verás estos controles en la parte de arriba:

### 🎯 Tamaño objetivo (MB)
El **peso máximo** que quieres que tenga cada archivo comprimido.
- Por defecto son **90 MB** (pensado para quedar por debajo del límite de 100 MB de SharePoint).
- Si pones un número **menor**, el archivo pesará menos pero perderá algo de calidad.
- Si pones un número **mayor**, pesará más pero se verá mejor.

### 📐 Resolución máxima
Reduce las **dimensiones** (el ancho y alto) para ahorrar peso.
- **1080p (Full HD)** — opción recomendada, buena calidad.
- **720p (HD)** — más liviano, calidad algo menor.
- **Original (sin cambiar tamaño)** — mantiene las dimensiones tal cual.
- 👉 Nunca **agranda**: si tu archivo ya es más pequeño que lo elegido, se deja como está.

### 📁 Carpeta de salida
Dónde se guardan los archivos comprimidos.
- Por defecto: **"Misma carpeta del original"** (deja el comprimido junto al archivo de origen).
- Con **"Elegir…"** puedes mandarlos a otra carpeta.
- El botón **↺** vuelve a la opción por defecto.

> El archivo original **nunca se modifica ni se borra**. Siempre se crea una copia nueva con el sufijo **`_comp`** (por ejemplo, `evidencia.mp4` → `evidencia_comp.mp4`).

---

## 3. Cómo comprimir (paso a paso)

1. **Arrastra** uno o varios archivos a la ventana (puedes mezclar videos e imágenes). También puedes usar el botón **"Elegir archivos…"**.
2. Ajusta el **tamaño objetivo** y la **resolución** si lo necesitas. Los valores por defecto funcionan bien para la mayoría de los casos.
3. Elige la **carpeta de salida** (o déjala en "misma carpeta del original").
4. Haz clic en **"Comprimir todo"**.
5. Verás una **barra de progreso** por cada archivo. Los videos pasan por dos fases: *Analizando* y *Comprimiendo*.
6. Al terminar, cada archivo muestra cuánto se redujo (por ejemplo, *"Listo · 78% más liviano"*) y aparece un enlace **"Ver archivo"** que abre la carpeta con el resultado.

Puedes pulsar **"Cancelar"** en cualquier momento para detener el proceso.

### Limpiar la lista

Encima de la lista de archivos tienes dos botones para no borrar uno por uno:
- **🧹 Quitar terminados** — quita de la lista los que ya se comprimieron (o fallaron), dejando los pendientes.
- **🗑 Quitar todos** — vacía la lista completa (los que se estén comprimiendo en ese momento se conservan).

Quitar un archivo de la lista **no borra nada de tu disco**; solo lo saca de la cola del programa.

---

## 4. Formatos soportados

- **Videos:** MP4, MOV, AVI, MKV, M4V, WMV
- **Imágenes:** JPG, PNG, WEBP, BMP, TIFF

Las imágenes se comprimen buscando automáticamente la mejor calidad que quepa dentro del tamaño objetivo que elegiste.

---

## 5. Actualizaciones automáticas

No tienes que estar pendiente de descargar versiones nuevas. **Cada vez que abres el programa**, revisa solo si hay una versión más reciente:

- Si la hay, aparece un aviso verde con el botón **"Actualizar ahora"**.
- Al pulsarlo, el programa se actualiza solo (descarga y verifica la nueva versión).
- Cuando termine, **cierra y vuelve a abrir** el programa. ¡Listo!

No necesitas volver a descargar el `.zip` ni tocar la carpeta `ffmpeg`.

---

## 6. Problemas frecuentes

**Aparece un mensaje rojo: "No se encontró FFmpeg".**
Significa que el programa no encuentra la carpeta `ffmpeg` que debe estar junto a él.
- Si lo instalaste con el **instalador**: vuelve a ejecutar `r2d2-compactor-setup.exe` (reinstala encima y repone lo que falte).
- Si usas la versión **portable** (zip): vuelve a descomprimir el `.zip` completo y abre el `.exe` desde ahí, con la carpeta `ffmpeg` al lado; no muevas el `.exe` solo.

**Un video muy largo no baja al tamaño que puse.**
Si el video es muy largo para un objetivo muy pequeño, el programa te avisará que podría superarlo (hay un límite para que no se vea demasiado mal). Prueba con un tamaño objetivo un poco mayor o una resolución menor (720p).

**Una imagen no baja al tamaño objetivo.**
Si ni con la máxima compresión se logra, el programa lo avisa. Prueba eligiendo una **resolución máxima** menor (720p).

**Windows bloquea el programa al abrirlo.**
Es el aviso normal de SmartScreen para programas nuevos: **"Más información" → "Ejecutar de todas formas"**.

---

¿Dudas o algo no funciona como esperas? Avisa al equipo de desarrollo.
