---
title: Multi-Cluster High Availability Architecture & Failover Guide
sidebar_label: Multi-Cluster HA
---

# Multi-Cluster High Availability Architecture & Failover Guide

This guide provides an end-to-end blueprint for deploying Stellar-K8s across
multiple geographical Kubernetes clusters. It covers topology design
(Active-Passive vs Active-Active RPC nodes with a Single Primary Validator),
GitOps-driven deployment, external DNS failover, and cross-cluster mTLS
networking.

The reference manifests live in [`examples/multi-cluster/`](../../examples/multi-cluster/).
All manifests are complete, functional, and copy-pasteable.

---

## 1. Architecture Overview

### 1.1 C4 Context Diagram

The system is composed of two Kubernetes clusters (`cluster-a` and `cluster-b`)
in different regions, connected over a private virtual bridge network. A global
DNS provider (ExternalDNS) routes client traffic to the active region.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              External Clients                                │
│                        (Wallets, DEXs, dApps, SDKs)                          │
└───────────────────────────────────┬─────────────────────────────────────────┘
                                    │ HTTPS (443)
                                    ▼
                        ┌───────────────────────┐
                        │   Global DNS / GLB    │
                        │  horizon.stellar.io   │
                        │  (ExternalDNS + LB)   │
                        └───────────┬───────────┘
                                    │
              ┌─────────────────────┴─────────────────────┐
              │                                           │
              ▼                                           ▼
┌─────────────────────────────┐            ┌─────────────────────────────┐
│        Cluster A            │            │        Cluster B            │
│        (Primary)            │            │        (Secondary)          │
│  region: us-east-1          │            │  region: eu-west-1          │
│                             │            │                             │
│  ┌───────────────────────┐  │            │  ┌───────────────────────┐  │
│  │  StellarNode          │  │            │  │  StellarNode          │  │
│  │  Validator (Primary)  │  │            │  │  Validator (Standby)  │  │
│  │  Horizon (Active)     │  │            │  │  Horizon (Standby)    │  │
│  │  Soroban RPC (Active) │  │            │  │  Soroban RPC (Active) │  │
│  └───────────────────────┘  │            │  └───────────────────────┘  │
│                             │            │                             │
│  Ingress (nginx)            │            │  Ingress (nginx)            │
│  cert-manager               │            │  cert-manager               │
│  ExternalDNS                │            │  ExternalDNS                │
└─────────────┬───────────────┘            └─────────────┬───────────────┘
              │                                           │
              └───────────────────┬───────────────────────┘
                                  │
                    ┌─────────────────────────────┐
                    │  Virtual Bridge Network     │
                    │  (WireGuard / Submariner /   │
                    │   VPC Peering)               │
                    │  mTLS on port 11625/11626    │
                    └─────────────────────────────┘
```

### 1.2 C4 Container Diagram (Cluster A)

```
┌───────────────────────────────────────────────────────────────────────────┐
│                              Cluster A (Primary)                          │
│                                                                            │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐   ┌────────────┐  │
│  │  Ingress     │──▶│  Horizon     │   │  Soroban RPC │   │  Validator │  │
│  │  Controller  │   │  (Active)    │   │  (Active)    │   │  (Primary) │  │
│  │  (nginx)     │   └──────┬───────┘   └──────┬───────┘   └─────┬──────┘  │
│  └──────────────┘          │                  │                 │         │
│                            │                  │                 │         │
│                     ┌──────▼──────┐    ┌──────▼──────┐   ┌──────▼──────┐  │
│                     │ PostgreSQL  │    │  Stellar    │   │  Stellar    │  │
│                     │ (Horizon DB)│    │  Core       │   │  Core       │  │
│                     └─────────────┘    │  (peer)     │   │  (peer)     │  │
│                                        └─────────────┘   └─────────────┘  │
│                                                                            │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐                   │
│  │ ExternalDNS  │   │ cert-manager │   │ Stellar-K8s  │                   │
│  │              │   │              │   │ Operator     │                   │
│  └──────────────┘   └──────────────┘   └──────────────┘                   │
└───────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Topology Designs

#### Active-Passive (Recommended for Validators)

One cluster hosts the **Single Primary Validator** that participates in the
Stellar network quorum. The secondary cluster runs a **standby validator** that
is fully synced but does not vote. On failure, the standby is promoted.

- **Pros:** Simple quorum management, deterministic failover, lower cross-region
  bandwidth.
- **Cons:** Standby capacity is idle until failover.

#### Active-Active RPC Nodes with Single Primary Validator

Horizon and Soroban RPC nodes run **active in both regions** and serve traffic
concurrently. Only the validator is single-primary. This maximizes read
availability and reduces failover RTO for the API layer.

- **Pros:** Read traffic served from the nearest region; validator quorum stays
  simple.
- **Cons:** Requires cross-region state replication for Horizon's PostgreSQL and
  careful write-path handling.

---

## 2. Networking Model

### 2.1 Cross-Cluster Connectivity

