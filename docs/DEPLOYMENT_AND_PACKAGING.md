# ScreenExtend — Guía de Despliegue, Empaquetado e Instalación

Este documento detalla los requisitos, pasos de instalación, configuración del sistema y empaquetado para poner en marcha **ScreenExtend** en cualquier distribución Linux moderna (Ubuntu, Debian, Fedora, Arch, etc.).

---

## 1. Requisitos del Sistema

### Hardware
- **Host (Equipo que comparte su pantalla)**:
  - CPU: Mínimo 2 núcleos (Intel Core i3 o AMD Ryzen 3 en adelante).
  - GPU: Aceleración por hardware recomendada (Intel QuickSync vía VA-API, AMD vía VA-API o NVIDIA NVENC). Codificación por CPU con `x264enc` funciona universalmente.
- **Client (Laptop o pantalla secundaria)**:
  - Cualquier equipo capaz de decodificar H.264 a 1080p60 por software o hardware.
- **Red**: Conexión de red local (Wi-Fi 5GHz recomendada o Ethernet cableado para mínima latencia y fluctuación).

### Software Base
- Linux Kernel 5.10+ (con módulo de kernel `uinput` disponible).
- GStreamer 1.20+ (probado exhaustivamente en GStreamer 1.24.2 / Ubuntu 24.04 LTS).
- GTK 4.12+ y Libadwaita 1.4+.
- Avahi Daemon (para resolución y publicación mDNS).

---

## 2. Dependencias de Sistema Requeridas

La aplicación depende de bibliotecas de desarrollo para compilar y de plugins de GStreamer en tiempo de ejecución:

| Categoría | Paquetes en Debian / Ubuntu | Propósito |
|---|---|---|
| **Compilación** | `build-essential`, `cargo`, `rustc`, `pkg-config`, `libssl-dev` | Cadena de herramientas Rust y compilación C |
| **GStreamer Core & Dev** | `libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`, `libgstreamer-plugins-bad1.0-dev` | Encabezados de enlace para crates de Rust |
| **GStreamer Plugins (Video/Audio)** | `gstreamer1.0-plugins-base`, `gstreamer1.0-plugins-good`, `gstreamer1.0-plugins-bad`, `gstreamer1.0-plugins-ugly`, `gstreamer1.0-libav` | `x264enc`, `opusenc`, `avdec_h264`, `ximagesrc` |
| **WebRTC & ICE** | `libnice-dev`, `gstreamer1.0-nice`, `libsrtp2-dev` | Negociación ICE en `webrtcbin` y cifrado SRTP |
| **Interfaz Gráfica** | `libgtk-4-dev`, `libadwaita-1-dev` | UI moderna con temas GNOME |
| **Captura y Pantalla** | `libpipewire-0.3-dev`, `gstreamer1.0-pipewire`, `pipewire`, `wireplumber`, `xdg-desktop-portal`, `xdg-desktop-portal-gnome`, `x11-xserver-utils`, `xcvt` | Captura Wayland mediante PipeWire/portal y gestión X11 (`xrandr`, `cvt`) |
| **Red & Periféricos** | `avahi-daemon`, `libsdl2-dev` | Descubrimiento mDNS y utilidades de hardware |

---

## 3. Instalación Automatizada mediante Script

El script [`scripts/setup_deps.sh`](file:///home/javier-espana/Escritorio/RemoteDesktopApp/scripts/setup_deps.sh) se encarga de:
1. Instalar todos los paquetes del sistema necesarios vía `apt`.
2. Instalar las reglas udev para `/dev/uinput` en `/etc/udev/rules.d/`.
3. Agregar al usuario actual al grupo del sistema `input`.
4. Ejecutar una verificación exhaustiva de cada plugin y herramienta.

```bash
chmod +x scripts/setup_deps.sh
./scripts/setup_deps.sh
```

> **Nota sobre permisos de uinput:**
> Si es la primera vez que se añade tu usuario al grupo `input`, es necesario cerrar sesión y volver a entrar (o reiniciar el equipo) para que la sesión tome el grupo nuevo.

### Wayland y portal de captura

En una sesión Wayland, el Host necesita que el portal de escritorio esté activo para
autorizar la captura. Después de instalar las dependencias, verifica:

```bash
systemctl --user is-active pipewire
systemctl --user is-active wireplumber
gst-inspect-1.0 --exists pipewiresrc
```

La captura Wayland está preparada para PipeWire, pero la creación de una salida de
monitor virtual depende del compositor y todavía no es universal. X11 sigue siendo
la ruta necesaria para crear la salida `xrandr` que permite colocar la pantalla
remota a la izquierda o a la derecha.

---

## 4. Compilación del Proyecto

Para compilar el binario optimizado para producción:

```bash
cargo build --release --workspace
```

El binario ejecutable quedará en:
```bash
target/release/screenextend-app
```

---

## 5. Empaquetado

### Opción A: Paquete Debian (.deb)
Los archivos de especificación Debian se encuentran en el directorio `debian/`:
- [`debian/control`](file:///home/javier-espana/Escritorio/RemoteDesktopApp/debian/control): Define las dependencias de compilación y ejecución.
- [`debian/rules`](file:///home/javier-espana/Escritorio/RemoteDesktopApp/debian/rules): Llama a Cargo con el perfil `--release`.

Para generar el paquete `.deb`:
```bash
sudo apt install -y debhelper devscripts dpkg-dev
dpkg-buildpackage -b -us -uc
```

### Opción B: Flatpak (Universal para todas las distribuciones)
Se incluye un manifiesto completo en [`build-aux/org.linux.screenextend.json`](file:///home/javier-espana/Escritorio/RemoteDesktopApp/build-aux/org.linux.screenextend.json) que utiliza el runtime `org.gnome.Platform//46` e integra acceso a PipeWire, sockets X11 y red local.

Para compilar e instalar el Flatpak:
```bash
flatpak-builder --user --install --force-clean build-dir build-aux/org.linux.screenextend.json
```
