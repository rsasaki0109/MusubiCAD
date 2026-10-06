# Plugin API

`opencad-plugin-api` defines the first versioned extension boundary for
MusubiCAD. It is a linked, stable-Rust contract: plugin implementations are
compiled against the crate and exchange request/result DTOs in process. It does
not claim an ABI-stable dynamic-loading format. The API includes deterministic
registration, host-controlled CLI/Agent invocation, compatibility tests, and a
standalone example crate.

## Boundary

The contract deliberately carries data rather than ownership:

| Contract | Request | Result |
|---|---|---|
| `FeaturePlugin` | `FeatureRequest` with semantic IDs, unit-bearing expression strings, and serializable inputs | `FeatureResult` containing a `DesignPatch` and diagnostics |
| `ImporterPlugin` | `ImportRequest` with a format, optional source name, and caller-owned bytes | `ImportResult` containing a `DesignPatch` and diagnostics |
| `ExporterPlugin` | `ExportRequest` with a format and immutable `serde_json::Value` state | `ExportResult` with format, media type, bytes, and diagnostics |

No contract accepts or returns `OcadDocument`, `&mut` document state, OCCT
handles, viewport/camera/selection state, filesystem paths/handles, network
clients, or direct mutation callbacks. The host must validate a returned
`DesignPatch` through the normal command/transaction boundary.

All fallible manifest and trait methods return the repository-standard
`opencad_core::Result<T>` (`OpenCadError`). `PluginError` is a serializable
diagnostic DTO for plugin-facing error data; it is not a replacement error
type for public fallible APIs.

## Manifest and compatibility

Each plugin declares a serializable `PluginManifest`:

```json
{
  "schema": "musubicad.plugin-manifest.v1",
  "id": "example.bracket-feature",
  "name": "Bracket Feature Example",
  "version": "0.1.0",
  "api_version": { "major": 1, "minor": 0 },
  "kind": "feature",
  "capabilities": ["feature_patch"]
}
```

The current contract is `CURRENT_PLUGIN_API = { major: 1, minor: 0 }`.
Compatibility is directional and explicit: a plugin is accepted when its major
version equals the host major and its minor version is less than or equal to the
host minor. Future minor versions and every major mismatch are rejected before
invocation. The manifest schema is also checked exactly.

The exact checked-in example is
[`examples/plugin-example/manifest.json`](../../examples/plugin-example/manifest.json).

`capabilities` is a sorted set of the data operations explicitly declared by
the plugin. A feature must declare `feature_patch`, an importer must declare
`import_patch`, and an exporter must declare `export_bytes`. A host
`PluginCapabilityPolicy` may allow a narrower set; undeclared, kind-incompatible,
or policy-disallowed capabilities are rejected deterministically. Older v1
manifest JSON without this field remains readable through serde defaults, but
cannot register until the required capability is declared.

`PluginRegistry` stores the three linked trait kinds in a `BTreeMap` keyed by
manifest ID. Registration validates manifest schema, API compatibility, kind,
required capabilities, policy, and duplicate IDs. Listing returns manifests and
IDs in lexical order regardless of registration order. Listing necessarily
calls each plugin's `manifest()` metadata accessor; it does not call
`FeaturePlugin::apply`, `ImporterPlugin::import`, or `ExporterPlugin::export`.

Linked in-process plugin code is trusted and is not sandboxed by P4-002. The
registry and capability enum expose no filesystem, network, UI, document
ownership, or OCCT/kernel capability. Process isolation and security policy
for untrusted code remain future work.

## Linked Rust shape

The core trait shape is intentionally small:

```rust
pub trait FeaturePlugin {
    fn manifest(&self) -> &PluginManifest;
    fn apply(&self, request: FeatureRequest) -> opencad_core::Result<FeatureResult>;
}

pub trait ImporterPlugin {
    fn manifest(&self) -> &PluginManifest;
    fn import(&self, request: ImportRequest) -> opencad_core::Result<ImportResult>;
}

pub trait ExporterPlugin {
    fn manifest(&self) -> &PluginManifest;
    fn export(&self, request: ExportRequest) -> opencad_core::Result<ExportResult>;
}
```

The request/result structures derive `Serialize` and `Deserialize` where both
directions are meaningful. Focused crate tests assert round trips and exact
manifest JSON bytes. Run them with:

```bash
cargo test -p opencad-plugin-api
cargo clippy -p opencad-plugin-api --all-targets -- -D warnings
```

## Product invocation

The CLI constructs a fresh, deterministic `PluginHost` for each command; the
Agent stdio loop constructs one host for that loop. Neither is global mutable
state. The linked built-ins demonstrate all three contracts:

- `example.bracket-feature` proposes a unit-bearing parameter `DesignPatch`;
- `example.patch-importer` decodes caller-owned DesignPatch JSON bytes;
- `example.json-exporter` serializes immutable document state to bytes.

List or invoke them with:

```text
musubicad plugin list --json
musubicad plugin invoke example.bracket-feature work/bracket.ocad.d examples/plugin-example/feature-request.json --dry-run --json
```

Feature/importer results are dry-run validated, then applied with the shared
transaction and backend history only when not in dry-run mode. Invalid plugin
patches leave the document byte-for-byte unchanged. Export bytes are written
only by the CLI/Agent host when an output path is supplied; plugin code receives
no filesystem handle or path capability.

The equivalent Agent methods are `opencad.plugin_list` and
`opencad.plugin_invoke`; see the checked-in requests under `examples/agent/`.
Loading and security isolation remain future work. See the
[plugin developer guide](../developer-guide/plugins.md) for authoring steps and
the exact compatibility and failure evidence.
