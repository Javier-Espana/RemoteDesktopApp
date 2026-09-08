# ScreenExtend 🖥️➡️💻

Una aplicación **P2P nativa para Linux (Ubuntu/Debian)** que extiende el escritorio a otra computadora en la red local con sincronización en tiempo real, latencia ultra-baja ($\le 16\text{ ms}$ para $60\text{ fps}$), transmisión de video/audio y reenvío fluido de mouse y teclado.

---

## 🌟 Características Principales

* **Descubrimiento Automático LAN (mDNS):** Encuentra automáticamente instancias de la app en la misma red local (`_linux-screenextend._tcp.local.`) sin necesidad de configurar direcciones IP manualmente.
* **Emparejamiento Seguro con PIN:** Código dinámico de 4 dígitos generado en el Host para autorizar la conexión.
* **Streaming de Ultra-Baja Latencia (WebRTC + GStreamer):**
  * Video: Captura de pantalla eficiente (`ximagesrc` / `pipewiresrc`), codificación H.264 optimizada sin frames B (`tune=zerolatency`, `speed-preset=ultrafast`) y payload RTP directo.
  * Audio: Captura de audio del sistema codificado en Opus (128 kbps).
  * Renderizado: Pipeline directo de texturas en memoria (`gdk::MemoryTexture`) en GTK4 con política de descarte de frames (*zero accumulated lag*).
* **Control Remoto Fluido (uinput):**
  * Captura de gestos en el cliente (movimiento absoluto/relativo, botones de mouse, scroll y teclado).
  * Inyección a nivel kernel en el host mediante `/dev/uinput` con soporte para permisos de usuario sin necesidad de ejecutar la app como `root`.
* **Interfaz Gráfica Nativa Moderna:** Construida en **GTK4** y **Libadwaita** adaptada al entorno de escritorio GNOME / Ubuntu.

---

## 🏗️ Arquitectura del Sistema

```text
[ Máquina Host ]                                      [ Máquina Cliente ]
+-----------------------------------+                +-----------------------------------+
| Módulo 1: mDNS & Handshake PIN    | <--- TCP/UDP --| Módulo 1: mDNS & Handshake PIN    |
| Módulo 2: Virtual Display (xrandr)|                |                                   |
| Módulo 3: Media Stream (H264/Opus)| --- WebRTC --->| Módulo 3: Renderizado (GTK4/GDK)  |
| Módulo 4: Input Injection (uinput)| <--- Input ----| Módulo 4: Input Capture (GTK)     |
+-----------------------------------+                +-----------------------------------+
```

### Estructura del Workspace Rust

```text
RemoteDesktopApp/
├── Cargo.toml                          # Workspace root
├── README.md                           # Documentación principal
├── PROGRESS.md                         # Registro de tareas realizadas y roadmap
├── config/
│   └── 99-screenextend-uinput.rules    # Regla udev para /dev/uinput
├── scripts/
│   └── setup_deps.sh                   # Script instalador de dependencias
└── crates/
    ├── common/       # Protocolo, serialización, tipos compartidos y config
    ├── discovery/    # Descubrimiento mDNS y autenticación TCP con PIN
    ├── display/      # Gestor de monitor virtual (xrandr / región / Wayland)
    ├── streaming/    # Pipelines GStreamer (Host encoder, Client decoder, WebRTC)
    ├── input/        # Captura de mouse/teclado e inyección uinput
    └── app/          # Aplicación GTK4 / Libadwaita
```

---

## 🚀 Requisitos e Instalación

### 1. Dependencias del Sistema

Ejecuta el script automatizado para instalar paquetes requeridos en Ubuntu 24.04 / 22.04:

```bash
chmod +x scripts/setup_deps.sh
./scripts/setup_deps.sh
```

O instala manualmente las bibliotecas:

```bash
sudo apt update && sudo apt install -y \
    build-essential pkg-config \
    gstreamer1.0-plugins-bad libgstreamer-plugins-bad1.0-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    libgtk-4-dev libadwaita-1-dev \
    libpipewire-0.3-dev libnice-dev gstreamer1.0-nice libsrtp2-dev \
    gstreamer1.0-plugins-good gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav gstreamer1.0-pipewire \
    avahi-daemon
```

### 2. Permisos para Inyección de Input (`/dev/uinput`)

Para permitir que el Host cree dispositivos virtuales de mouse y teclado sin privilegios de `root`:

```bash
sudo cp config/99-screenextend-uinput.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG input $USER
```
*(Nota: Requiere reiniciar sesión para que el grupo `input` surta efecto).*

---

## 💻 Compilación y Ejecución

### Compilar todo el proyecto:

```bash
cargo build --release
```

### Ejecutar la aplicación:

```bash
cargo run --bin screenextend-app
```

### Modo de Uso:

1. **En la máquina Host (pantalla principal):**
   - Inicia `screenextend-app` y selecciona la pestaña **"Host Mode"**.
   - Haz clic en **"Start Sharing"**.
   - Se mostrará el PIN de 4 dígitos generado.

2. **En la máquina Cliente (segunda computadora o laptop):**
   - Inicia `screenextend-app` y selecciona la pestaña **"Client Mode"**.
   - Verás automáticamente el nombre del Host en la lista **"Discovered Hosts (mDNS)"**.
   - Selecciona el Host, ingresa el PIN de 4 dígitos y pulsa **"Connect"**.
   - ¡El escritorio extendido se proyectará en tiempo real en la ventana del cliente, permitiendo el control del cursor y teclado!

---

## 🧪 Tests Automatizados

```bash
cargo test --workspace
```

---

## 📄 Licencia

MIT © Javier España