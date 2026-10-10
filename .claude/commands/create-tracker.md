---
description: Generates a hierarchical project progress tracker with interactive or visual status indicators
---

# Build Tracker Generator

You are a Principal Engineer and Lead Tech Architect. Analyze the current project state, implementation documentation, and outstanding tasks to generate a structured, scannable **Build Tracker** as a Markdown file for claude use and as an interactive artifact, in parity, for me to use. 

## Layout Requirements

1. **Top Metrics & Insights:**
   - **Progress Bars:** Provide high-level completion bars for the active milestones.
   - **Just Closed:** A concise card summarizing the most recently completed technical implementations.
   - **Up Next:** A list of immediate next tasks or upcoming workstreams.
   - **Known Gaps:** Call out active technical debt, architectural holes, or unaddressed edge cases.

2. **Hierarchical Tracking Matrix:**
   Organize the remaining and ongoing work strictly using the following four-tier architecture:
   - **Milestones:** Top-level major features, significant epics, or core project domains.
   - **Phases:** Structural breakdowns of a Milestone (e.g., architectural concerns, sub-features).
   - **Stages:** Operational sequences or distinct workstreams within a Phase.
   - **Steps:** Concrete, low-level, atomic tasks that require direct action.

## Style & Execution

- Maintain absolute sync with existing implementation documentation.
- Use explicit visual anchors (such as progress indicators or status states) to show which sections can expand or contract.
- Keep the output fluff-free, dense, and highly scannable. 
- Focus heavily on structural verification, data integration, and tracking the technical state accurately over writing actual verbose code blocks.