The two clusters are connected over a **virtual bridge network**. The reference
setup uses two local `kind` clusters joined by a WireGuard bridge, but the same
topology applies to VPC peering, Submariner, or Cilium ClusterMesh.

| Layer          | Technology                          | Ports            |
|----------------|-------------------------------------|------------------|
| L3/L4 bridge   | WireGuard / VPC Peering / Submariner| UDP 51820 (WG)   |
| Peer traffic   | Stellar Core peer protocol          | TCP 11625        |
| HTTP API       | Horizon / Soroban RPC               | TCP 11626        |
| mTLS           | cert-manager + Istio/SPIFFE         | TCP 8443         |

### 2.2 Traffic Routing

```
Client ──▶ Global DNS ──▶ Cluster A Ingress ──▶ Horizon (Active)
                              │
                              └──▶ Soroban RPC (Active)

Validator A ──(mTLS, 11625)──▶ Validator B (Standby, synced)
```

### 2.3 State Replication

- **Horizon PostgreSQL:** Logical replication from Cluster A to Cluster B
  (see `docs/cross-cloud-failover.md` for the full CNPG/PostgreSQL setup).
- **Stellar Core ledger:** The standby validator syncs from the primary via the
  peer protocol; no manual state copy is required.
- **History archives:** Both clusters read from the same public history archive
  endpoints.

### 2.4 Quorum Communication

The Single Primary Validator in Cluster A is the only voting node. The standby
in Cluster B is configured with `QUORUM_SET` that references the primary, so it
stays in sync without voting. On failover, the standby's quorum weight is raised
and it becomes the primary.

---

## 3. Deployment with GitOps

### 3.1 Repository Layout

```
gitops/
├── clusters/
│   ├── cluster-a/
│   │   ├── kustomization.yaml
│   │   └── stellar/
│   │       ├── validator-primary.yaml
│   │       ├── horizon-active.yaml
│   │       ├── soroban-active.yaml
│   │       ├── ingress.yaml
│   │       └── external-dns.yaml
│   └── cluster-b/
│       ├── kustomization.yaml
│       └── stellar/
│           ├── validator-standby.yaml
│           ├── horizon-standby.yaml
│           ├── soroban-active.yaml
│           ├── ingress.yaml
│           └── external-dns.yaml
└── apps/
    └── stellar-operator/
        └── kustomization.yaml
```

### 3.2 ArgoCD Application (Cluster A)

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: stellar-cluster-a
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/your-org/gitops.git
    targetRevision: main
    path: clusters/cluster-a
  destination:
    server: https://kubernetes.default.svc
    namespace: stellar-nodes
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
```

### 3.3 Flux Kustomization (Cluster B)

```yaml
apiVersion: kustomize.toolkit.fluxcd.io/v1
kind: Kustomization
metadata:
  name: stellar-cluster-b
  namespace: flux-system
spec:
  interval: 5m
  path: ./clusters/cluster-b
  prune: true
  sourceRef:
    kind: GitRepository
    name: gitops
  targetNamespace: stellar-nodes
```

---

## 4. External DNS Failover

ExternalDNS manages the global DNS record. The primary cluster owns the
`horizon.stellar.io` record with a low TTL. On failover, the record is updated
to point at Cluster B's ingress IP.

### 4.1 ExternalDNS Deployment (per cluster)

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: external-dns
  namespace: kube-system
spec:
  replicas: 1
  selector:
    matchLabels:
      app: external-dns
  template:
    metadata:
      labels:
        app: external-dns
    spec:
      serviceAccountName: external-dns
      containers:
        - name: external-dns
          image: registry.k8s.io/external-dns/external-dns:v0.14.2
          args:
            - --source=service
            - --source=ingress
            - --provider=aws
            - --policy=upsert-only
            - --registry=txt
            - --txt-owner-id=stellar-cluster-a
            - --txt-prefix=stellar-
            - --domain-filter=stellar.io
            - --interval=30s
          env:
            - name: AWS_REGION
              value: us-east-1
```

### 4.2 Failover Record Update

On failover, update the DNS record to point at Cluster B:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: horizon-ingress
  namespace: stellar-nodes
  annotations:
    external-dns.alpha.kubernetes.io/hostname: horizon.stellar.io
    external-dns.alpha.kubernetes.io/ttl: "30"
    external-dns.alpha.kubernetes.io/aws-weight: "100"
spec:
  type: LoadBalancer
  selector:
    app.kubernetes.io/name: stellar-node
    app.kubernetes.io/instance: horizon
  ports:
    - name: http
      port: 80
      targetPort: 8000
```

---

## 5. Cross-Cluster mTLS Networking

### 5.1 Certificate Authority

Create a shared CA that both clusters trust:

```bash
openssl genrsa -out ca.key 4096
openssl req -new -x509 -days 3650 -key ca.key -out ca.crt \
  -subj "/CN=stellar-multi-cluster-ca"
```

Install the CA into both clusters:

```bash
kubectl create secret tls stellar-ca \
  --cert=ca.crt --key=ca.key -n stellar-nodes --context=cluster-a
