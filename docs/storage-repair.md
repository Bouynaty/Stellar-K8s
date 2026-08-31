# Storage Corruption Recovery & Database Repair Playbook

This guide provides operational procedures for diagnosing and repairing database corruption within Stellar Core and Horizon pods running on Kubernetes.

## ⚠️ Safety First
- **Always backup the corrupted PVC** before attempting repairs. Use a volume snapshot or `kubectl cp`.
- Destructive commands are marked with **[DESTRUCTIVE]**. Proceed with caution.
- Attempting repair is usually faster than a full ledger resync, but not guaranteed.

---

## 1. Diagnostics: Identifying Corruption

### Signs of Corruption
- **Stellar Core:** Logs showing `Storage Error`, `database disk image is malformed`, or `Failed to apply ledger`.
- **Horizon:** Logs showing `pq: database "horizon" is corrupted` or `relation "history_ledgers" does not exist`.

### Identifying the Faulty PVC
Check which pod is in `CrashLoopBackOff`:
```bash
kubectl get pods -n stellar
kubectl describe pod <pod-name>
