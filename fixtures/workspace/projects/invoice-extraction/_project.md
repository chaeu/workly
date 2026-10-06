---
key: IE
title: Invoice Extraction
status: active
color: proj-2
order: 2
repos: [~/repos/invoice-extraction]
links: []
created: 2026-09-01
usecase:
  type: ai
  area: Finance
  step: pilot
  step_since: 2026-09-24
  status: active
  blocked_by: null
  next_step: Evaluate pilot results with finance team
  current_state: Pilot with 20 sample invoices, 85 % fields correct
  decisions:
    - { date: 2026-09-10, gate: null, text: "Start pilot, timebox 3 weeks" }
---
Extract header fields from PDF invoices with an LLM, human check for low-confidence fields.
