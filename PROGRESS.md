# ScreenExtend — Progreso y Estado del Proyecto

Documento de seguimiento del desarrollo de la aplicación nativa en Linux/Rust para extensión de escritorio Peer-to-Peer (P2P).

---

## 📌 Resumen de Arquitectura Implementada

La aplicación está construida como un **Workspace de Rust** modular dividido en 6 crates especializados:

1. **`screenextend-common`**:
   - Modelos de protocolo de señalización (`SignalingMessage`, `AnnouncePayload`, `PinRequest`, `SdpOffer`, `IceCandidate`, `Input`).
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
   - Integración con sesiones Wayland mediante captura directa de buffers PipeWire.

4. **`screenextend-streaming`**:
   - **Host Pipeline (GStreamer)**: Captura de pantalla de ultra-baja latencia (`ximagesrc` para X11 o `pipewiresrc` para Wayland), detección y selección automática de aceleración por hardware (`nvh264enc` para NVIDIA NVENC, `vaapih264enc` para Intel/AMD VA-API, y fallback `x264enc` ultrafast zero-latency para CPU).
   - Ajuste dinámico de bitrate en tiempo real (`set_bitrate(kbps)`).
   - Captura y codificación de audio con `opusenc` y transporte WebRTC (`webrtcbin`).
   - **Client Pipeline (GStreamer)**: Recepción por WebRTC (`webrtcbin`), demultiplexado dinámico al recibir pads (`rtph264depay` -> `avdec_h264` -> `videoconvert` -> `appsink` para video; `rtpopusdepay` -> `opusdec` -> `autoaudiosink` para audio).
   - Control de buffers para latencia mínima (`max-buffers=1`, `drop=true`, `sync=false`).
   - Negociador de señalización WebRTC (`negotiate_as_host`, `negotiate_as_client`, intercambio de ICE candidates).

5. **`screenextend-input`**:
   - Captura de eventos del lado cliente: movimiento relativo y absoluto del mouse, botones, scroll horizontal/vertical y teclas con scancodes de Linux.
   - Reenvío reactivo y continuo de eventos a través de canal asíncrono con descarte para evitar acumulación de lag.
   - Inyección en el host mediante `/dev/uinput` con el crate `evdev` (creación de teclado y mouse virtuales sin necesidad de correr la UI como root gracias a reglas udev).

6. **`screenextend-app`**:
   - Interfaz de usuario nativa moderna basada en **GTK4** y **Libadwaita** (`ViewSwitcher`, `PreferencesGroup`, `ActionRow`).
   - Vista **Host Mode**: Generación y visualización de PIN, estado de servicio con indicación del codificador activo (NVENC, VA-API o x264), inicio/parada de streaming.
   - Vista **Client Mode**: Lista reactiva de hosts descubiertos en LAN por mDNS, formulario de conexión manual por IP/PIN, viewport de video con aceleración de texturas de memoria (`gdk::MemoryTexture`) y controladores de entrada para captura en tiempo real.

---

## ✅ Tareas Realizadas

- [x] Configuración de dependencias de sistema (GStreamer 1.24 bad/base/good/ugly, GTK4, Libadwaita, Nice, SRTP, uinput).
- [x] Estructura del workspace de Rust con `Cargo.toml` unificado.
- [x] Reglas udev (`config/99-screenextend-uinput.rules`) y script de automatización (`scripts/setup_deps.sh`).
- [x] Implementación completa de `screenextend-common`.
- [x] Implementación completa de `screenextend-discovery` (mDNS y autenticación PIN con tests unitarios en loopback).
- [x] Implementación de `screenextend-display` (gestor de pantalla virtual con xrandr y fallback).
- [x] Implementación de `screenextend-streaming` (Host pipeline, Client pipeline, WebRTC signaling).
- [x] **Aceleración por Hardware (VA-API / NVENC)**: Conmutación automática a `nvh264enc` (NVIDIA NVENC) o `vaapih264enc` (VA-API) con fallback seguro a `x264enc` zero-latency.
- [x] **Ajuste Dinámico de Bitrate**: Control programático de bitrate en el codificador en caliente sin detener el pipeline (`set_bitrate`).
- [x] **Canal de Datos y Reenvío de Input**: Conexión de extremo a extremo desde los controladores de entrada de GTK en el cliente hasta la inyección en `/dev/uinput` en el host.
- [x] **Soporte de Fuentes Wayland**: Integración condicional de fuentes de captura `pipewiresrc` vs `ximagesrc` según `$XDG_SESSION_TYPE`.
- [x] **Empaquetado y Distribución**:
  - Manifiesto Flatpak (`build-aux/org.linux.screenextend.json`) con runtime GNOME 46.
  - Especificación de empaquetado Debian (`debian/control`, `debian/rules`).
  - Entrada de escritorio (`data/org.linux.screenextend.desktop`).
  - Icono SVG vectorial (`data/icons/hicolor/scalable/apps/org.linux.screenextend.svg`).
- [x] Implementación de `screenextend-app` (UI GTK4/Libadwaita, widget de video y orquestación de red).

---

## ⏳ Tareas Faltantes / Roadmap Futuro

- [ ] **Portal ScreenCast con Token de Sesión Persistente**: Guardar la autorización de screencast en GNOME Shell para reconexiones instantáneas sin cuadro de diálogo del portal.
- [ ] **DataChannel SCTP Nativo**: Migración del transporte de señalización de entrada TCP directamente a sub-canales SCTP dentro del peer connection WebRTC.
- [ ] **Compresión Avanzada AV1**: Añadir soporte de codificación AV1 (`svtav1enc` / `vaapiav1enc`) para redes con ancho de banda reducido.
