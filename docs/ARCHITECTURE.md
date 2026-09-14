# ScreenExtend — Arquitectura del Sistema y Diseño Técnico

Este documento detalla el diseño de arquitectura, el flujo de datos y las responsabilidades de cada componente de **ScreenExtend** para permitir que cualquier desarrollador o colaborador comprenda e itere sobre el proyecto.

---

## 1. Visión General

**ScreenExtend** es una utilidad nativa en Linux desarrollada en **Rust** que permite extender la pantalla de una computadora de escritorio o laptop hacia otra máquina en la misma red local (LAN) en modo **Peer-to-Peer (P2P)**, sin depender de servidores centrales ni servicios en la nube.

### Objetivos Clave
- **Latencia ultra-baja**: Menor a 30-50 ms en Wi-Fi / Ethernet local mediante WebRTC (`webrtcbin`) y GStreamer con codificación H.264 acelerada por hardware o con perfil `zerolatency`.
- **Experiencia de usuario nativa**: Interfaz moderna GTK4 + Libadwaita integrada en el entorno GNOME / Linux.
- **Configuración cero**: Descubrimiento dinámico de máquinas en la red vía mDNS (`_screenextend._tcp.local.`) y enlace seguro mediante PIN efímero de 4 dígitos.
- **Control bidireccional de periféricos**: Reenvío en tiempo real de eventos de ratón (movimiento, clics, scroll) y teclado desde la máquina cliente hacia el host con inyección a nivel de kernel vía `/dev/uinput`.

---

## 2. Diagrama de Arquitectura Global

```text
+-------------------------------------------------------------------------------+
|                                  HOST (PC)                                    |
|                                                                               |
|  +--------------------+         +------------------------------------------+  |
|  |   host_view.rs     |         |             HandshakeServer              |  |
|  |  (GTK4/Libadwaita) |         |             (TCP Port: 9876)             |  |
|  +---------+----------+         +--------------------+---------------------+  |
|            |                                         |                        |
|            v                                         v                        |
|  +--------------------+         +--------------------+---------------------+  |
|  |  DiscoveryService  |         |             SignalingChannel             |  |
|  | (mDNS Registration)|         |            (TCP JSON framing)            |  |
|  +---------+----------+         +---------+--------------------+-----------+  |
|            |                              |                    |              |
|            v                              v                    v              |
|   mDNS Broadcast            WebRTC SDP Offer / ICE     InputEvent Packet      |
|  UDP Port: 5353             Exchange over TCP :9876    Forwarding over TCP    |
|            |                              |                    |              |
+------------|------------------------------|--------------------|--------------+
             |                              |                    |
       Local Network                        |                    |
       (LAN / Wi-Fi)                        |                    |
             |                              |                    |
+------------|------------------------------|--------------------|--------------+
|            |                              |                    |              |
|            v                              v                    v              |
|  +---------+----------+         +---------+----------+  +------+-----------+  |
|  |  DiscoveryService  |         |  HandshakeClient   |  |   InputCapture   |  |
|  |    (mDNS Browse)   |         |    (TCP Client)    |  |  (GTK Controllers|  |
|  +---------+----------+         +---------+----------+  +------+-----------+  |
|            |                              |                    ^              |
|            v                              v                    |              |
|  +---------+----------+         +---------+----------+         |              |
|  |  client_view.rs    +-------->|   ClientPipeline   +---------+              |
|  |  (Host Selection)  |         |     (GStreamer)    |  User interaction      |
|  +--------------------+         +---------+----------+                        |
|                                           |                                   |
|                                           v                                   |
|                                 +---------+----------+                        |
|                                 |    VideoWidget     |                        |
|                                 | (gdk::MemoryTexture|                        |
|                                 +--------------------+                        |
|                                                                               |
|                                 CLIENT (Laptop)                               |
+-------------------------------------------------------------------------------+
```

---

## 3. División de Crates del Workspace

El proyecto está estructurado como un **Cargo Workspace** de 6 crates modulares e independientes:

| Crate | Directorio | Responsabilidad Principal | Dependencias Clave |
|---|---|---|---|
| `screenextend-common` | `crates/common` | Protocolo de mensajes, tipos compartidos, detección del servidor gráfico (`X11` vs `Wayland`), configuración. | `serde`, `serde_json`, `hostname`, `thiserror` |
| `screenextend-discovery` | `crates/discovery` | Anuncio y búsqueda de peers en LAN por mDNS, handshake TCP y autenticación por PIN dinámico. | `mdns-sd`, `if-addrs`, `tokio`, `rand` |
| `screenextend-display` | `crates/display` | Gestión de pantallas virtuales (`xrandr` en X11, backend dummy y soporte futuro de headless/Wayland). | `tokio`, `anyhow` |
| `screenextend-streaming` | `crates/streaming` | Creación y gestión de pipelines GStreamer para Host y Cliente, detección de codificadores H.264, negociación WebRTC. | `gstreamer`, `gstreamer-webrtc`, `gstreamer-video`, `gstreamer-app` |
| `screenextend-input` | `crates/input` | Captura de entradas en el cliente e inyección virtual de teclado/mouse en el host mediante `/dev/uinput`. | `evdev`, `tokio` |
| `screenextend-app` | `crates/app` | Aplicación visual en GTK4 y Libadwaita que coordina todas las capas anteriores. | `gtk4`, `libadwaita`, `gstreamer` |

---

## 4. Flujo de Comunicación y Estados

### Paso 1: Descubrimiento (mDNS)
1. El **Host** inicia y llama a `DiscoveryService::register(port, is_host: true)`.
2. Se resuelve la dirección IP física real de la LAN (priorizando `192.168.x.x` o `10.x.x.x` sobre adaptadores virtuales como Docker).
3. Se publica el servicio mDNS `_screenextend._tcp.local.` con metadatos (nombre de host, versión, rol).
4. La **Laptop/Cliente** ejecuta `DiscoveryService::start_browsing()` para recibir eventos de red y poblar la lista de hosts disponibles.

### Paso 2: Handshake y Autenticación por PIN
1. El Host levanta un listener TCP en el puerto `9876` (`HandshakeServer`) y genera un PIN aleatorio de 4 dígitos (ej: `4821`).
2. El Cliente se conecta por TCP y envía un mensaje `PinRequest { pin: "4821" }`.
3. Si el PIN coincide, el Host responde `PinResponse::Success` y se establece un canal enmarcado de mensajes (`SignalingChannel`).

### Paso 3: Negociación WebRTC y Streaming
1. El Host inicializa su `HostPipeline` (detectando dinámicamente si el codificador H.264 por hardware funciona o usando fallback software).
2. El Host crea una SDP Offer con `webrtcbin` y la envía al Cliente mediante el canal de señalización.
3. El Cliente aplica la Remote Description en su `ClientPipeline`, crea una SDP Answer y la devuelve.
4. Ambos extremos intercambian candidatos ICE a través de `SignalingMessage::IceCandidate` utilizando `gstreamer1.0-nice`.
5. Se establece el canal de medios P2P; el Host envía video H.264 y audio Opus.

### Paso 4: Reenvío e Inyección de Periféricos (Input)
1. En el Cliente, los `EventControllerMotion`, `GestureClick`, `EventControllerScroll` y `EventControllerKey` de GTK4 capturan las interacciones del usuario en el widget de video.
2. Estos eventos se serializan como `InputEvent` y viajan por el `SignalingChannel` TCP.
3. En el Host, `InputInjector` recibe los eventos y los traduce a ioctls de Linux mediante `evdev`, simulando un ratón y teclado de hardware en `/dev/uinput`.
