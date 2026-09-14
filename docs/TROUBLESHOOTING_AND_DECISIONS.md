# ScreenExtend — Registro de Decisiones Técnicas y Resolución de Problemas

Este documento recopila las lecciones aprendidas, problemas críticos enfrentados durante el desarrollo y las decisiones de diseño tomadas para superarlos. Servirá de referencia indispensable para cualquier persona que colabore en el proyecto.

---

## 1. El Conflicto con `webrtcbin` y `libnice` (GStreamer)

### Síntoma
Al presionar **Start Sharing** en el Host o al intentar iniciar la negociación WebRTC, el pipeline fallaba con el siguiente error:
```text
Host service error: Failed to parse host pipeline: no se pudo enlazar rtph264pay0 a sendrecv, 
sendrecv no puede manejar la capacidad application/x-rtp, media=(string)video, encoding-name=(string)H264, payload=(int)96
```

### Causa Raíz
El elemento `webrtcbin` de GStreamer no implementa internamente el protocolo ICE (Interactive Connectivity Establishment), sino que delega la generación de candidatos y la gestión de sockets en la librería `libnice` a través de los elementos `nicesink` y `nicesrc`.
Cuando el paquete `gstreamer1.0-nice` **no está instalado**, `webrtcbin` no puede crear pads de recepción dinámicos (`sink_%u`) para flujos RTP. Al fallar la creación del pad, `gst_parse_launch` asume que las capacidades son incompatibles y aborta.

### Solución y Decisión
1. Se añadió `gstreamer1.0-nice` como dependencia obligatoria en:
   - `scripts/setup_deps.sh`
   - `debian/control`
   - `README.md`
2. Se implementó una verificación explícita en `scripts/setup_deps.sh`:
   ```bash
   gst-inspect-1.0 --exists nicesink && echo "✓ nicesink (libnice ICE agent)"
   ```

---

## 2. Incompatibilidad de Drivers NVIDIA Blackwell / 595+ con `nvh264enc`

### Síntoma
En sistemas con GPU NVIDIA moderna (como RTX 5050 con driver 595.84) el pipeline fallaba con:
```text
Host service error: Failed to set pipeline to Playing: Element failed to change its state
...
ERROR: del elemento /GstPipeline:pipeline0/GstNvH264Enc:nvh264enc0: No se pudo configurar la biblioteca de soporte.
Selected preset not supported
```

### Causa Raíz
A partir de los drivers NVIDIA 595+ y el SDK NVENC 13+, NVIDIA depreció y eliminó los presets antiguos (como `low-latency-hq`). El plugin `nvcodec` de GStreamer 1.24.2 todavía intenta enviar los GUIDs antiguos, provocando que la inicialización del hardware falle inmediatamente al pasar al estado `GST_STATE_PAUSED`.

