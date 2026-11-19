# ISA skills

This Rust module implements local, text-only reusable guidance for the response harness. Skill records live in the workspace's `.isa/skills/` directory, not this source folder. They contain plaintext and are not encrypted. Make sure `.isa/` is ignored in each workspace. IDs contain lowercase letters, numbers and internal hyphens; `auto` is reserved. Each record is limited to 8 KiB. New and edited skills start disabled; editing replaces the whole content and disables the skill until explicitly enabled again.

```sh
isa-cli skill propose rust-testing "Create a checklist for testing Rust changes" /path/to/workspace
isa-cli skill list /path/to/workspace
isa-cli skill show rust-testing /path/to/workspace
isa-cli skill enable rust-testing /path/to/workspace
ISA_SKILLS=rust-testing isa-cli ask demo "Review this test strategy" /path/to/workspace
ISA_SKILLS=auto isa-cli ask demo "Rust testing strategy" /path/to/workspace
isa-cli skill disable rust-testing /path/to/workspace
isa-cli skill create my-skill ./skill.txt /path/to/workspace
isa-cli skill edit my-skill ./revised.txt /path/to/workspace
isa-cli skill delete my-skill /path/to/workspace
```

`propose` asks the configured local Ollama model for a draft appropriate to the task and displays it for explicit interactive approval. The model cannot save or enable its proposal without that approval. `create` and `edit` are explicit user commands using a local text file. `show`, `list`, `enable`, `disable`, and `delete` act on the selected workspace. Deletion is immediate and not undoable. Keep your own backups if needed.

Skills are used **only when explicitly requested** through `ISA_SKILLS`: a comma-separated list of up to three enabled IDs, or `auto` to select up to three enabled skills through the task planner, with deterministic ID-word matching as a fallback when planning is unavailable. No selected skill means no skill is sent. A disabled, missing, duplicated or invalid explicitly selected ID fails the request. Selection is a process environment setting, not a persistent per-chat preference. Set `ISA_HARNESS=0` to bypass the harness and skills.

The harness sends up to 1,500 characters per selected skill as JSON-encoded, untrusted reference data to the local Ollama model. Skill text may influence the model's response but is not a system instruction, a tool, a script, or permission to execute commands, access external services, or edit files. Do not put secrets or adversarial text in skills. Prompt injection remains possible despite these boundaries. Skills are not auto-created or modified during a chat: generated drafts require interactive review, and no agent tool execution exists yet. The browser server uses its own current working directory and process environment, not the selected browser folder. The Node preview has no model harness.

## Detecting needs

The response harness now runs a local model planning call by default. It analyzes the latest task and the skill catalog, identifies relevant enabled skills, proposes missing skill outlines (purpose, applicability, and steps), and recommends tools. The planner sees enabled skill IDs and their first nonempty line (up to 160 characters); disabled skills are listed by ID only. Treat this catalog and the task as untrusted and do not store secrets in skill descriptions. This call adds latency and sends that data to the local Ollama service. Disable it with `ISA_HARNESS_PLANNER=0`; `ISA_HARNESS=0` bypasses the whole harness.

To inspect a detailed plan without starting a chat:

```sh
isa-cli skill plan "Review a Rust change and its tests" /path/to/workspace
```

The JSON output recommends skills and tools but does **not** execute tools or assert that they are available. Use an outline to run `skill propose` with the desired ID and task details, then review and approve the resulting disabled draft. Planning alone never creates or modifies a skill. Automatic creation is not implemented; approval is required.

With `ISA_SKILLS=auto`, a valid model plan selects up to three enabled catalog skills; unknown, disabled, and duplicate IDs are rejected. If planning is disabled or fails, automatic selection falls back to the earlier deterministic ID-word matching. An explicitly named `ISA_SKILLS` list is not overridden by planning. Planning failure is reported in server/CLI logs and does not stop the response; `skill plan` itself instead returns an error. Model-generated recommendations are fallible and not a safety check. The web UI does not yet display the plan or provide skill management.

## Imported web-design-engineer

`web-design-engineer/` contains the original `SKILL.md`, referenced materials,
metadata and English documentation from [ConardLi/garden-skills](https://github.com/ConardLi/garden-skills/tree/9578a34edf3c359da23d8a632b72790834961d11/skills/web-design-engineer),
revision `9578a34edf3c359da23d8a632b72790834961d11`, under its included MIT
license. The original skill is 35,406 bytes and references additional files;
ISA's current local skill loader accepts at most 8 KiB and does not load
`SKILL.md` directories or follow references. Copying this folder alone does
not register or enable it. The upstream Chinese README is omitted; the skill
and its referenced materials are preserved.

For the current harness, an independently written compact adaptation is
provided as `web-design-engineer/ISA-COMPAT.txt`. From the ISA repository root,
register it for a specific workspace and review it before enabling:

```sh
isa-cli skill create web-design-engineer Skills/web-design-engineer/ISA-COMPAT.txt /path/to/workspace
isa-cli skill show web-design-engineer /path/to/workspace
isa-cli skill enable web-design-engineer /path/to/workspace
ISA_SKILLS=web-design-engineer isa-cli ask demo "Review this web interface" /path/to/workspace
```

The adaptation is not the full upstream skill; only the first 1,500 characters
of each enabled skill are sent to the model. The local harness does not
execute instructions, load the original references, or automatically activate
this skill. The web server requires the skill to be registered in its own
working directory and `ISA_SKILLS` set in the server environment. Review
upstream instructions and use them only when compatible with the task, user
requirements, safety boundaries, and available tools.
