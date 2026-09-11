# repository-detection Specification

## Purpose
Provide observable and verifiable repository detection behavior for Spec Autonomous users while preserving clear execution and compatibility boundaries.

## Requirements

### Requirement: Discover supported framework markers
The CLI SHALL provide read-only detection of recognizable OpenSpec and Spec Kit markers from a selected directory and SHALL report the evidence rather than claiming execution readiness.

#### Scenario: Inspect OpenSpec
- **WHEN** a repository contains openspec/config.yaml
- **THEN** detect reports OpenSpec and its marker evidence

### Requirement: Honor repository boundaries
Detection SHALL find the nearest marked project without traversing beyond a Git boundary and SHALL ignore nested reference checkouts unless explicitly selected.

#### Scenario: Inspect a repository containing references
- **WHEN** the root contains only .references with upstream examples
- **THEN** detect does not infer that those examples configure the current repository

### Requirement: Expose ambiguity and invalid selection
The CLI SHALL report multiple detected frameworks without an automatic selection and SHALL fail an explicitly requested framework that was not detected.

#### Scenario: Both frameworks exist
- **WHEN** OpenSpec and Spec Kit markers coexist
- **THEN** JSON reports ambiguous true until a specific detected framework is selected

### Requirement: Avoid script and symlink side effects
Detection SHALL not execute repository scripts and SHALL ignore symlinked framework markers and report incomplete setup without writing files.

#### Scenario: Symlinked framework
- **WHEN** openspec is a symlink to an external directory
- **THEN** the marker is ignored and a diagnostic is returned
