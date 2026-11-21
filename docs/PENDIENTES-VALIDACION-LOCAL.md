# Pendientes para terminar y comprobar ISA en local

Este documento resume el estado del árbol de trabajo de la rama `arena/ce6975bb-isa`. Es una guía de revisión, **no una certificación de seguridad ni una lista necesariamente exhaustiva de errores**. Los cambios actuales no tienen commit; conserva también los archivos nuevos sin seguimiento al copiarlos a tu máquina. Los resultados de CI de commits anteriores no verifican estos cambios.

## Antes de empezar

- Usa una máquina Windows de prueba, un workspace desechable sin secretos y, para Hyper-V, una VM e imagen VHDX autorizadas y desechables. No uses credenciales de dominio o del host dentro del invitado.
- No elimines las restricciones de plataforma para hacer que una función parezca disponible. Actualmente el perfil Windows `sandboxed` debe **fallar cerrado** y no recurrir al PowerShell del host.
- La vista previa Node que se abrió en el entorno remoto solo muestra la interfaz: no incluye backend Rust, puente xKiro ni Hyper-V. No demuestra que esas funciones operen.
- El servidor desde la carpeta y `cargo run` deben seguir funcionando como alternativa al instalador; no convertir el escritorio en requisito.

## 1. Sandbox Windows y transferencia: aún sin terminar

**Estado:** ni Hyper-V ni Windows Sandbox están conectados como sandbox de ejecución de ISA. La transferencia del workspace **no está conectada a la VM** y no importa archivos al host. Los módulos Rust `windows_sandbox_*` son prototipos de prueba, no componentes de producción. Las capacidades del servidor indican `windowsSandboxAvailable: false`.

### Trabajo que falta

- [ ] Elegir y completar un transporte de host a invitado y de invitado a host con autenticación real, identidad ligada a la instancia, límites de memoria y tamaño antes de deserializar datos, plazos, cancelación y cierre. Tratar todo lo enviado por la VM como no confiable. Los marcadores de VM no son atestación ni protegen contra un administrador o proceso local malicioso.
- [ ] Resolver el bloqueo de PowerShell Direct: `New-PSSession -VMId` no expone directamente `-SessionOption` en su conjunto de parámetros documentado. Aún no se ha demostrado cómo limitar objetos/datos recibidos, tiempo y cancelación para esta ruta; alternativamente diseñar otro transporte autenticado y acotado. Fuente: [New-PSSession](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/new-pssession?view=powershell-7.5). Una sesión autenticada con credenciales del invitado no garantiza la integridad del invitado.
- [ ] Implementar un agente invitado y conectar el transporte con el protocolo/receptor; integrar ciclo de vida, ejecución, supervisión de chat y manejo de fallos en ISA. No enviar credenciales del host al invitado.
- [ ] Implementar la **copia del workspace al invitado** con instantáneas del host fiables y validación mediante handles. Evitar escapes, reparse points, enlaces duros, alias de Windows, rutas especiales y carreras; comprobar límites y objetos especiales. Los chequeos léxicos actuales no bastan.
- [ ] Implementar la recepción y revisión de cambios del invitado. La importación requiere **aprobación separada** de la propuesta exacta, revisión de eliminaciones, nueva comprobación de conflictos e identidades inmediatamente al aplicar, protección contra carreras y undo persistente. Un hash entregado por el invitado no hace confiable el contenido. No aplicar nada automáticamente.
- [ ] Completar la política de red elegida (host/LAN e internet) y verificar su alcance efectivo y riesgos; adjuntar un switch existente no demuestra conectividad ni aislamiento.
- [ ] Completar límites de recursos, cancelación y teardown verificable, incluyendo limpieza segura de discos, carpetas y residuos después de errores o caídas. Nunca borrar recursos ajenos basándose solo en un prefijo de nombre.
- [ ] Completar también la opción Windows Sandbox, elegida junto a Hyper-V; los planes y validadores existentes no constituyen una ejecución integrada.
- [ ] Integrar únicamente después de pruebas Windows reales, pruebas adversariales y revisión de seguridad; mantener supervisión del chat y ausencia de fallback al host.

### Archivos que revisar

- `desktop/windows/hyperv-lifecycle.ps1` y `hyperv-policy.ps1`: prototipo manual de creación/destrucción. No comprobado con Hyper-V real. La destrucción deja discos y carpetas; hay que inspeccionarlos y limpiarlos manualmente con cuidado. Los controles de imagen y rutas no eliminan carreras locales.
- `desktop/windows/hyperv-channel-check.ps1`: diagnóstico manual de conexión con PowerShell Direct; no transfiere datos ni conecta ISA. Solicita credenciales del invitado y ejecuta una expresión fija. No garantiza límite de tiempo o limpieza tras caída.
- `desktop/windows/test-hyperv-lifecycle.ps1`, `test-hyperv-channel.ps1`: pruebas con mocks, no ejecutadas en este entorno. No prueban una VM real.
- `src/cli/windows_sandbox_lifecycle.rs`, `windows_sandbox_protocol.rs`, `windows_sandbox_frame.rs`, `windows_sandbox_wire.rs`, `windows_sandbox_transfer.rs`, `windows_sandbox_changes.rs`, `windows_sandbox_manifest.rs`, `windows_sandbox_export_plan.rs`, `windows_sandbox_import_plan.rs`, `windows_sandbox_wsb.rs`: primitivas de prueba sin transporte real ni aplicación al filesystem. El límite de frames es 4 MiB incluyendo overhead; verificar que sea apropiado para el diseño final.
- `docs/windows-sandbox-design.md`: diseño, límites y ejemplo manual del ciclo de vida. Leer sus advertencias antes de ejecutar algo.

