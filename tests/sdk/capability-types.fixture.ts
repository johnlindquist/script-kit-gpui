/**
 * Compile-only author contract for the host-owned SDK capability wire types.
 * No SDK runtime is imported and no application/provider action is performed.
 */
import type {
  Action,
  ExecResult,
  FieldDef,
  HostCommandDoctorEntry,
  HostCommandDoctorReport,
  HostCommandDoctorState,
  HostSdkAvailability,
  HostSdkAuthoringResource,
  HostSdkCapability,
  HostSdkCapabilityCatalog,
  HostScriptValidationIssue,
  SdkCapabilityDiagnostic,
  SdkCapabilityDiagnosticCode,
  SdkExecutionTopology,
  ScriptMetadata,
} from '../../scripts/kit-sdk';

const topologies = [
  'typescript-script',
  'typescript-scriptlet',
  'typescript-scriptlet-interactive',
  'shell-scriptlet',
  'python-scriptlet',
] as const satisfies readonly SdkExecutionTopology[];

const diagnosticCodes = [
  'unknown_capability',
  'unsupported_capability',
  'missing_sdk_transport',
  'interactive_prompt_unavailable',
  'unsupported_platform',
  'missing_permission',
  'permission_inventory_unavailable',
  'host_version_too_old',
  'invalid_host_version',
] as const satisfies readonly SdkCapabilityDiagnosticCode[];

const commandDoctorStates = [
  'ready',
  'experimental',
  'unsupported',
  'blocked',
  'permissionPending',
] as const satisfies readonly HostCommandDoctorState[];

type MissingDiagnosticCode = Exclude<SdkCapabilityDiagnosticCode, typeof diagnosticCodes[number]>;
type MissingTopology = Exclude<SdkExecutionTopology, typeof topologies[number]>;
type MissingDoctorState = Exclude<HostCommandDoctorState, typeof commandDoctorStates[number]>;
const allDiagnosticCodesCovered: MissingDiagnosticCode extends never ? true : false = true;
const allTopologiesCovered: MissingTopology extends never ? true : false = true;
const allDoctorStatesCovered: MissingDoctorState extends never ? true : false = true;

const capability: HostSdkCapability = {
  name: 'computer.captureNativeWindow',
  support: 'supported',
  minimumHostVersion: '1.2.3',
  requiresInteractivePrompt: false,
  requiredPermissions: ['accessibility', 'screen-recording'],
  supportedPlatforms: ['macos'],
};

const catalog: HostSdkCapabilityCatalog = {
  schemaVersion: 1,
  hostVersion: '1.2.3',
  capabilities: [capability],
};

const host: HostSdkAvailability = {
  hostVersion: '1.2.3',
  platform: 'macos',
  grantedPermissions: ['accessibility'],
};

const missingPermission: SdkCapabilityDiagnostic = {
  code: 'missing_permission',
  capability: capability.name,
  message: 'Screen Recording has not been granted.',
  alternatives: ['Grant Screen Recording explicitly.'],
};

const doctorResource: HostSdkAuthoringResource = {
  uri: 'kit://command-doctor',
  name: 'Command Doctor',
  description: 'Read-only command readiness and safe author repairs.',
};

const pendingIssue: HostScriptValidationIssue = {
  severity: 'warning',
  path: '/tmp/move-window.ts',
  scriptName: 'Move Window',
  message: 'Accessibility permission inventory is not available.',
  kind: {
    kind: 'capabilityUnavailable',
    capability: 'moveWindow',
    code: 'permission_inventory_unavailable',
  },
};

const pendingCommand: HostCommandDoctorEntry = {
  source: 'script',
  name: 'Move Window',
  path: '/tmp/move-window.ts',
  pluginId: 'main',
  state: 'permissionPending',
  executable: false,
  primaryAction: {
    title: 'Run Script',
    enabled: false,
    reason: 'permission_pending',
    identityFingerprint: 'sha256:deadbeef',
  },
  issues: [pendingIssue],
};

const doctorReport: HostCommandDoctorReport = {
  schemaVersion: 1,
  hostVersion: '1.2.3',
  platform: 'macos',
  permissionInventoryKnown: false,
  totalCommands: 1,
  readyCount: 0,
  experimentalCount: 0,
  unsupportedCount: 0,
  blockedCount: 0,
  permissionPendingCount: 1,
  commands: [pendingCommand],
};