kubectl create secret tls stellar-ca \
  --cert=ca.crt --key=ca.key -n stellar-nodes --context=cluster-b
```

### 5.2 cert-manager ClusterIssuer (both clusters)

```yaml
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: stellar-ca-issuer
spec:
  ca:
    secretName: stellar-ca
```

### 5.3 mTLS Certificate for Peer Traffic

```yaml
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: validator-peer-cert
  namespace: stellar-nodes
spec:
  secretName: validator-peer-tls
  duration: 2160h
  renewBefore: 360h
  commonName: validator.stellar-nodes.svc.cluster.local
  dnsNames:
    - validator.stellar-nodes.svc.cluster.local
    - validator-cluster-a.stellar.io
    - validator-cluster-b.stellar.io
  issuerRef:
    name: stellar-ca-issuer
    kind: ClusterIssuer
```

### 5.4 Istio PeerAuthentication (mTLS STRICT)

```yaml
apiVersion: security.istio.io/v1beta1
kind: PeerAuthentication
metadata:
  name: stellar-mtls
  namespace: stellar-nodes
spec:
  mtls:
    mode: STRICT
  selector:
    matchLabels:
      app.kubernetes.io/name: stellar-node
```

---

## 6. Reference Manifests

The complete, copy-pasteable manifests are in
[`examples/multi-cluster/`](../../examples/multi-cluster/):

| File | Purpose |
|------|---------|
| `cluster-a/validator-primary.yaml` | Single Primary Validator (Cluster A) |
| `cluster-a/horizon-active.yaml` | Active Horizon (Cluster A) |
| `cluster-a/soroban-active.yaml` | Active Soroban RPC (Cluster A) |
| `cluster-a/ingress.yaml` | Ingress + cert-manager (Cluster A) |
| `cluster-a/external-dns.yaml` | ExternalDNS deployment (Cluster A) |
| `cluster-b/validator-standby.yaml` | Standby Validator (Cluster B) |
| `cluster-b/horizon-standby.yaml` | Standby Horizon (Cluster B) |
| `cluster-b/soroban-active.yaml` | Active Soroban RPC (Cluster B) |
| `cluster-b/ingress.yaml` | Ingress + cert-manager (Cluster B) |
| `cluster-b/external-dns.yaml` | ExternalDNS deployment (Cluster B) |
| `mtls/ca.yaml` | Shared CA + cert-manager issuer |
| `mtls/peer-certificate.yaml` | mTLS peer certificate |
| `mtls/peer-authentication.yaml` | Istio STRICT mTLS |
| `kind/bridge-network.sh` | Two-cluster kind bridge setup |
| `kind/cluster-a.yaml` | kind config for Cluster A |
| `kind/cluster-b.yaml` | kind config for Cluster B |

---

## 7. Validation on Two Local kind Clusters

The reference setup in `examples/multi-cluster/kind/` provisions two `kind`
clusters connected over a virtual bridge network. Run:

```bash
cd examples/multi-cluster/kind
./bridge-network.sh
```

This script:

1. Creates `kind-cluster-a` and `kind-cluster-b`.
2. Joins both to a shared Docker bridge network.
3. Installs the Stellar-K8s operator, cert-manager, ExternalDNS, and Istio.
4. Applies the Cluster A and Cluster B manifests.
5. Verifies cross-cluster peer connectivity on port 11625.

---

## 8. Failover Procedure

### 8.1 Detecting Failure

Monitor the primary validator's health:

```bash
kubectl get stellarnode validator-primary -n stellar-nodes --context=cluster-a
# Expect Ready=True
```

### 8.2 Promoting the Standby

```bash
# 1. Quiesce the primary validator
kubectl patch stellarnode validator-primary -n stellar-nodes \
  --type=merge -p '{"spec":{"replicas":0}}' --context=cluster-a

# 2. Promote the standby validator in Cluster B
kubectl patch stellarnode validator-standby -n stellar-nodes \
  --type=merge -p '{"spec":{"validatorConfig":{"quorumWeight":100}}}' \
  --context=cluster-b

# 3. Point DNS at Cluster B
kubectl annotate service horizon-ingress -n stellar-nodes \
  external-dns.alpha.kubernetes.io/aws-weight=100 --context=cluster-b
```

### 8.3 RTO / RPO

| Metric | Target |
|--------|--------|
| RTO (API layer) | < 2 min (Active-Active RPC) |
| RTO (Validator quorum) | < 5 min |
| RPO | Near-zero (logical replication) |

---

## 9. Related Documentation

- [`docs/cross-cloud-failover.md`](../cross-cloud-failover.md) — Horizon DB replication & GLB
- [`docs/dr-failover.md`](../dr-failover.md) — Manual DR failover procedure
- [`docs/peer-discovery.md`](../peer-discovery.md) — Cross-cluster peer discovery
- [`docs/mtls-guide.md`](../mtls-guide.md) — mTLS configuration
- [`docs/ingress-guide.md`](../ingress-guide.md) — Ingress + cert-manager
- [`docs/gitops/argocd.mdx`](../gitops/argocd.mdx) — ArgoCD GitOps golden path
