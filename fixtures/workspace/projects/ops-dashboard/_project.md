---
key: OPS
title: Ops Monitoring Dashboard
status: active
color: proj-3
order: 3
repos: [~/repos/ops-dashboard]
links: []
created: 2026-08-20
usecase:
  type: rule
  area: Operations
  step: infra
  step_since: 2026-09-12
  status: waiting
  blocked_by: null
  next_step: Wait for read access to the job database
  current_state: Concept approved, self-build without demand
  decisions:
    - { date: 2026-08-28, gate: G1, text: "Yes, pursue" }
    - { date: 2026-08-28, gate: Switch, text: "No demand needed, self-build" }
    - { date: 2026-09-11, gate: G3, text: "Approvals ok" }
---
Internal dashboard for nightly job runs, self-hosted.