### Comprobaciones locales sugeridas

```powershell
cargo check --locked --all-targets
cargo test --locked --bin isa windows_sandbox_
cargo test --locked --test windows_supervision
powershell -NoProfile -File .\desktop\windows\test-hyperv-channel.ps1
pwsh -NoProfile -File .\desktop\windows\test-hyperv-lifecycle.ps1
pwsh -NoProfile -File .\desktop\windows\check-sandbox.ps1
```

Después de corregir cualquier error de compilación o de mocks, probar en un host Hyper-V desechable: identidad equivocada, VM apagada, cancelación y credenciales inválidas, invitado malicioso o que no responde, archivos grandes/truncados, rutas conflictivas y teardown con fallos. Los mocks no sustituyen esas pruebas. No hay todavía una prueba extremo a extremo de transferencia porque la conexión no existe.

## 2. Puente xKiro: implementado experimentalmente, no verificado

- [ ] Compilar `src/xkiro_bridge.rs` y su integración en `src/main.rs` con las dependencias bloqueadas; ejecutar pruebas Rust y corregir incompatibilidades.
- [ ] Probar desde una instancia real no-streaming, SSE, errores, límites, timeouts, cancelación, Agent Pools y rechazo de solicitudes con origen/cabeceras no permitidas; comprobar que proveedores ajenos a xKiro mantienen su transporte. Archivos: `frontend/src/provider-client.mjs`, `assets/provider-client.js`, `tests/provider-client.mjs` y `docs/providers.md`.
- [ ] Revisar SSRF, redirecciones, DNS, límites y privacidad. El puente no autentica procesos locales ni scripts del mismo origen. La cancelación del navegador no garantiza que el trabajo remoto termine inmediatamente.
- [ ] No poner claves en archivos, capturas, logs o commits. La disponibilidad del proveedor no está garantizada.

## 3. Escritorio Windows, instalador y opciones experimentales

- [ ] Compilar y probar `desktop/windows/Isa.Desktop.csproj` y `desktop/tests/DesktopPolicy.Tests.csproj`; revisar `DesktopPolicy.cs`, `MainWindow.cs`, `installer.iss` y el icono `isa.ico`.
- [ ] Verificar interactivamente el icono en ventana, ejecutable, accesos directos e instalador; instalación/desinstalación, ausencia de WebView2, puerto ocupado y cierre del backend. El instalador/artefacto es experimental y no se ha firmado ni probado aquí.
- [ ] Confirmar enlaces externos HTTPS con confirmación y exportación de blobs con selector de destino; verificar que se deniegan permisos de WebView.
- [ ] Probar validación y reenvío de workspace/token y perfil explícito; sin configuración válida las acciones deben fallar cerradas. Probar que `sandboxed` en Windows se rechaza sin fallback.
- [ ] Verificar los modos de supervisión del chat para comandos, herramientas y skills. El PowerShell experimental manual **no está aislado**: opera con los privilegios y red del usuario. No confundirlo con el sandbox.
- [ ] Revisar auditoría cifrada, inicialización de clave en el almacén de credenciales, ACL, recuperación y aviso de auditoría incompleta ante errores. Los argumentos/resultados pueden contener datos sensibles. El undo guarda originales sin cifrar en `.isa`, por lo que quien tenga acceso puede leerlos.
- [ ] No presentar como habilitados los componentes Windows de edición/undo web, bucle de herramientas del modelo y MCP que sigan bloqueados o desconectados.

Guía adicional y ejemplo de configuración con workspace de prueba: `docs/desktop-experimental-review.md` y `desktop/windows/README.md`.

```powershell
dotnet run --project desktop/tests/DesktopPolicy.Tests.csproj --configuration Release
cargo build --locked --release --bin isa
dotnet publish desktop/windows/Isa.Desktop.csproj -c Release -r win-x64 --self-contained true -o desktop-prototype
```

Consulta `.github/workflows/rust.yml` y la documentación del instalador para los demás pasos y requisitos; las pruebas automatizadas no sustituyen la revisión interactiva.

## 4. Pruebas generales y estado conocido

En este entorno **no había Rust/cargo, Windows, PowerShell ni .NET**, así que los cambios actuales en Rust, scripts y C# no se compilaron ni ejecutaron aquí. Los checks Node de interfaz de capacidades y cliente de proveedores, `npm run check --prefix frontend`, `git diff --check` y el análisis sintáctico con tree-sitter de los scripts de canal pasaron; esto no demuestra funcionamiento en Windows ni seguridad. La configuración de CI nueva aún no se ha ejecutado para este árbol.

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets
cargo test --locked --all-targets
node tests/server-capability-ui.cjs
node tests/provider-client.mjs
npm ci --prefix frontend
npm run check --prefix frontend
git diff --check
git status --short
```

Ejecuta también las comprobaciones de seguridad y las pruebas específicas que aparecen en `.github/workflows/rust.yml`, y revisa los resultados y archivos no rastreados. La integración de URLs de proveedores en `assets/interface.js` necesita comprobación real de disponibilidad y configuración de cuenta; una URL documentada no garantiza acceso ni credenciales.
