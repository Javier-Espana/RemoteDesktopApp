# ScreenExtend — Progreso y Estado del Proyecto

Documento de seguimiento del desarrollo de la aplicación nativa en Linux/Rust para extensión de escritorio Peer-to-Peer (P2P).

---

## 📌 Resumen de Arquitectura Implementada

La aplicación está construida como un **Workspace de Rust** modular dividido en 6 crates especializados:

1. **`screenextend-common`**:
   - Modelos de protocolo de señalización (`SignalingMessage`, `AnnouncePayload`, `PinRequest`, `SdpOffer`, `IceCandidate`).
   - Serialización y deserialización de eventos de entrada (`InputEvent`, `MouseButton`).
   - Detección de sesión Linux en runtime (`X11` vs `Wayland` vía `$XDG_SESSION_TYPE`).
   - Configuración global (`AppConfig`, resoluciones estándar).

2. **`screenextend-discovery`**:
   - Descubrimiento local sin servidor central vía **mDNS** (`_linux-screenextend._tcp.local.`) con `mdns-sd`.
   - Handshake TCP con **autenticación por PIN de 4 dígitos** generado dinámicamente en el Host.
   - Canal de señalización TCP bidireccional mediante JSON delimitado por saltos de línea (`SignalingChannel`).

3. **`screenextend-display`**:
   - Abstracción de pantalla virtual (`VirtualDisplay` trait).
   - Backend X11 con `xrandr`: detección del monitor primario, soporte para outputs desconectados (`VIRTUAL`, `DUMMY`, `HDMI`) y fallback para captura de región.
   - Limpieza automática al cerrar la sesión (`Drop` trait).
   - Módulo preparado para headless outputs de Wayland (`mutter` / `wlroots`).

4. **`screenextend-streaming`**:
   - **Host Pipeline (GStreamer)**: Captura de pantalla de ultra-baja latencia (`ximagesrc` / `pipewiresrc`), codificación H.264 (`x264enc` con `tune=zerolatency`, `speed-preset=ultrafast`, `bframes=0`), captura y codificación de audio con `opusenc`, y transporte WebRTC (`webrtcbin`).
   - **Client Pipeline (GStreamer)**: Recepción por WebRTC (`webrtcbin`), demultiplexado dinámico al recibir pads (`rtph264depay` -> `avdec_h264` -> `videoconvert` -> `appsink` para video; `rtpopusdepay` -> `opusdec` -> `autoaudiosink` para audio).
   - Control de buffers para latencia mínima (`max-buffers=1`, `drop=true`, `sync=false`).
   - Negociador de señalización WebRTC (`negotiate_as_host`, `negotiate_as_client`, intercambio de ICE candidates).

5. **`screenextend-input`**:
   - Captura de eventos del lado cliente: movimiento relativo y absoluto del mouse, botones, scroll horizontal/vertical y teclas con scancodes de Linux.
   - Inyección en el host mediante `/dev/uinput` con el crate `evdev` (creación de teclado y mouse virtuales sin necesidad de correr la UI como root gracias a reglas udev).

6. **`screenextend-app`**:
   - Interfaz de usuario nativa moderna basada en **GTK4** y **Libadwaita** (`ViewSwitcher`, `PreferencesGroup`, `ActionRow`).
   - Vista **Host Mode**: Generación y visualización de PIN, estado de servicio, inicio/parada de streaming.
   - Vista **Client Mode**: Lista reactiva de hosts descubiertos en LAN por mDNS, formulario de conexión manual por IP/PIN, viewport de video con aceleración de texturas de memoria (`gdk::MemoryTexture`) y controladores de entrada para captura en tiempo real.

---

## ✅ Tareas Realizadas

- [x] Configuración de dependencias de sistema (GStreamer 1.24 bad/base/good/ugly, GTK4, Libadwaita, Nice, SRTP, uinput).
- [x] Estructura del workspace de Rust con `Cargo.toml` unificado.
- [x] Reglas udev (`config/99-screenextend-uinput.rules`) y script de automatización (`scripts/setup_deps.sh`).
- [x] Implementación completa de `screenextend-common`.
- [x] Implementación completa de `screenextend-discovery` (mDNS y autenticación PIN).
- [x] Implementación de `screenextend-display` (gestor de pantalla virtual con xrandr y fallback).
- [x] Implementación de `screenextend-streaming` (Host pipeline, Client pipeline, WebRTC signaling).
- [x] Implementación de `screenextend-input` (Captura en cliente e inyección en host vía `/dev/uinput`).
- [x] Implementación de `screenextend-app` (UI GTK4/Libadwaita, widget de video y orquestación de red).

---

## ⏳ Tareas Faltantes / Roadmap Futuro

- [ ] **Soporte Wayland Nativo**: Integración con D-Bus Screencast Portal (`org.freedesktop.portal.ScreenCast`) y `libei` para Wayland puro sin XWayland.
- [ ] **Aceleración por Hardware (VA-API / NVENC)**: Conmutación automática a `vaapih264enc` o `nvh264enc` cuando estén disponibles en el hardware del host.
- [ ] **Canal de Datos WebRTC (SCTP DataChannel)**: Mover el transporte de `InputEvent` del canal TCP de señalización directamente a un DataChannel SCTP de WebRTC para menor jitter.
- [ ] **Ajuste Dinámico de Bitrate**: Detección de pérdida de paquetes en tiempo real para adaptar el bitrate sobre Wi-Fi.
- [ ] **Empaquetado**: Crear manifiesto Flatpak o paquete `.deb` para distribución en Ubuntu.
