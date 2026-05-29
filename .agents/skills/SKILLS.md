# PolyEdge Agent Skills Guide

A comprehensive guide to the skills available in the PolyEdge repository for Claude Code agents.

## Table of Contents

- [Overview](#overview)
- [Core Development Skills](#core-development-skills)
  - [feature-dev](#feature-dev)
  - [code-review](#code-review)
  - [simplify](#simplify)
  - [deep-research](#deep-research)
  - [verify](#verify)
  - [run](#run)
- [Version Control Skills](#version-control-skills)
  - [commit-commands:commit](#commit-commandscommit)
  - [commit-commands:commit-push-pr](#commit-commandscommit-push-pr)
  - [commit-commands:clean_gone](#commit-commandsclean_gone)
  - [review](#review)
- [Blockchain & Smart Contract Skills](#blockchain--smart-contract-skills)
  - [sui-stack-dev:sui-move-development](#sui-stack-devsui-move-development)
  - [sui-stack-dev:deploy-contract](#sui-stack-devdeploy-contract)
  - [sui-stack-dev:test-move](#sui-stack-devtest-move)
  - [sui-stack-dev:sui-typescript-sdk](#sui-stack-devsui-typescript-sdk)
  - [sui-stack-dev:sui-wallet-integration](#sui-stack-devsui-wallet-integration)
  - [sui-stack-dev:seal-encryption](#sui-stack-devseal-encryption)
  - [sui-stack-dev:walrus-storage](#sui-stack-devwalrus-storage)
  - [sui-stack-dev:init-dapp](#sui-stack-devinit-dapp)
  - [sui-stack-dev:sui-cli-usage](#sui-stack-devsui-cli-usage)
- [Design & Frontend Skills](#design--frontend-skills)
  - [frontend-design](#frontend-design)
  - [nextjs-shadcn](#nextjs-shadcn)
  - [tailwind-design-system](#tailwind-design-system)
  - [vercel-composition-patterns](#vercel-composition-patterns)
  - [vercel-react-best-practices](#vercel-react-best-practices)
  - [web-design-guidelines](#web-design-guidelines)
  - [accessibility](#accessibility)
- [Deployment & Infrastructure Skills](#deployment--infrastructure-skills)
  - [deploy-to-vercel](#deploy-to-vercel)
- [Tooling & Meta Skills](#tooling--meta-skills)
  - [update-config](#update-config)
  - [keybindings-help](#keybindings-help)
  - [fewer-permission-prompts](#fewer-permission-prompts)
  - [find-skills](#find-skills)
  - [loop](#loop)
  - [init](#init)
  - [security-review](#security-review)
- [Claude API Skills](#claude-api-skills)
  - [claude-api](#claude-api)
- [GLM Platform Skills](#glm-platform-skills)
  - [glm-plan-usage:usage-query](#glm-plan-usageusage-query)
  - [glm-plan-bug:case-feedback](#glm-plan-bugcase-feedback)

---

## Overview

Skills are structured prompt files stored in `.agents/skills/` that give Claude Code specialized capabilities. Each skill contains:

- **A YAML frontmatter block** (`---` delimited) with metadata
- **Required fields**: `name`, `description`
- **Optional fields**: `license`, `compatibility`, `metadata.allowed-tools`, `argument-hint`
- **Markdown body** with instructions, guardrails, and anti-patterns

Skills are triggered in two ways:
1. **Keyword matching** -- Claude automatically invokes a skill when user input matches trigger phrases defined in the description
2. **Explicit invocation** -- Users can call skills directly with `/skill-name` syntax

When a skill matches, Claude loads it **before** generating any other response.

---

## Core Development Skills

### feature-dev

**Location:** `.agents/skills/feature-dev/SKILL.md`
**Usage:** `/feature-dev <feature description>`
**Description:** Guided feature development with codebase understanding and architecture focus.

This skill provides a structured workflow for implementing new features:

1. **Context Gathering** -- Understands the existing codebase structure, patterns, and conventions
2. **Planning** -- Creates a detailed implementation plan before writing any code
3. **Implementation** -- Writes code following project conventions and best practices
4. **Verification** -- Ensures the feature works correctly

**When to use:**
- Building a new feature from scratch
- Adding significant functionality to existing code
- When you want structured guidance through the development process

---

### code-review

**Location:** `.agents/skills/code-review/SKILL.md`
**Usage:** `/code-review` or `/code-review --comment` or `/code-review --fix`
**Description:** Review the current diff for correctness bugs and reuse/simplification/efficiency cleanups.

**Modes:**
- **Default (low/medium effort):** Fewer, high-confidence findings
- **High effort:** Broader coverage, may include uncertain findings
- **`--comment`:** Posts findings as inline PR comments
- **`--fix`:** Applies the findings to the working tree after the review

**When to use:**
- Before committing changes
- When reviewing a pull request
- To catch bugs and improvement opportunities

---

### simplify

**Location:** `.agents/skills/simplify/SKILL.md`
**Usage:** `/simplify`
**Description:** Reviews changed code for reuse, simplification, and efficiency, then applies the fixes.

**Note:** Quality only -- does not hunt for bugs. Use `/code-review` for bug detection.

**When to use:**
- After implementing a feature to clean up the code
- To reduce complexity and improve readability
- To identify opportunities for code reuse

---

### deep-research

**Location:** `.agents/skills/deep-research/SKILL.md`
**Usage:** `/deep-research <research question>`
**Description:** Deep research harness -- fans out web searches, fetches sources, adversarially verifies claims, and synthesizes a cited report.

**Features:**
- Multi-source research with web search
- Adversarial verification of claims
- Citation-backed reports
- Handles clarifying questions for underspecified topics

**When to use:**
- Researching technical concepts or best practices
- Investigating unfamiliar technologies or libraries
- When you need fact-checked, multi-source information

---

### verify

**Location:** `.agents/skills/verify/SKILL.md`
**Usage:** `/verify`
**Description:** Verifies that a code change actually does what it's supposed to by running the app and observing behavior.

**When to use:**
- After implementing a fix to confirm it works
- Validating local changes before pushing
- Testing a feature manually
- Confirming a PR works as expected

---

### run

**Location:** `.agents/skills/run/SKILL.md`
**Usage:** `/run`
**Description:** Launches and drives this project's app to see a change working.

**Features:**
- First looks for a project-specific skill for launching the app
- Falls back to built-in patterns per project type (CLI, server, TUI, Electron, browser-driven, library)

**When to use:**
- Starting the application
- Taking a screenshot of the app
- Confirming a change works in the real app (not just tests)

---

## Version Control Skills

### commit-commands:commit

**Location:** `.agents/skills/commit-commands/commit/SKILL.md`
**Usage:** `/commit`
**Description:** Creates a git commit.

**When to use:**
- Committing staged changes
- When you want a standardized commit message

---

### commit-commands:commit-push-pr

**Location:** `.agents/skills/commit-commands/commit-push-pr/SKILL.md`
**Usage:** `/commit-push-pr`
**Description:** Commits changes, pushes to remote, and opens a pull request.

**When to use:**
- After completing a feature or fix
- When you want to quickly create a PR with all changes

---

### commit-commands:clean_gone

**Location:** `.agents/skills/commit-commands/clean-gone/SKILL.md`
**Usage:** `/clean_gone`
**Description:** Cleans up all git branches marked as `[gone]` (branches that have been deleted on the remote but still exist locally), including removing associated worktrees.

**When to use:**
- Housekeeping local branches after remote cleanup
- When `git branch` shows branches marked as `[gone]`

---

### review

**Location:** `.agents/skills/review/SKILL.md`
**Usage:** `/review`
**Description:** Reviews a pull request.

**When to use:**
- Reviewing a PR before merging
- Getting feedback on code changes

---

## Blockchain & Smart Contract Skills

### sui-stack-dev:sui-move-development

**Location:** `.agents/skills/sui-stack-dev/sui-move-development/SKILL.md`
**Usage:** `/sui-move-development <task description>`
**Description:** Guided Sui Move module development -- code, tests, publish.

**When to use:**
- Writing new Sui Move modules or packages
- Adding functionality to existing Move code
- Structuring Move projects

---

### sui-stack-dev:deploy-contract

**Location:** `.agents/skills/sui-stack-dev/deploy-contract/SKILL.md`
**Usage:** `/deploy-contract`
**Description:** Interactively deploys a Move package to the Sui network with pre-deployment checks.

**When to use:**
- Deploying Move packages to Sui mainnet, testnet, or devnet
- When you want guided deployment with verification

---

### sui-stack-dev:test-move

**Location:** `.agents/skills/sui-stack-dev/test-move/SKILL.md`
**Usage:** `/test-move <test query or path>`
**Description:** Runs Move tests with filtering, coverage, and gas profiling options.

**When to use:**
- Running Move unit tests
- Generating test coverage reports
- Profiling gas usage in tests

---

### sui-stack-dev:sui-typescript-sdk

**Location:** `.agents/skills/sui-stack-dev/sui-typescript-sdk/SKILL.md`
**Usage:** `/sui-typescript-sdk <task description>`
**Description:** Build TypeScript clients with the Sui SDK.

**When to use:**
- Building TypeScript applications that interact with Sui
- Querying on-chain data
- Executing transactions from TypeScript

---

### sui-stack-dev:sui-wallet-integration

**Location:** `.agents/skills/sui-stack-dev/sui-wallet-integration/SKILL.md`
**Usage:** `/sui-wallet-integration <integration task>`
**Description:** Integrate Sui wallets into dApps.

**When to use:**
- Adding wallet connection to a dApp
- Implementing transaction signing
- Building wallet UI components

---

### sui-stack-dev:seal-encryption

**Location:** `.agents/skills/sui-stack-dev/seal-encryption/SKILL.md`
**Usage:** `/seal-encryption <task description>`
**Description:** Encrypt and decrypt data using Seal on Sui.

**When to use:**
- Implementing encryption for on-chain or off-chain data
- Managing encryption keys on Sui
- Building access-controlled data storage

---

### sui-stack-dev:walrus-storage

**Location:** `.agents/skills/sui-stack-dev/walrus-storage/SKILL.md`
**Usage:** `/walrus-storage <storage task>`
**Description:** Use Walrus for decentralized storage on Sui.

**When to use:**
- Storing large data blobs on Walrus
- Building applications that use decentralized storage
- Managing Walrus storage operations

---

### sui-stack-dev:init-dapp

**Location:** `.agents/skills/sui-stack-dev/init-dapp/SKILL.md`
**Usage:** `/init-dapp`
**Description:** Initializes a new Sui dApp project using the official scaffolding tool.

**When to use:**
- Starting a new Sui dApp from scratch
- When you want a project scaffolded with best practices

---

### sui-stack-dev:sui-cli-usage

**Location:** `.agents/skills/sui-stack-dev/sui-cli-usage/SKILL.md`
**Usage:** `/sui-cli-usage <CLI task>`
**Description:** Explains and guides usage of the Sui CLI tool.

**When to use:**
- Learning Sui CLI commands
- Managing Sui accounts, objects, or packages via CLI
- When you need help with specific CLI operations

---

## Design & Frontend Skills

### frontend-design

**Location:** `.agents/skills/frontend-design/SKILL.md`
**Usage:** `/frontend-design <design task>`
**Description:** Designs and codes high-end, visually refined web interfaces.

**Key capabilities:**
- Deep design thinking with color, typography, spacing, and motion systems
- Visual polish and restraint
- Component-based architecture
- Integration with React, Tailwind CSS, shadcn/ui, Radix primitives, and Lucide icons
- Always tests with Puppeteer after every change

**When to use:**
- Building polished, production-ready UIs
- Designing landing pages, dashboards, or application interfaces
- When visual quality and refinement matter

---

### nextjs-shadcn

**Location:** `.agents/skills/nextjs-shadcn/SKILL.md`
**Usage:** `/nextjs-shadcn <task description>`
**Description:** Creates Next.js 16 frontends with shadcn/ui.

**When to use:**
- Building React UIs, components, pages, or applications
- Working with shadcn, Tailwind, or modern frontend patterns
- Creating new Next.js projects
- Adding UI components or styling pages

---

### tailwind-design-system

**Location:** `.agents/skills/tailwind-design-system/SKILL.md`
**Usage:** `/tailwind-design-system <task description>`
**Description:** Builds scalable design systems with Tailwind CSS v4.

**When to use:**
- Creating component libraries
- Implementing design systems
- Standardizing UI patterns
- Working with design tokens

---

### vercel-composition-patterns

**Location:** `.agents/skills/vercel-composition-patterns/SKILL.md`
**Usage:** `/vercel-composition-patterns <task description>`
**Description:** React composition patterns that scale.

**When to use:**
- Refactoring components with boolean prop proliferation
- Building flexible component libraries
- Designing reusable APIs
- Working with compound components, render props, or context providers

---

### vercel-react-best-practices

**Location:** `.agents/skills/vercel-react-best-practices/SKILL.md`
**Usage:** `/vercel-react-best-practices <task description>`
**Description:** React and Next.js performance optimization guidelines from Vercel Engineering.

**When to use:**
- Writing, reviewing, or refactoring React/Next.js code
- Optimizing performance
- Working with bundle optimization or data fetching

---

### web-design-guidelines

**Location:** `.agents/skills/web-design-guidelines/SKILL.md`
**Usage:** `/web-design-guidelines <task description>`
**Description:** Reviews UI code for Web Interface Guidelines compliance.

**When to use:**
- Reviewing UI code
- Checking accessibility
- Auditing design
- Reviewing UX
- Checking a site against best practices

---

### accessibility

**Location:** `.agents/skills/accessibility/SKILL.md`
**Usage:** `/accessibility <task or URL or file path>`
**Description:** Audits and improves web accessibility following WCAG 2.2 guidelines.

**When to use:**
- Improving accessibility
- Running an a11y audit
- Ensuring WCAG compliance
- Adding screen reader support
- Improving keyboard navigation
- Making a site accessible

---

## Deployment & Infrastructure Skills

### deploy-to-vercel

**Location:** `.agents/skills/deploy-to-vercel/SKILL.md`
**Usage:** `/deploy-to-vercel`
**Description:** Deploys applications and websites to Vercel.

**When to use:**
- Deploying an app to Vercel
- Getting a deployment link
- Pushing code live
- Creating a preview deployment

---

## Tooling & Meta Skills

### update-config

**Location:** `.agents/skills/update-config/SKILL.md`
**Usage:** `/update-config <configuration task>`
**Description:** Configures the Claude Code harness via `settings.json`.

**When to use:**
- Setting up automated behaviors ("from now on when X", "each time I do Y")
- Configuring hooks in `settings.json`
- Managing permissions ("allow X", "move permission to")
- Setting environment variables ("set X=Y")
- Troubleshooting hooks
- Any changes to `settings.json` or `settings.local.json` files

**Note:** For simple settings like theme or model, suggest the `/config` command instead.

---

### keybindings-help

**Location:** `.agents/skills/keybindings-help/SKILL.md`
**Usage:** `/keybindings-help <keybinding task>`
**Description:** Customizes keyboard shortcuts, rebinds keys, adds chord bindings, or modifies `~/.claude/keybindings.json`.

**When to use:**
- Rebinding keyboard shortcuts (e.g., "rebind ctrl+s")
- Adding chord shortcuts
- Changing the submit key
- Customizing keybindings

---

### fewer-permission-prompts

**Location:** `.agents/skills/fewer-permission-prompts/SKILL.md`
**Usage:** `/fewer-permission-prompts`
**Description:** Scans your transcripts for common read-only Bash and MCP tool calls, then adds a prioritized allowlist to project `.claude/settings.json` to reduce permission prompts.

**When to use:**
- When you're tired of approving the same read-only commands
- To streamline your workflow by pre-approving common operations

---

### find-skills

**Location:** `.agents/skills/find-skills/SKILL.md`
**Usage:** `/find-skills`
**Description:** Helps users discover and install agent skills.

**When to use:**
- When asking "how do I do X"
- When looking for "a skill for X"
- When asking "is there a skill that can..."
- When expressing interest in extending capabilities

---

### loop

**Location:** `.agents/skills/loop/SKILL.md`
**Usage:** `/loop <interval> <command>` (e.g., `/loop 5m /foo`)
**Description:** Runs a prompt or slash command on a recurring interval.

**Features:**
- Omit the interval to let the model self-pace
- Uses `CronCreate` behind the scenes

**When to use:**
- Setting up a recurring task
- Polling for status
- Running something repeatedly on an interval (e.g., "check the deploy every 5 minutes")

**Do NOT use for:** One-off tasks

---

### init

**Location:** `.agents/skills/init/SKILL.md`
**Usage:** `/init`
**Description:** Bootstraps Claude Code for this project.

**What it does:**
1. Scans for existing CLAUDE.md, `.claude/`, `AGENTS.md`, and agent-specific docs
2. Skips sections that are already documented
3. Generates well-structured CLAUDE.md with project fundamentals
4. Asks 2-3 clarifying questions after generating the draft

**Sections it creates (in order):**
1. Project Overview (always included)
2. Build & Development Commands (always included)
3. Code Style Guidelines (if code exists)
4. Testing Guidelines (if tests exist)
5. Repository Structure (if multi-directory)
6. Important Instructions & Constraints (from config files)

---

### security-review

**Location:** `.agents/skills/security-review/SKILL.md`
**Usage:** `/security-review`
**Description:** Performs a comprehensive security review.

---

## Claude API Skills

### claude-api

**Location:** `.agents/skills/claude-api/SKILL.md`
**Usage:** `/claude-api <task description>`
**Description:** Build, debug, and optimize Claude API / Anthropic SDK apps.

**Features:**
- Apps built with this skill include prompt caching
- Handles migrating existing Claude API code between Claude model versions (4.5 to 4.6, 4.6 to 4.7, retired-model replacements)

**Triggers when:**
- Code imports `anthropic` or `@anthropic-ai/sdk`
- User asks for the Claude API, Anthropic SDK, or Managed Agents
- User adds, modifies, or tunes a Claude feature (caching, thinking, compaction, tool use, batch, files, citations, memory) or model (Opus/Sonnet/Haiku) in a file
- Questions about prompt caching or cache hit rate in an Anthropic SDK project

**Skips when:**
- File imports `openai` or other-provider SDK
- Filename like `*-openai.py` or `*-generic.py`
- Provider-neutral code
- General programming/ML

---

## GLM Platform Skills

### glm-plan-usage:usage-query

**Location:** `.agents/skills/glm-plan-usage/SKILL.md`
**Usage:** `/usage-query`
**Description:** Queries the usage information for the current account.

**When to use:**
- Checking API usage
- Reviewing account consumption
- Monitoring usage limits

---

### glm-plan-bug:case-feedback

**Location:** `.agents/skills/glm-plan-bug/SKILL.md`
**Usage:** `/case-feedback`
**Description:** Submits case feedback to report issues or suggestions for the current conversation.

**When to use:**
- Reporting bugs encountered during a session
- Suggesting improvements to the platform
- Providing feedback on Claude Code behavior

---

## How Skills Work Internally

### Skill File Structure

Every skill file follows this structure:

```markdown
---
name: skill-name
description: Brief description of what the skill does. This text is used for keyword matching.
argument-hint: Optional hint for what arguments the skill expects (e.g., "<task description>")
---

# Skill Name

Main instructions go here...

## Sub-sections
- Guardrails
- Anti-patterns
- Specific instructions
```

### Metadata Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Unique identifier for the skill |
| `description` | Yes | Used for keyword matching and display; include trigger phrases |
| `argument-hint` | No | Shows users what arguments to pass (e.g., `<task description>`) |
| `license` | No | License identifier (e.g., `Apache-2.0`) |
| `compatibility` | No | Target platform (e.g., `claude-code`, `claude-desktop`, `claude-api`) |
| `metadata.allowed-tools` | No | List of tools the skill is allowed to use |

### Skills vs. Hooks

| Aspect | Skills | Hooks |
|--------|--------|-------|
| **Purpose** | Structured prompts for guidance | Automated shell commands |
| **Trigger** | User input matching or explicit invocation | Specific events (PreToolUse, PostToolUse, etc.) |
| **Format** | Markdown files with YAML frontmatter | Shell commands in `settings.json` |
| **Interaction** | Can guide Claude through complex workflows | Run silently without Claude interaction |
| **Location** | `.agents/skills/*/SKILL.md` | `settings.json` hooks array |

### Adding New Skills

To add a new skill to PolyEdge:

1. Create a directory: `.agents/skills/<skill-name>/`
2. Create a `SKILL.md` file inside it
3. Add the YAML frontmatter with `name` and `description`
4. Write clear instructions in markdown
5. Include guardrails and anti-patterns for edge cases
6. The skill will be automatically available to Claude Code

### Best Practices for Skill Authors

1. **Be specific in descriptions** -- Include trigger phrases so Claude knows when to invoke the skill
2. **Provide guardrails** -- Tell Claude what NOT to do as well as what to do
3. **Include examples** -- Show expected inputs and outputs
4. **Reference conventions** -- Point to existing patterns in the codebase
5. **Handle edge cases** -- What should happen when things go wrong?
6. **Keep it focused** -- One skill should do one thing well

---

## Quick Reference Table

| Skill | Command | Best For |
|-------|---------|----------|
| feature-dev | `/feature-dev` | Building new features |
| code-review | `/code-review` | Reviewing code for bugs |
| simplify | `/simplify` | Cleaning up code |
| verify | `/verify` | Testing changes work |
| run | `/run` | Starting the app |
| commit | `/commit` | Creating commits |
| commit-push-pr | `/commit-push-pr` | Full commit-to-PR workflow |
| deploy-to-vercel | `/deploy-to-vercel` | Deploying to Vercel |
| deep-research | `/deep-research` | Researching topics |
| accessibility | `/accessibility` | Auditing a11y |
| update-config | `/update-config` | Configuring Claude Code |
| init | `/init` | Bootstrapping Claude Code |

---

*Generated for the PolyEdge repository. Skills are located in `.agents/skills/`.*
