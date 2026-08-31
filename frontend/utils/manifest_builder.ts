/**
 * @file manifest_builder.ts
 * @description Generates valid Kubernetes YAML manifests for StellarNode custom
 * resources from the topology configurator's in-memory state.
 *
 * All YAML is built via template literals — no external serialisation library
 * is used — so the output is stable, whitespace-correct, and suitable for
 * direct use with `kubectl apply`.
 *
 * @module manifest_builder
 */

import type {
  TopologyState,
  PlacedStellarNode,
  AvailabilityZone,
  WorkerNodeConfig,
} from '../configurator/src/topology_builder/types';

// WorkerNode is the alias used throughout this module; the underlying type
// imported from types.ts is WorkerNodeConfig.
type WorkerNode = WorkerNodeConfig;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/**
 * Converts a string to a safe Kubernetes DNS-label by lowercasing and
 * replacing any character that is not alphanumeric or a hyphen with '-'.
 * Leading/trailing hyphens are removed.
 */
function toDnsLabel(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

/**
 * Indents every line of a multi-line string by `spaces` space characters.
 * Empty lines are left as-is (no trailing whitespace).
 */
function indent(str: string, spaces: number): string {
  const pad = ' '.repeat(spaces);
  return str
    .split('\n')
    .map((line) => (line.length > 0 ? pad + line : line))
    .join('\n');
}

// ---------------------------------------------------------------------------
// Exported helpers
// ---------------------------------------------------------------------------

/**
 * Escapes a string for safe embedding in a YAML block scalar (literal block
 * style, denoted by `|`).  The returned value is the raw content that
 * should follow the `|` indicator, indented by the caller as needed.
 *
 * Any trailing newlines are normalised to a single trailing newline as
 * required by the YAML block scalar spec (`clip` chomping — the default).
 *
 * @param str - The raw string to escape (e.g. a TOML quorum-set fragment).
 * @returns The string with internal `\r\n` normalised to `\n` and a single
 *   trailing newline appended if missing.
 *
 * @example
 * ```ts
 * const toml = escapeYaml('[[QUORUM_SET]]\nTHRESHOLD_PERCENT=67\n');
 * // Returns: '[[QUORUM_SET]]\nTHRESHOLD_PERCENT=67\n'
 * ```
 */
export function escapeYaml(str: string): string {
  // Normalise Windows-style line endings to Unix
  const normalised = str.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
  // Ensure exactly one trailing newline (YAML clip chomping)
  return normalised.endsWith('\n') ? normalised : normalised + '\n';
}

// ---------------------------------------------------------------------------
// Topology spread constraints
// ---------------------------------------------------------------------------

/**
 * Builds the `topologySpreadConstraints` YAML block for a placed node.
 *
 * One constraint entry is emitted per availability zone the node is assigned
 * to, using `topology.kubernetes.io/zone` as the topology key.  The
 * `whenUnsatisfiable` field is always set to `DoNotSchedule` so pods are
 * never placed in a zone that would violate the spread requirement.
 *
 * @param nodeName - The Kubernetes resource name of the StellarNode; used as
 *   the `app` label in the `labelSelector`.
 * @param zones - All availability zones from the topology state; the function
 *   filters to the single zone referenced by `zoneId`.
 * @param zoneId - The `id` of the `AvailabilityZone` this node is placed in.
 * @returns A YAML string (without a leading key) representing the
 *   `topologySpreadConstraints` array, ready to be indented and embedded
 *   inside a `spec:` block.  Returns an empty string when `zones` contains
 *   no entry matching `zoneId`.
 */
function buildTopologySpreadConstraints(
  nodeName: string,
  zones: AvailabilityZone[],
  zoneId: string,
): string {
  const zone = zones.find((z) => z.id === zoneId);
  if (!zone) return '';

  // One constraint per zone (currently the node lives in exactly one zone,
  // but the helper signature accepts the full zones array for forward
  // compatibility with multi-zone spread in future iterations).
  const entries: string[] = [];

  entries.push(
    [
      `- maxSkew: 1`,
      `  topologyKey: topology.kubernetes.io/zone`,
      `  whenUnsatisfiable: DoNotSchedule`,
      `  labelSelector:`,
      `    matchLabels:`,
      `      app: ${toDnsLabel(nodeName)}`,
    ].join('\n'),
  );

  return entries.join('\n');
}

// ---------------------------------------------------------------------------
// PodAntiAffinity block
// ---------------------------------------------------------------------------

/**
 * Builds a `podAntiAffinity` YAML block based on the node's configured
 * affinity mode.
 *
 * - `Hard` → `requiredDuringSchedulingIgnoredDuringExecution`
 * - `Soft` → `preferredDuringSchedulingIgnoredDuringExecution` (weight 100)
 * - `None` → returns an empty string (no block emitted)
 *
 * The `topologyKey` is always `kubernetes.io/hostname` so that replicas are
 * spread across different physical/virtual hosts.
 *
 * @param node - The placed node whose `podAntiAffinity` mode is used.
 * @returns A YAML string starting with `podAntiAffinity:`, or `''`.
 */
function buildPodAntiAffinityBlock(node: PlacedStellarNode): string {
  if (node.podAntiAffinity === 'None') return '';

  const labelName = toDnsLabel(node.name);

  if (node.podAntiAffinity === 'Hard') {
    return [
      `podAntiAffinity:`,
      `  requiredDuringSchedulingIgnoredDuringExecution:`,
      `  - labelSelector:`,
      `      matchLabels:`,
      `        app: ${labelName}`,
      `    topologyKey: kubernetes.io/hostname`,
    ].join('\n');
  }

  // Soft
  return [
    `podAntiAffinity:`,
    `  preferredDuringSchedulingIgnoredDuringExecution:`,
    `  - weight: 100`,
    `    podAffinityTerm:`,
    `      labelSelector:`,
    `        matchLabels:`,
    `          app: ${labelName}`,
    `      topologyKey: kubernetes.io/hostname`,
  ].join('\n');
}

// ---------------------------------------------------------------------------
// ValidatorConfig block
// ---------------------------------------------------------------------------

/**
 * Builds the `validatorConfig` YAML block for Validator nodes.
 *
 * @param node - A placed node whose `nodeType` is `'Validator'` and whose
 *   `validatorConfig` field is populated.
 * @returns A YAML string starting with `validatorConfig:`, or `''` when the
 *   node type is not Validator or `validatorConfig` is absent.
 */
function buildValidatorConfigBlock(node: PlacedStellarNode): string {
  if (node.nodeType !== 'Validator' || !node.validatorConfig) return '';

  const vc = node.validatorConfig;
  const lines: string[] = [
    `validatorConfig:`,
    `  seedSecretRef: ${vc.seedSecretRef}`,
    `  enableHistoryArchive: ${vc.enableHistoryArchive}`,
  ];

  if (vc.quorumSet) {
    // Use YAML literal block scalar for potentially multi-line TOML content
    const escaped = escapeYaml(vc.quorumSet);
    lines.push(`  quorumSet: |`);
    // Each line of the block must be indented two more spaces (total 4)
    escaped
      .split('\n')
      .forEach((l) => lines.push(l.length > 0 ? `    ${l}` : ``));
    // Remove trailing empty line added by the split if present
    if (lines[lines.length - 1] === '') lines.pop();
  }

  if (vc.historyArchiveUrls && vc.historyArchiveUrls.length > 0) {
    lines.push(`  historyArchiveUrls:`);
    vc.historyArchiveUrls.forEach((url) => lines.push(`  - ${url}`));
  }

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Core manifest builders
// ---------------------------------------------------------------------------

/**
 * Builds a single `StellarNode` Kubernetes custom resource YAML manifest for
 * the provided placed node.
 *
 * The manifest includes:
 * - Standard Kubernetes metadata (`apiVersion`, `kind`, `metadata`)
 * - Resource requests and limits
 * - Persistent storage configuration
 * - `topologySpreadConstraints` derived from the node's assigned zone
 * - Optional `podAntiAffinity` when the strategy is not `'None'`
 * - Optional `validatorConfig` for Validator nodes
 *
 * No YAML serialisation library is used; the output is assembled directly
 * from template literals to ensure deterministic, whitespace-correct output
 * that can be piped straight to `kubectl apply`.
 *
 * @param node - The placed node to serialise.
 * @param zones - All availability zones in the current topology; used to
 *   resolve the zone name and build `topologySpreadConstraints`.
 * @param workerNodes - All worker nodes in the current topology (reserved for
 *   future use, e.g. node-affinity generation).
 * @returns A complete YAML string representing one `StellarNode` manifest.
 *
 * @example
 * ```ts
 * const yaml = buildNodeManifest(placedNode, state.zones, state.workerNodes);
 * console.log(yaml);
 * ```
 */
export function buildNodeManifest(
  node: PlacedStellarNode,
  zones: AvailabilityZone[],
  workerNodes: WorkerNode[],
): string {
  // workerNodes is accepted for API completeness / future node-affinity use.
  void workerNodes;

  const safeName = toDnsLabel(node.name);
  const namespace = toDnsLabel(node.namespace);

  // --- topologySpreadConstraints ---
  const tscRaw = buildTopologySpreadConstraints(safeName, zones, node.availabilityZoneId);
  const tscBlock = tscRaw
    ? `  topologySpreadConstraints:\n${indent(tscRaw, 4)}`
    : '';

  // --- podAntiAffinity (nested inside affinity:) ---
  const paaRaw = buildPodAntiAffinityBlock(node);
  const affinityBlock = paaRaw
    ? `  affinity:\n${indent(paaRaw, 4)}`
    : '';

  // --- validatorConfig ---
  const vcRaw = buildValidatorConfigBlock(node);
  const vcBlock = vcRaw ? indent(vcRaw, 2) : '';

  // Collect optional spec sections; filter empty strings before joining
  const optionalSections = [tscBlock, affinityBlock, vcBlock]
    .filter(Boolean)
    .join('\n');

  const manifest = `apiVersion: stellar.org/v1alpha1
kind: StellarNode
metadata:
  name: ${safeName}
  namespace: ${namespace}
  labels:
    app: ${safeName}
    app.kubernetes.io/managed-by: topology-configurator
    app.kubernetes.io/name: ${safeName}
    stellar.org/node-type: ${node.nodeType}
    stellar.org/network: ${node.network}
spec:
  nodeType: ${node.nodeType}
  network: ${node.network}
  version: "${node.version}"
  replicas: ${node.replicas}
  resources:
    requests:
      cpu: "${node.resources.cpu}"
      memory: "${node.resources.memory}"
    limits:
      cpu: "${node.resources.cpu}"
      memory: "${node.resources.memory}"
  storage:
    storageClass: ${node.storage.storageClass}
    size: ${node.storage.size}
    mode: ${node.storage.mode}
    retentionPolicy: ${node.storage.retentionPolicy}
  maxUnavailable: ${node.maxUnavailable}
  minAvailable: ${node.minAvailable}${optionalSections ? '\n' + optionalSections : ''}
`;

  return manifest;
}

/**
 * Generates a `PodDisruptionBudget` manifest that matches the given placed
 * node.
 *
 * The PDB uses the same `app: <name>` label selector that is applied to the
 * StellarNode pods, and enforces `minAvailable` from the node's configuration.
 * This prevents Kubernetes voluntary disruptions (node drains, rolling
 * upgrades) from taking down more replicas than the operator allows.
 *
 * @param node - The placed node for which to generate a PDB.
 * @returns A complete YAML string representing a `PodDisruptionBudget`
 *   resource, ready for `kubectl apply`.
 *
 * @example
 * ```ts
 * const pdbYaml = buildPodDisruptionBudget(placedNode);
 * ```
 */
export function buildPodDisruptionBudget(node: PlacedStellarNode): string {
  const safeName = toDnsLabel(node.name);
  const namespace = toDnsLabel(node.namespace);

  return `apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: ${safeName}-pdb
  namespace: ${namespace}
  labels:
    app: ${safeName}
    app.kubernetes.io/managed-by: topology-configurator
    app.kubernetes.io/name: ${safeName}
    stellar.org/node-type: ${node.nodeType}
    stellar.org/network: ${node.network}
spec:
  minAvailable: ${node.minAvailable}
  selector:
    matchLabels:
      app: ${safeName}
`;
}

/**
 * Generates concatenated Kubernetes YAML manifests for every placed node in
 * the given topology state.
 *
 * For each node the output contains:
 * 1. A `StellarNode` custom resource manifest (via {@link buildNodeManifest}).
 * 2. A `PodDisruptionBudget` manifest (via {@link buildPodDisruptionBudget}).
 *
 * Documents are separated by the standard YAML document separator `---`.
 * The resulting string can be written directly to a `.yaml` file and applied
 * with `kubectl apply -f`.
 *
 * If the topology contains no placed nodes an empty string is returned.
 *
 * @param state - The complete topology state from the configurator UI.
 * @param namespace - Optional namespace override.  When provided, all
 *   manifests use this namespace instead of each node's own `namespace`
 *   field.  Useful for targeting a specific cluster environment (staging,
 *   production, etc.).
 * @returns A single string containing all manifests separated by `---\n`,
 *   or `''` when `state.placedNodes` is empty.
 *
 * @example
 * ```ts
 * const yaml = buildManifests(topologyState, 'stellar-staging');
 * navigator.clipboard.writeText(yaml);
 * ```
 */
export function buildManifests(state: TopologyState, namespace?: string): string {
  if (state.placedNodes.length === 0) return '';

  const documents: string[] = [];

  for (const node of state.placedNodes) {
    // Apply namespace override when supplied
    const effectiveNode: PlacedStellarNode =
      namespace !== undefined ? { ...node, namespace } : node;

    documents.push(buildNodeManifest(effectiveNode, state.zones, state.workerNodes));
    documents.push(buildPodDisruptionBudget(effectiveNode));
  }

  // Join with the YAML document separator; each manifest already ends with '\n'
  return documents.join('---\n');
}
