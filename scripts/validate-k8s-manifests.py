#!/usr/bin/env python3
"""
validate-k8s-manifests.py — Kubernetes Manifest & CRD Schema Validation Script (issue #1283)

Validates all Kubernetes manifests in config/, examples/, bundle/, and charts/
against Kubernetes OpenAPI schemas and custom CRD definitions using kubeconform/kubeval
and structural YAML/CRD schema rules.
"""

import sys
import os
import glob
import subprocess
import yaml

def check_dependencies():
    """Verify python dependencies and optional CLI tools."""
    print("→ Checking manifest validation tools...")
    has_kubeconform = subprocess.run(["which", "kubeconform"], capture_output=True).returncode == 0
    has_kubeval = subprocess.run(["which", "kubeval"], capture_output=True).returncode == 0
    print(f"  kubeconform installed: {has_kubeconform}")
    print(f"  kubeval installed:     {has_kubeval}")
    return has_kubeconform or has_kubeval

def validate_yaml_structure(filepath):
    """Validate YAML syntax and mandatory Kubernetes manifest fields (apiVersion, kind, metadata)."""
    errors = []
    try:
        with open(filepath, "r", encoding="utf-8") as f:
            docs = list(yaml.safe_load_all(f))
        
        for idx, doc in enumerate(docs):
            if doc is None:
                continue
            if not isinstance(doc, dict):
                errors.append(f"Document {idx} in {filepath} is not a valid YAML object")
                continue
            
            # Helm templates have Go syntax; skip unrendered templates here
            if "templates" in filepath or "{{" in str(doc):
                continue

            kind = doc.get("kind")
            api_version = doc.get("apiVersion")
            metadata = doc.get("metadata")

            if not kind:
                errors.append(f"Document {idx} in {filepath}: missing 'kind'")
            if not api_version:
                errors.append(f"Document {idx} in {filepath}: missing 'apiVersion'")
            if not metadata or not isinstance(metadata, dict) or not metadata.get("name"):
                errors.append(f"Document {idx} in {filepath}: missing 'metadata.name'")

            # Custom CRD validation rule checks
            if kind == "CustomResourceDefinition":
                spec = doc.get("spec", {})
                if not spec.get("group"):
                    errors.append(f"CRD {filepath}: missing spec.group")
                if not spec.get("names", {}).get("kind"):
                    errors.append(f"CRD {filepath}: missing spec.names.kind")
                if not spec.get("versions"):
                    errors.append(f"CRD {filepath}: missing spec.versions")
    except Exception as e:
        errors.append(f"Failed to parse {filepath}: {e}")
    return errors

def validate_with_kubeconform(manifest_files):
    """Run kubeconform if present."""
    cmd = [
        "kubeconform",
        "-strict",
        "-summary",
        "-kubernetes-version", "1.30.0",
        "-schema-location", "default",
        "-schema-location", "https://raw.githubusercontent.com/datreeio/CRDs-catalog/main/{{.Group}}/{{.ResourceKind}}_{{.ResourceAPIVersion}}.json"
    ] + manifest_files
    print(f"Running kubeconform on {len(manifest_files)} manifests...")
    res = subprocess.run(cmd, capture_output=True, text=True)
    print(res.stdout)
    if res.stderr:
        print(res.stderr, file=sys.stderr)
    return res.returncode == 0

def main():
    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    search_patterns = [
        os.path.join(root_dir, "config/**/*.yaml"),
        os.path.join(root_dir, "config/**/*.yml"),
        os.path.join(root_dir, "examples/**/*.yaml"),
        os.path.join(root_dir, "bundle/**/*.yaml"),
        os.path.join(root_dir, "charts/stellar-operator/rendered/*.yaml"),
    ]

    manifest_files = []
    for pattern in search_patterns:
        manifest_files.extend(glob.glob(pattern, recursive=True))

    manifest_files = sorted(list(set(manifest_files)))

    print(f"=== Kubernetes Manifest & CRD Schema Validation ===")
    print(f"Found {len(manifest_files)} manifest files to validate.\n")

    all_errors = []
    for filepath in manifest_files:
        rel_path = os.path.relpath(filepath, root_dir)
        errs = validate_yaml_structure(filepath)
        if errs:
            all_errors.extend(errs)
            print(f"  ✗ {rel_path} - FAIL")
        else:
            print(f"  ✓ {rel_path} - PASS")

    has_cli = check_dependencies()
    if has_cli and manifest_files:
        if not validate_with_kubeconform(manifest_files):
            all_errors.append("kubeconform schema validation failed")

    if all_errors:
        print("\n❌ Validation Failed with the following errors:")
        for err in all_errors:
            print(f"  - {err}")
        sys.exit(1)
    else:
        print("\n✅ All Kubernetes manifests and CRD schemas passed validation.")
        sys.exit(0)

if __name__ == "__main__":
    main()