const response: ExecResult = { stdout: 'ok', stderr: '', exitCode: 0 };
const dateTimeField: FieldDef = {
  name: 'launchAt',
  label: 'Launch at',
  type: 'datetime-local',
};

const promptActions: Action[] = [{ name: 'Save', value: 'save' }];
const fieldsActions: Exclude<Parameters<typeof globalThis.fields>[1], undefined> = promptActions;
const formActions: Exclude<Parameters<typeof globalThis.form>[1], undefined> = promptActions;
const terminalActions: Exclude<Parameters<typeof globalThis.term>[1], undefined> = promptActions;

type AlwaysRejects<Feature extends (...args: never[]) => unknown> =
  ReturnType<Feature> extends Promise<never> ? true : false;
type AlwaysThrows<Feature extends (...args: never[]) => unknown> =
  ReturnType<Feature> extends never ? true : false;

const widgetNeverResolves: AlwaysRejects<typeof globalThis.widget> = true;
const webcamNeverResolves: AlwaysRejects<typeof globalThis.webcam> = true;
const microphoneNeverResolves: AlwaysRejects<typeof globalThis.mic> = true;
const eyeDropperNeverResolves: AlwaysRejects<typeof globalThis.eyeDropper> = true;
const panelAlwaysThrows: AlwaysThrows<typeof globalThis.setPanel> = true;
const previewAlwaysThrows: AlwaysThrows<typeof globalThis.setPreview> = true;
const promptAlwaysThrows: AlwaysThrows<typeof globalThis.setPrompt> = true;

const inlineChatMethods = [
  'addMessage',
  'startStream',
  'appendChunk',
  'completeStream',
  'clear',
  'setError',
  'clearError',
  'getMessages',
  'getResult',
] as const satisfies readonly (keyof typeof chat)[];

type MissingInlineChatMethod = Exclude<keyof typeof chat, typeof inlineChatMethods[number]>;
const allInlineChatMethodsCovered: MissingInlineChatMethod extends never ? true : false = true;

const metadata: ScriptMetadata = {
  name: 'Safe author contract',
  keyword: ':safe',
  fallback: false,
  sdkCapabilities: ['arg', 'readFile', 'writeFile', 'exec'],
  executionTopology: 'typescript-script',
};

// @ts-expect-error SDK capability declarations are an array, never a bare string.
const malformedCapabilities: ScriptMetadata = { sdkCapabilities: 'arg' };

// @ts-expect-error Execution topology must match a known transport identifier.
const malformedTopology: ScriptMetadata = { executionTopology: 'ruby-scriptlet' };

// @ts-expect-error Ruby scriptlets are not a supported SDK execution topology.
const invalidTopology: SdkExecutionTopology = 'ruby-scriptlet';

// @ts-expect-error Diagnostic codes are stable snake_case wire identifiers.
const invalidDiagnostic: SdkCapabilityDiagnosticCode = 'missingPermission';

// @ts-expect-error Catalog schema version 1 is an exact compatibility contract.
const invalidCatalogVersion: HostSdkCapabilityCatalog['schemaVersion'] = 2;

// @ts-expect-error Command identities must use the shared SHA-256 receipt prefix.
const rawDoctorIdentity: NonNullable<HostCommandDoctorEntry['primaryAction']>['identityFingerprint'] =
  'script:main:private';

// @ts-expect-error Pending permission is a distinct camelCase wire state.
const invalidDoctorState: HostCommandDoctorState = 'permission_pending';

void [
  allDiagnosticCodesCovered,
  allTopologiesCovered,
  allDoctorStatesCovered,
  catalog,
  host,
  doctorResource,
  pendingIssue,
  pendingCommand,
  doctorReport,
  missingPermission,
  response,
  dateTimeField,
  fieldsActions,
  formActions,
  terminalActions,
  widgetNeverResolves,
  webcamNeverResolves,
  microphoneNeverResolves,
  eyeDropperNeverResolves,
  panelAlwaysThrows,
  previewAlwaysThrows,
  promptAlwaysThrows,
  inlineChatMethods,
  allInlineChatMethodsCovered,
  metadata,
  malformedCapabilities,
  malformedTopology,
  invalidTopology,
  invalidDiagnostic,
  invalidCatalogVersion,
  rawDoctorIdentity,
  invalidDoctorState,
];