### Solución y Decisión
No podemos asumir ciegamente que porque `/dev/nvidia0` y el plugin `nvh264enc` existan, el encoder va a funcionar.
Se introdujo una función de **prueba dinámica en caliente** (`test_encoder`) en [`crates/streaming/src/host_pipeline.rs`](file:///home/javier-espana/Escritorio/RemoteDesktopApp/crates/streaming/src/host_pipeline.rs):
```rust
fn test_encoder(desc: &str) -> bool {
    let _ = gstreamer::init();
    if let Ok(pipeline) = gstreamer::parse::launch(desc) {
        let res = pipeline.set_state(gstreamer::State::Paused);
        let _ = pipeline.set_state(gstreamer::State::Null);
        res.is_ok()
    } else {
        false
    }
}
```
Si la prueba del pipeline con `nvh264enc` falla, el sistema emite una advertencia y recurre de manera transparente a `x264enc` (CPU con perfil `tune=zerolatency speed-preset=ultrafast`), el cual funciona sin latencia perceptible en redes LAN.

---

## 3. Descubrimiento mDNS y Selección de Interfaces de Red

### Síntoma
La laptop cliente no listaba al Host en la interfaz de usuario, o el servicio mDNS reportaba error al registrarse.

### Causa Raíz 1: Formato FQDN en el hostname
`mdns-sd` exige estrictamente que el nombre del host termine en un dominio completamente calificado con punto final (`.local.`). Si se registraba como `javier-espana`, la librería fallaba con `Hostname must end with '.local.'`.

### Causa Raíz 2: Registro con IP vacía (`""`)
En `ServiceInfo::new`, se pasaba `""` esperando que la librería descubriera las IPs automáticamente. La implementación de `mdns-sd` trata una cadena vacía como un conjunto vacío de direcciones (`HashSet::new()`). Como consecuencia, la laptop recibía el paquete mDNS pero descartaba el host con el mensaje `Service has no addresses`.

### Causa Raíz 3: Descarte del objeto `DiscoveryService` en el Cliente
En `client_view.rs`, la variable `discovery` se instanciaba dentro del método `start_discovery(&self)` y se destruía (Drop) al terminar la función, cerrando prematuramente el daemon de búsqueda.

### Solución y Decisión
1. Normalización del hostname:
   ```rust
   let host_name = format!("{}.local.", hostname.trim_end_matches('.').trim_end_matches(".local"));
   ```
2. Extracción de IPs reales con `if_addrs`: Filtramos interfaces virtuales (como `docker0`, `br-*`, `veth`) para enviar únicamente la IP física del adaptador Wi-Fi o Ethernet (`192.168.x.x` / `10.x.x.x`).
3. Retención de `DiscoveryService`: Se guarda una referencia en `ClientView` dentro de un `Arc<Mutex<Option<DiscoveryService>>>` para mantener la escucha activa durante toda la vida útil de la pantalla.

---

## 4. Cadena Literal `$DISPLAY` en `ximagesrc`

### Síntoma
Al ejecutar el pipeline en X11, se producía un error de sintaxis en `ximagesrc display-name=$DISPLAY`.

### Causa Raíz
`gstreamer::parse::launch` **no** es una shell de Bash; no expande variables de entorno como `$DISPLAY`. Pasarle `display-name=$DISPLAY` provocaba que `ximagesrc` intentara conectarse literalmente a una pantalla llamada `"$DISPLAY"`.

### Solución y Decisión
Se removió el atributo `display-name`. Por omisión, el plugin `ximagesrc` consulta la variable de entorno del proceso de manera nativa sin necesidad de forzar el parámetro en el string del pipeline.

---

## 5. Permisos de Usuario para Inyección de Input (`/dev/uinput`)

### Síntoma
El Host no podía crear los dispositivos virtuales de mouse y teclado a través de `evdev`, requiriendo permisos de superusuario (`root`).

### Causa Raíz
Por motivos de seguridad, el dispositivo de kernel `/dev/uinput` tiene permisos restrictivos `0660 root:root` por defecto en la mayoría de distribuciones Linux.

### Solución y Decisión
1. Creamos la regla udev [`config/99-screenextend-uinput.rules`](file:///home/javier-espana/Escritorio/RemoteDesktopApp/config/99-screenextend-uinput.rules):
   ```udev
   KERNEL=="uinput", GROUP="input", MODE="0660"
   ```
2. En `scripts/setup_deps.sh`, automatizamos la copia a `/etc/udev/rules.d/`, la recarga de udev y la inclusión del usuario actual en el grupo del sistema `input`:
   ```bash
   sudo usermod -aG input "$USER"
   ```

## 6. Errores de mDNS al cerrar y nombres de host largos

### Síntomas

```text
Service name length must be <= 15 bytes
unregister: cannot find such service
failed to send response: sending on a closed channel
```

### Causa y solución

`mdns-sd` limita la etiqueta de instancia a 15 bytes. El nombre anterior incluía
el hostname completo, por lo que algunos equipos no podían registrarse. Ahora la
instancia usa el formato corto `se-XXXXXXXX`, derivado de un hash estable del
hostname. El cierre también tolera que el servicio no haya llegado a registrarse o
que el daemon ya esté cerrando.

## 7. Estado actual de Wayland

La captura Wayland usa `pipewiresrc` y los portales de escritorio. No existe una API
única para crear un monitor virtual en GNOME, KDE, wlroots y otros compositores, por
lo que esta versión no puede prometer todavía una pantalla extendida real en todas
las sesiones Wayland. En ese caso debe usarse X11 para la salida virtual, o continuar
el trabajo específico del compositor antes de habilitar una disposición izquierda /
derecha.

El siguiente paso técnico es separar el contrato de captura del contrato de salida:
la pantalla remota debe anunciar resolución y posición, y el Host debe aplicar esa
geometría al backend de display antes de iniciar la captura y la inyección de input.
