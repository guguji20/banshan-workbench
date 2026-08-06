# 华邦互娱商务系统 1.0 formal release evidence

This directory stores only final, reviewable gate attestations. Installers and runtime data remain in isolated CI artifacts or `.runtime` and must not be committed.

The only publishing workflow is `.github/workflows/promote-business-workbench-1.0.yml`. It refuses to publish unless Windows and macOS evidence comes from the same commit and all device, signing, upgrade, data-integrity, security, and five-case business gates pass.

Copy `business-workbench-1.0-final-gates.example.json` to `business-workbench-1.0-final-gates.json` only after replacing every blocked gate with verified evidence. The example intentionally cannot authorize a release.
