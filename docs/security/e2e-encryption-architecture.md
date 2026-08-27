# End-to-End Inter-Service Encryption Architecture & Certificate Management

This document defines the Zero-Trust End-to-End (E2E) Encryption architecture for inter-service communication across Stellar Core, Horizon, Soroban RPC, and companion services within the `Stellar-K8s` ecosystem (issue #1281).

---

## 1. Zero-Trust Networking Model

In accordance with modern security standards, all network traffic traversing cluster nodes or pod boundaries must be encrypted in transit using Mutual TLS (mTLS).

```text
                                 ┌───────────────────────┐
                                 │   Stellar Operator    │
                                 └───────────┬───────────┘
                                             │ (cert-manager CRDs / Vault PKI)
                                             ▼
 ┌──────────────────────┐   mTLS    ┌──────────────────────┐   mTLS    ┌──────────────────────┐
 │     Stellar Core     │ ◄───────► │       Horizon        │ ◄───────► │     Soroban RPC      │
 └──────────────────────┘           └──────────────────────┘           └──────────────────────┘
            ▲                                                                      ▲
            │                                 mTLS                                 │
            └──────────────────────────────────────────────────────────────────────┘
```

---

## 2. Certificate Authority Hierarchy & cert-manager Integration

Certificates are issued and automatically rotated via `cert-manager` integrated with either an internal CA Issuer or HashiCorp Vault PKI backend.

### Key Components:
- **`Issuer / ClusterIssuer`**: `stellar-inter-service-ca` provisions leaf TLS certificates for internal microservices.
- **`Certificate Resources`**:
  - `stellar-core-mtls-cert`
  - `horizon-mtls-cert`
  - `soroban-rpc-mtls-cert`
- **Key Parameters**:
  - **Algorithm**: `ECDSA` (P-256)
  - **Validity Duration**: 90 Days (`2160h`)
  - **Renewal Window**: 15 Days before expiration (`360h`)

---

## 3. Automated Zero-Downtime Certificate Rotation

To ensure zero downtime during certificate renewal:
1. `cert-manager` automatically generates a new secret when the `renewBefore` window (15 days) is reached.
2. Services watch mounted TLS secret volumes (`/etc/stellar/tls`) or leverage dynamic TLS reloader handles.
3. The operator monitors expiry via `ExpiryMonitor` Prometheus metrics (`stellar_cert_expiry_days`) and issues alerting thresholds (30-day warning, 7-day critical, 24-hour emergency).

---

## 4. Verification & Diagnostics

Verify mTLS secret creation and cert-manager status:

```bash
# Check cert-manager Certificates
kubectl -n stellar-system get certificate

# Inspect secret expiration
kubectl -n stellar-system get secret horizon-mtls-secret -o yaml
```
