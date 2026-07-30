# Stellar-K8s API Reference

This directory contains the API reference and integration documentation for Stellar-K8s.

## Contents

- [StellarNode CRD Reference](../api-reference.md) — field-level CRD schema (generated)
- [OpenAPI Specification](openapi.yaml) — operator REST API (Swagger-compatible)
- [Webhook API](webhook.md)
- [Metrics API](metrics.md)
- [Client Libraries and SDK Guidance](client-libraries.md)
- [Error Codes and Troubleshooting](error-codes.md)

## Swagger / OpenAPI

| Artifact | Location |
|----------|----------|
| Static OpenAPI 3.0 spec | `docs/api/openapi.yaml` |
| Live JSON (API gateway) | `GET /gateway/openapi.json` |
| Interactive Swagger UI | `GET /developer` (when gateway enabled) |

Validate the static spec locally:

```bash
make generate-openapi-spec
make check-openapi-spec
```

## Overview

Stellar-K8s exposes the following integration layers:

- `StellarNode` CRD definitions and validation rules
- Operator REST API for cluster management, health, and diagnostics
- Admission webhook request/response validation for CRD operations
- Prometheus-compatible metrics and observability endpoints

## Notes

- The canonical CRD schema is documented in [docs/api-reference.md](../api-reference.md).
- Use the [OpenAPI specification](openapi.yaml) for code generation and API clients.
- Refer to [client-libraries.md](client-libraries.md) for SDK guidance and integration patterns.
