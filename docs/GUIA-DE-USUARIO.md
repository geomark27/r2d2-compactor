# Guía de usuario — R2D2 Compactor

Herramienta para **reducir el peso de videos e imágenes de evidencia** antes de subirlos, sin perder calidad de forma notoria. Es un programa de escritorio para Windows: se abre, arrastras los archivos y los comprime.

---

## 1. Instalación (una sola vez)

1. Entra a la página de descargas del proyecto:
   **https://github.com/geomark27/r2d2-compactor/releases**
2. En la versión más reciente, descarga el archivo **`r2d2-compactor-windows-amd64.zip`**.
3. **Descomprime el `.zip` completo** en la carpeta donde quieras tenerlo (por ejemplo, en tu Escritorio o en Documentos). Al descomprimir verás algo así:

   ```
   📁 r2d2-compactor
   ├── r2d2-compactor.exe      ← el programa
   └── 📁 ffmpeg               ← el motor de compresión (ya incluido)
   ```

4. Abre **`r2d2-compactor.exe`** con doble clic.

> ⚠️ **Importante:** no separes el `r2d2-compactor.exe` de la carpeta `ffmpeg`. Deben quedar siempre juntos, tal como vienen en el `.zip`. Si mueves solo el `.exe` a otro lado, el programa no podrá comprimir.

### Abrirlo después desde el buscador de Windows

La **primera vez** que abres el programa, este se registra solo en el menú Inicio. A partir de ahí ya **no necesitas ir a buscar el `.exe`**: solo pulsa la tecla **Windows** (o clic en el buscador) y escribe **`r2d2`** — aparecerá "R2D2 Compactor" para abrirlo directo.

> La primera vez, Windows puede mostrar un aviso de seguridad ("Windows protegió tu PC"). Es normal en programas nuevos: haz clic en **"Más información" → "Ejecutar de todas formas"**.

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
Significa que el programa no encuentra la carpeta `ffmpeg`. Casi siempre es porque:
- No se descomprimió el `.zip` completo, o
- Se movió el `.exe` a otra ubicación sin la carpeta `ffmpeg`.

Solución: vuelve a descomprimir el `.zip` completo y abre el `.exe` desde ahí, con la carpeta `ffmpeg` al lado.

**Un video muy largo no baja al tamaño que puse.**
Si el video es muy largo para un objetivo muy pequeño, el programa te avisará que podría superarlo (hay un límite para que no se vea demasiado mal). Prueba con un tamaño objetivo un poco mayor o una resolución menor (720p).

**Una imagen no baja al tamaño objetivo.**
Si ni con la máxima compresión se logra, el programa lo avisa. Prueba eligiendo una **resolución máxima** menor (720p).

**Windows bloquea el programa al abrirlo.**
Es el aviso normal de SmartScreen para programas nuevos: **"Más información" → "Ejecutar de todas formas"**.

---

¿Dudas o algo no funciona como esperas? Avisa al equipo de desarrollo.
