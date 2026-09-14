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

## 📚 Documentación Técnica y para Colaboradores

- **[Arquitectura y Diseño Técnico](docs/ARCHITECTURE.md)**: Estructura del workspace, flujo de señalización WebRTC, diagramas y responsabilidades de los crates.
- **[Decisiones de Diseño y Resolución de Problemas (Troubleshooting)](docs/TROUBLESHOOTING_AND_DECISIONS.md)**: Diagnóstico detallado de errores (libnice/webrtcbin, compatibilidad NVENC RTX 50/driver 595, mDNS, uinput).
- **[Guía de Despliegue y Empaquetado](docs/DEPLOYMENT_AND_PACKAGING.md)**: Instalación de dependencias, generación de paquetes `.deb` y manifiesto Flatpak.
- **[Guía de Contribución](docs/CONTRIBUTING.md)**: Estándares de código, cómo ejecutar tests y roadmap de tareas prioritarias.

---

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

### 3. Actualizar a una nueva versión sin conservar artefactos antiguos

Ejecuta esta secuencia desde la raíz del repositorio cada vez que cambies de
versión o hagas `pull` de cambios importantes. Cierra primero cualquier instancia
de `screenextend-app` que esté ejecutándose:

```bash
cd /ruta/a/RemoteDesktopApp

# Comprueba que no haya cambios locales que puedan sobrescribirse.
git status

# Actualiza el código sin crear un merge automático.
git pull --ff-only

# Actualiza dependencias del sistema, plugins GStreamer y reglas udev.
chmod +x scripts/setup_deps.sh
./scripts/setup_deps.sh

# Elimina todos los artefactos compilados de versiones anteriores.
cargo clean

# Descarga las dependencias declaradas en Cargo.lock/Cargo.toml.
cargo fetch

# Verifica el workspace antes de generar el binario final.
cargo test --workspace

# Genera una compilación limpia de producción.
cargo build --release --workspace
```

Si también cambió alguna versión declarada en `Cargo.toml`, actualiza el árbol de
dependencias de Rust de forma explícita y vuelve a limpiar y validar:

```bash
cargo update
cargo clean
cargo fetch
cargo test --workspace
cargo build --release --workspace
```

No ejecutes `cargo update` en cada actualización normal: puede cambiar versiones
transitivas y hacer que dos equipos compilen árboles distintos. Para reproducir
exactamente el árbol bloqueado, usa:

```bash
cargo fetch --locked
cargo test --workspace --locked
cargo build --release --workspace --locked
```

Si todavía aparecen errores de una versión anterior después de `cargo clean`, haz
una limpieza profunda del directorio de salida y repite la secuencia:

```bash
rm -rf target
cargo fetch
cargo test --workspace
cargo build --release --workspace
```

Después de reinstalar o modificar las reglas de `/dev/uinput`, aplica también:

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Si el usuario acaba de entrar al grupo `input`, debe cerrar sesión y volver a
entrar antes de ejecutar la aplicación.

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