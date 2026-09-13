# Guía de Contribución — ScreenExtend 🤝

¡Bienvenido al proyecto **ScreenExtend**! Nos alegra mucho recibir colaboraciones para hacer crecer esta herramienta de extensión de escritorio libre y abierta para Linux.

---

## 1. Primeros Pasos

1. Clona el repositorio:
   ```bash
   git clone https://github.com/Javier-Espana/RemoteDesktopApp.git
   cd RemoteDesktopApp
   ```
2. Instala las dependencias y configura permisos ejecutando:
   ```bash
   chmod +x scripts/setup_deps.sh
   ./scripts/setup_deps.sh
   ```
3. Verifica que la suite de tests unitarios pase correctamente:
   ```bash
   cargo test --workspace
   ```

---

## 2. Convenciones de Código y Estilo

- **Rustfmt**: Asegúrate de que el código esté formateado según las guías estándar de Rust:
  ```bash
  cargo fmt --all -- --check
  ```
- **Clippy**: Ejecuta el linter de Rust para prevenir malas prácticas y advertencias:
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  ```
- **Commits Convencionales**: Sigue el estándar [Conventional Commits](https://www.conventionalcommits.org/):
  - `feat(...)`: Nueva funcionalidad.
  - `fix(...)`: Corrección de un fallo o bug.
  - `docs(...)`: Mejoras o adiciones a la documentación.
  - `refactor(...)`: Reestructuración de código sin cambios de comportamiento.
  - `chore(...)`: Tareas de mantenimiento o configuración.

---

## 3. Arquitectura del Código

Antes de realizar cambios, lee detenidamente:
- [`docs/ARCHITECTURE.md`](file:///home/javier-espana/Escritorio/RemoteDesktopApp/docs/ARCHITECTURE.md): Diagrama y ciclo de vida de los componentes.
- [`docs/TROUBLESHOOTING_AND_DECISIONS.md`](file:///home/javier-espana/Escritorio/RemoteDesktopApp/docs/TROUBLESHOOTING_AND_DECISIONS.md): Fallas comunes y por qué el código está estructurado de esa forma.

Cada crate tiene un propósito específico. No introduzcas dependencias circulares ni rompas la separación de responsabilidades:
- La UI (`screenextend-app`) nunca debe manejar sockets TCP o GStreamer directamente; delega en `screenextend-discovery` y `screenextend-streaming`.
- `screenextend-common` no debe depender de crates pesados como GTK o GStreamer.

---

## 4. Áreas Clave donde Necesitamos Ayuda (Roadmap)

1. **Wayland Nativo (ScreenCast Portal)**:
   - Implementar el backend `xdg-desktop-portal` con token de restauración para no solicitar permisos de captura en cada reconexión en GNOME/KDE.
2. **SCTP DataChannels WebRTC**:
   - Migrar la transmisión de eventos de ratón y teclado desde el canal TCP secundario al canal de datos SCTP nativo de WebRTC.
3. **Soporte de Codificación AV1**:
   - Incorporar `svtav1enc` o `vaapiav1enc` con autodetección de capacidades para optimizar el consumo de ancho de banda.
4. **Pruebas en Otras Distribuciones**:
   - Validar y crear guías específicas para Fedora (RPM), Arch Linux (PKGBUILD) y openSUSE.
