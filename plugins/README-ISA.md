# ISA plugin catalog

The nine plugin directories are imported unchanged from
[wesleyegberto/software-engineering-skills](https://github.com/wesleyegberto/software-engineering-skills/tree/94fb08a3f136aa8010f6e1c6585f68836ce7cf1d/plugins)
(revision `94fb08a3f136aa8010f6e1c6585f68836ce7cf1d`). The upstream MIT
license is in `software-engineering/LICENSE`. Imported skills, agents, commands, scripts, and
configuration files are third-party material and are not automatically trusted
or executed by ISA.

`SKILL-LIST.md` is an overview of imported skill names and descriptions. To
refresh it after changing the plugin directories, run from the ISA repository:

```sh
cargo run --quiet --bin isa-plugin-index -- . > plugins/SKILL-LIST.md
```

The Rust web server scans `plugins/software-engineering/<folder>/skills/<skill>/SKILL.md`
and standalone `plugins/<plugin>/skills/<skill>/SKILL.md` in its
own current working directory for each catalog request and model request; it
ignores symlinked directories and oversized or symlinked files. The Markdown
snapshot is not the runtime registry. Start the server in the repository
root, or put the plugin directory in its working directory. The browser's
folder selector does not change that server directory.

In Chat settings, Automatic is on-demand: for each message, ISA compares words
from the user task with names and descriptions of skills in enabled plugins.
It supplies excerpts of at most three matching skills, up to 1,500 characters
each, as explicitly untrusted reference data. This simple lexical selection can
miss relevant skills or choose irrelevant ones. ISA does not follow skill
references, run imported agents or commands, grant permissions, or install
plugin dependencies. The imported software-engineering collection is shown as one expandable plugin
with individual folder switches and its own enable switch. Folder choices are
saved per conversation when the collection is disabled. Global Off blocks
these switches and uses no plugin excerpts. Plugin instructions can still influence a model, so
review third-party material before enabling it and do not rely on it as a
security boundary.

Only the configured local Ollama model is currently supported; Automatic uses
it when `ISA_WEB_MODEL` is set and otherwise returns the decoy's no-provider
error. The Node preview lists the bundled catalog but does not use plugins or
call Ollama. Preferences are saved locally per conversation; they apply to
future messages and do not alter requests already running.
