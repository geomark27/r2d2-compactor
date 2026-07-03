# Nota técnica: por qué `running` solo se apaga con `Msg::Finished`

Post-mortem de una condición de carrera entre el hilo de UI y el hilo de trabajo, y del invariante que la evita. **Si vas a tocar `poll()` o el manejo de `running` en `app.rs`, lee esto primero.**

## El invariante (regla que no se debe romper)

> El campo `running` de `App` se apaga **única y exclusivamente** cuando llega la señal explícita `Msg::Finished` desde el hilo de trabajo (o, como red de seguridad, cuando el canal se desconecta: `TryRecvError::Disconnected`).
>
> **Nunca** se debe inferir "la cola terminó" a partir del estado de los `jobs` (por ejemplo, "ninguno está `Processing`").

## El bug que motivó esta regla

Antes, `poll()` deducía el fin de la cola inspeccionando los estados de los jobs. El problema es de diseño: intentaba inferir un **evento discreto** ("terminó") a partir de un **muestreo periódico de estado** (el contenido de `jobs` en un instante). Eso tiene una ventana de ambigüedad.

### Secuencia problemática

`poll()` corre al inicio de cada `update()` (cada frame). `start_run()` corre más abajo en ese mismo `update()`, al procesar el clic. El hilo de trabajo arranca en paralelo, sin garantía de cuándo el SO le da CPU ni cuándo manda su primer mensaje.

- **Frame N (el del clic):** `start_run()` pone `running = true`, deja los jobs en `Queued`, crea el canal y hace `thread::spawn(...)`. El hilo **aún no ha corrido**.
- **Frame N+1 (microsegundos después; un clic o mover el mouse fuerza un repaint inmediato):**
  1. El hilo todavía no llegó a enviar su primer `Progress`.
  2. `poll()` no recibe nada.
  3. El viejo heurístico evaluaba: ¿ningún job `Processing`? sí. ¿`running`? sí. ¿todos `Queued`/`Done`/`Error`? sí → **daba "terminó"** aunque la compresión ni había empezado.
  4. `running = false`.

### Consecuencias

- El botón "Comprimir todo" se re-habilitaba (`can_run = !running && pending > 0 …`, y los jobs seguían `Queued`).
- Un **doble-clic** (o un repaint por mover el mouse) en esa ventana de milisegundos ejecutaba `start_run()` otra vez → `collect_pending()` volvía a tomar los mismos jobs `Queued` → **dos hilos** comprimiendo el mismo archivo:
  - Ambos escribían el mismo `{stem}_comp.mp4` con `-y` → salida corrupta o archivo bloqueado en Windows.
  - Ambos compartían el mismo `Arc<Mutex<Option<Child>>>`; el segundo hilo **pisaba** la referencia al proceso del primero, dejándolo huérfano de control (Cancelar solo mataba a uno).

## La corrección

1. **`Msg::Finished`** (`model.rs`): variante nueva del canal trabajo→UI.
2. **`run_queue`** (`queue.rs`) envía `Msg::Finished` al terminar el bucle — siempre, incluido tras un `break` por cancelación. Es el único camino de salida.
3. **`poll()`** (`app.rs`) drena el canal y decide con señales reales, no con el estado:
   - `Err(Empty)` → no hay nada este frame; `break` y **`running` sigue `true`**.
   - `Err(Disconnected)` → el hilo ya no existe (terminó o murió por panic sin enviar `Finished`) → `finished = true`.
   - `Msg::Finished` → `finished = true`.
   - Solo si `finished`: `running = false; rx = None`.

Con esto, en el Frame N+1 del escenario anterior `try_recv()` devuelve `Empty`, así que `running` permanece `true`, el botón sigue deshabilitado y el doble-submit es imposible.

## Por qué el fallback `Disconnected` importa

`Msg::Finished` cubre el fin normal. Pero si el hilo **entrara en panic** a mitad de un trabajo, nunca enviaría `Finished`; al soltarse el `Sender`, el canal se desconecta y `poll()` lo detecta, evitando que la UI quede colgada en `running = true` para siempre. Es una red de seguridad, no el camino principal.

## Defensa en profundidad (sigue vigente aparte de este arreglo)

- `start_run()` empieza con `if self.running { return; }`.
- El botón usa `can_run = !self.running && pending > 0 && !self.ffmpeg_missing`.
- `collect_pending()` solo toma jobs `Queued` (idempotencia, con tests en `queue.rs`).
- `add_file()` ignora rutas ya encoladas.

Ninguna de estas *reemplaza* al invariante de `Msg::Finished`; lo complementan.

## Regla práctica al editar

Si algún día `poll()` "parece" que podría simplificarse mirando los estados de los jobs para decidir el fin: **no**. Esa es exactamente la trampa que este documento existe para evitar. La fuente de verdad de "terminó" es el hilo que ejecuta el trabajo, y se comunica con `Msg::Finished`.
