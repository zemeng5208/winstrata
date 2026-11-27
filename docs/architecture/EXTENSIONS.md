# Extensions, MCP, and rescue

[简体中文](../zh-CN/architecture/EXTENSIONS.md) · [Index](../INDEX.md)

<a id="section-5"></a>

## Plugin and script system

Plan 014 records the native contract and process lifecycle in [Plugins](PLUGINS.md). The 25 collectors, user opt-in, Python and TypeScript/JavaScript SDKs, OS containment, the shared market, and signed transactional rollback remain later plans.

Manage each plugin's load, start, pause, stop, disable, update, and recovery independently. Third-party plugins can be uninstalled. Built-ins remain installed. Disable stops exclusive subscriptions, resources, and tasks and preserves history. Shared collection uses references. Support live install, replacement, disable, and update without restarting the GUI or Windows where safe. Report driver restart requirements. Verify compatible dependencies, data and config migration, and transactional rollback.

Declare capabilities, dependencies, events, services, UI components, pages, navigation and dashboard widgets, permissions, locale resources, MCP, and script interfaces. A manifest file is a candidate. Format, protocol fields, runtime and IPC require approval. Installation grants scoped permissions, updates request new permissions, and revocation takes effect at any time. Audit file and directory reads, registry, capture, discovery, outbound network, process and system state, history, plugin communication, dedicated storage, and GUI. Deny undeclared or indirect privilege.

Provide Python and TypeScript/JavaScript SDKs only. Centrally resolve interpreters and dependencies and start them on demand. Users need no separately preinstalled Python or Node.js. Isolate plugin dependency environments, permissions, and faults. Validate OS-level containment of file, network, child process, and system access, and enforce disablement and revocation. Treat input to native plugins as untrusted.

### Shared market

The official, community, installed, and updates views share one market for plugins and scripts. Custom sources and local packages have navigation entry points. The market uses public repository indexes and releases without tied accounts or proprietary cloud dependencies. Submit and update community packages freely with automatic format, dependency, compatibility and basic security checks. Distinguish official, community, and unverified items, and display source, license, version and permissions. Support reports, delisting, warnings, signatures and integrity, custom sources, and local imports. Apply permission checks before trusting any extension.

Composite packages can include multiple Python, TS, and JS modules, pages and widgets, scripts, rules, config, translations, dependencies, permissions, MCP and script extensions, and documentation. Version, install, update and uninstall a package together and enable its capabilities separately. Installed scripts are initially inactive. Save independent script revisions. User confirmation is required before public release. Store publication credentials locally and securely, outside ordinary plugin access, and exclude monitoring data from publication.

### Script workbench and updates

StrataScript provides shared editing, syntax highlighting, completion, diagnostics, testing, debugging, hot reload, manual and event/scheduled/conditional triggers, permissions, dependencies, resource limits, logs, errors, history, versions, and import/export. Plugins register interfaces and use Core's runtime and isolation. Integrate filter, rules, tags, storage and snapshot policies. Snapshots start manual, with no preset automation. User or external agent scripts may request authorized automation within read-only, rescue and cleanup boundaries.

Version and update Core, service and drivers, all plugins, scripts, and runtimes independently. Support manual and automatic updates, pins, rollback, version/protocol/dependency validation, signature and integrity checks, config and history preservation, and explicit driver restart prompts. Distribute publicly without uploading observed data and validate updates in the development build.

<a id="section-6"></a>

## Local MCP and temporary rescue

Authenticate local agents. Candidate transports are stdio and loopback. Verify process identity and authentication separately from interface locality, reject LAN and internet clients, and apply per-agent authorization, revocation, audit and budgets. Dynamically register authorized tools, resources and prompts and unregister on disable. Treat imported, network, and plugin content as untrusted. External platforms manage agent operation and decisions. There is no general administrator command tool.

The authorized catalog covers:

- System and hardware, software lifecycles and remnants, files, registry and environment, processes and services, captures and traffic, topology, signatures and dependencies, security and updates, history, search, timeline, snapshots and diagnostics.
- Plugin development with market lookup, SDK and contract discovery, scaffolding and editing, build, validation, package, tests, logs, hot reload, local install, and version management. Core recompilation is unnecessary.
- Scripts with create, read, edit, test, debug, permission and dependency checks, enable/disable, versions and triggers, and results, resources, and errors.
- Market dependency checks, package, version, documentation and publication request. Confirm external publication and apply the ordinary install and new-permission authorization workflow.
- Authorized tags, filters, storage and resource policies, and manual snapshots. In historical mode, serve historical and non-collecting functions without starting observation.

### StrataRescue

The built-in is disabled by default and excluded from onboarding and batch enablement. It has no separate rescue page, bundled detections, malware database, automatic repair or preset rules. The user opens a temporary session and authorizes restricted local operations through StrataMCP. Closing the session revokes the channel. The external agent owns target selection and operation decisions.

| Permitted | Outside the capability |
|---|---|
| Terminate an identified process or process tree | Arbitrary admin commands, shell, PowerShell, or general command delegation |
| Quarantine or remove an identified file | Direct registry, environment, service, firewall or security policy changes |
| Controlled uninstall and cleanup of remnants with confirmed ownership | Arbitrary driver management, system repair, other users' data, or direct calls by normal plugins or scripts |

Validate identities, parameters, targets, authorization, and true outcomes. Audit requests and results. Prefer reversible treatment and protect critical components and Windows protections. Verify that legitimate uninstall cannot become arbitrary execution. Propose target-verification and audit implementation for owner approval. Test with harmless targets on isolated Windows. Explicit permission is required for dangerous tests on a real computer.
