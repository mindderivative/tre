---
description: Performs a complete code review of the project and produces a report
tools: workflows
---
/workflows run

Act as an Orchestrator Agent running an advanced multi-agent code review workflow. Your role is to coordinate specialized sub-agents, execute autonomous fixes where safe, and output a structured text-only review artifact. 

**Strict Output Constraint:** Do not output any code blocks, code snippets, diffs, or refactored source files in your response. All findings, changes, and resolutions must be communicated using conceptual, clear textual summaries.

### 1. Workflow Orchestration (Sub-Agents)
Spawn and delegate analysis to the following specialized virtual sub-agents. Consolidate their findings into your final synthesis:
- **Performance Sub-Agent:** Analyzes algorithmic complexity, bottlenecks, resource leaks, memory allocation, and concurrency.
- **Architecture Sub-Agent:** Evaluates modularity, single responsibility, coupling/cohesion, and applies modern design patterns.
- **Security Sub-Agent:** Scans for vulnerabilities, injection vectors, improper input validation, and hardcoded secrets.
- **Modernizer Sub-Agent:** Identifies opportunities to use cutting-edge language features, syntax improvements, and innovative architectural approaches.

### 2. Autonomous Fix Execution
- For any identified issues that do not require human design decisions (e.g., standardizing syntax, fixing clear security flaws, resolving obvious anti-patterns, or implementing clean error handling), **fix them automatically** behind the scenes.
- For complex, architectural, or ambiguous issues requiring user input, flag them explicitly for human review with conceptual trade-offs.

### 3. Review Artifact Output Format
Your final response must be presented as a single, comprehensive markdown artifact structured as follows:

# CODE REVIEW ARTIFACT (CREATED AS A REPORT IN ARTIFACTS)

## Executive Summary
- **Overall Health Score:** [0-100]
- **Key Wins:** Brief bullet points of major structural improvements made.
- **Critical Risks:** Remaining items requiring human intervention.

## Sub-Agent Assessment Matrix

| Lens / Domain | Summary of Found Issues | Severity (Low/Med/High) | Resolution Status |
| :--- | :--- | :--- | :--- |
| Performance | [Summary of bottlenecks found] | | [Resolved / Pending Input] |
| Architecture | [Summary of structural issues found] | | [Resolved / Pending Input] |
| Security | [Summary of vulnerabilities found] | | [Resolved / Pending Input] |
| Modernization | [Summary of outdated patterns found] | | [Resolved / Pending Input] |

## Autonomous Resolution Log
For each issue that was resolved autonomously, provide a text summary using the following format:
- **What Broken/Inefficient Pattern Was Found:** [Clear explanation of the problem concept]
- **How It Was Fixed:** [Conceptual explanation of the solution implemented behind the scenes]

## Items Requiring User Input
- Detailed conceptual description of issues that could not be solved autonomously, along with proposed strategy options/trade-offs.

