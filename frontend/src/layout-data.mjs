// Declarative interface tree. Strings are text nodes; elements contain tag, attributes and children.
export const layout = [
  ["div", {"class":"app"}, [
    ["aside", {"class":"sidebar","id":"sidebar"}, [
      ["div", {"class":"window-bar"}, [
        ["span", {"class":"window-dot"}, []],
        ["span", {"class":"window-dot"}, []],
        ["span", {"class":"window-dot"}, []],
        ["span", {"class":"chevrons"}, [
          "‹ ›"
        ]],
        ["button", {"class":"sidebar-toggle sidebar-collapse","id":"collapseSidebar","aria-label":"Collapse sidebar","aria-controls":"sidebar","aria-expanded":"true","title":"Collapse sidebar"}, [
          ["svg", {"aria-hidden":"true","width":"16","height":"16","viewBox":"0 0 20 20","fill":"none","stroke":"currentColor","stroke-width":"1.4","stroke-linecap":"round","stroke-linejoin":"round"}, [
            ["rect", {"x":"2.5","y":"3","width":"15","height":"14","rx":"1.5"}, []],
            ["path", {"d":"M7 3v14M4.1 8l1.3 2-1.3 2"}, []]
          ]]
        ]]
      ]],
      ["div", {"class":"workspace-switch"}, [
        "▣ ",
        ["span", {}, [
          "✧   General"
        ]]
      ]],
      ["nav", {"class":"nav","aria-label":"Main navigation"}, [
        ["button", {"id":"newTask"}, [
          ["span", {"class":"ico"}, [
            "◉"
          ]],
          " New task"
        ]],
        ["button", {"id":"projectsNav"}, [
          ["span", {"class":"ico"}, [
            "▦"
          ]],
          " Projects"
        ]],
        ["button", {"data-name":"Agents"}, [
          ["span", {"class":"ico"}, [
            "✧"
          ]],
          " Agents"
        ]],
        ["button", {"data-hub":"Chats"}, [
          ["span", {"class":"ico"}, [
            "☷"
          ]],
          " View chats"
        ]]
      ]],
      ["div", {"class":"label"}, [
        "Projects"
      ]],
      ["div", {"id":"sidebarProjects","class":"sidebar-projects","aria-label":"Projects"}, []],
      ["div", {"class":"label"}, [
        "Recent conversations"
      ]],
      ["div", {"id":"recentChats","tabindex":"0","role":"region","aria-label":"Recent conversations"}, []],
      ["div", {"class":"bottom-side"}, [
        ["div", {"class":"secondary"}, [
          ["button", {"data-name":"Knowledge center"}, [
            ["span", {"class":"ico"}, [
              "▤"
            ]],
            " Knowledge center"
          ]],
          ["button", {"data-name":"Automations"}, [
            ["span", {"class":"ico"}, [
              "◷"
            ]],
            " Automations"
          ]],
          ["button", {"data-name":"Extensions"}, [
            ["span", {"class":"ico"}, [
              "⌘"
            ]],
            " Extensions"
          ]],
          ["button", {"data-name":"Settings"}, [
            ["span", {"class":"ico"}, [
              "⚙"
            ]],
            " Settings"
          ]]
        ]],
        ["div", {"class":"brand"}, [
          ["span", {"class":"mark"}, [
            ["img", {"src":"/assets/isa-logo-transparent.png","alt":"ISA logo"}, []]
          ]],
          " ISA ",
          ["span", {"class":"right"}, [
            ["button", {"class":"settings-gear","data-name":"Settings","aria-label":"Open settings","title":"Settings"}, [
              "⚙"
            ]]
          ]]
        ]]
      ]]
    ]],
    "\n",
    ["main", {"class":"main"}, [
      ["header", {"class":"topbar"}, [
        ["button", {"class":"icon-button mobile-menu","id":"menu","aria-label":"Open menu","aria-controls":"sidebar","aria-expanded":"false"}, [
          "☰"
        ]],
        ["button", {"class":"sidebar-toggle desktop-expand","id":"expandSidebar","aria-label":"Expand sidebar","aria-controls":"sidebar","aria-expanded":"false","title":"Expand sidebar"}, [
          ["svg", {"aria-hidden":"true","width":"16","height":"16","viewBox":"0 0 20 20","fill":"none","stroke":"currentColor","stroke-width":"1.4","stroke-linecap":"round","stroke-linejoin":"round"}, [
            ["rect", {"x":"2.5","y":"3","width":"15","height":"14","rx":"1.5"}, []],
            ["path", {"d":"M7 3v14M4.1 8l1.3 2-1.3 2"}, []]
          ]]
        ]],
        ["span", {}, [
          "✧"
        ]],
        ["strong", {"id":"title"}, [
          "New chat"
        ]],
        ["button", {"type":"button","title":"Chat settings","class":"icon-button","id":"more","aria-label":"Chat settings","aria-haspopup":"dialog"}, [
          "···"
        ]],
        ["div", {"class":"top-actions"}, [
          ["button", {"type":"button","id":"taskOverviewToggle","class":"icon-button","title":"Task overview","aria-label":"Task overview","aria-controls":"inspector","aria-expanded":"false"}, [
            "☷"
          ]],
          ["button", {"type":"button","id":"changesToggle","class":"icon-button","title":"Changes","aria-label":"Changes","aria-controls":"inspector","aria-expanded":"false"}, [
            "▣"
          ]]
        ]]
      ]],
      ["div", {"class":"workspace-body"}, [
        ["div", {"class":"workspace-main"}, [
          ["div", {"class":"content","id":"content"}, [
            ["section", {"class":"hub","id":"hub","hidden":""}, []],
            ["section", {"class":"projects","id":"projects","hidden":""}, [
              ["div", {"class":"projects-top"}, [
                ["div", {}, [
                  ["h1", {}, [
                    "Projects"
                  ]],
                  ["p", {"class":"projects-subtitle"}, [
                    "Organize your work and conversations in one place."
                  ]]
                ]],
                ["button", {"class":"primary","id":"createProject"}, [
                  "＋   New project"
                ]]
              ]],
              ["div", {"id":"projectList"}, [
                ["div", {"class":"projects-toolbar"}, [
                  ["strong", {}, [
                    "All projects"
                  ]],
                  ["span", {"class":"count","id":"projectCount"}, [
                    "0"
                  ]],
                  ["input", {"class":"search-projects","id":"projectSearch","type":"search","placeholder":"Search projects...","aria-label":"Search projects"}, []]
                ]],
                ["div", {"class":"project-grid","id":"projectGrid"}, []],
                ["div", {"class":"empty-projects","id":"projectEmpty"}, [
                  ["div", {"class":"empty-icon"}, [
                    "▦"
                  ]],
                  ["h2", {}, [
                    "No projects yet"
                  ]],
                  ["p", {}, [
                    "Create a project to start organizing your work.",
                    ["br", {}, []],
                    "Projects in this preview are saved in this browser only."
                  ]],
                  ["button", {"class":"secondary-action","id":"emptyCreate"}, [
                    "＋   Create project"
                  ]]
                ]]
              ]],
              ["div", {"id":"projectDetail","hidden":""}, []]
            ]],
            ["div", {"class":"welcome","id":"welcome"}, [
              ["div", {"class":"welcome-mark"}, [
                ["img", {"src":"/assets/isa-logo-transparent.png","alt":"ISA logo"}, []]
              ]],
              ["h1", {}, [
                "Beyond coding"
              ]],
              ["p", {}, [
                "What can ISA help you with?"
              ]],
              ["div", {"class":"activity-card"}, [
                ["div", {"class":"activity-header"}, [
                  "Token activity ",
                  ["span", {}, [
                    "Daily · Reported usage"
                  ]]
                ]],
                ["div", {"class":"heatmap","id":"heatmap","aria-label":"Conversation activity by day"}, []],
                ["div", {"class":"months","id":"months"}, []],
                ["div", {"class":"activity-legend heatmap","aria-label":"Daily reported token usage intensity"}, [
                  ["span", {}, [
                    "No reported usage"
                  ]],
                  ["i", {"title":"No reported usage"}, []],
                  ["i", {"class":"level-1","title":"Up to 100K tokens"}, []],
                  ["i", {"class":"level-2","title":"Over 100K–1M tokens"}, []],
                  ["i", {"class":"level-3","title":"Over 1M–10M tokens"}, []],
                  ["i", {"class":"level-4","title":"Over 10M–200M tokens"}, []],
                  ["i", {"class":"level-5","title":"Over 200M–1B tokens"}, []],
                  ["i", {"class":"level-6","title":"Over 1B tokens"}, []],
                  ["span", {}, [
                    "Tokens / day"
                  ]]
                ]]
              ]]
            ]],
            ["div", {"class":"thread","id":"thread","hidden":""}, [
              ["div", {"class":"status"}, [
                ["span", {"class":"status-icon"}, [
                  "→"
                ]],
                " Ready to create"
              ]],
              ["p", {}, [
                "Welcome to ",
                ["strong", {}, [
                  "ISA"
                ]],
                ", your space to turn ideas into something real. Tell me what you would like to build, and we can work through it together."
              ]],
              ["div", {"class":"status"}, [
                ["span", {"class":"status-icon"}, [
                  "✧"
                ]],
                " Your workspace is ready"
              ]],
              ["p", {}, [
                "Start a conversation, explore your project, or ask ISA to help you plan your next task. Your work and context will live here, all in one place."
              ]],
              ["button", {"class":"file","id":"project"}, [
                ["span", {"class":"file-icon"}, [
                  "▤"
                ]],
                ["span", {}, [
                  ["b", {}, [
                    "My project"
                  ]],
                  ["small", {}, [
                    "Workspace"
                  ]]
                ]],
                ["span", {"class":"more"}, [
                  "···"
                ]]
              ]],
              ["div", {"class":"reactions"}, [
                ["button", {"data-info":"Copy is available in the final app.","aria-label":"Copy"}, [
                  "▣"
                ]],
                ["button", {"data-info":"Feedback is available in the final app.","aria-label":"Good response"}, [
                  "♧"
                ]],
                ["button", {"data-info":"Feedback is available in the final app.","aria-label":"Bad response"}, [
                  "♧"
                ]],
                ["button", {"data-info":"Sharing is available in the final app.","aria-label":"Share"}, [
                  "↗"
                ]],
                ["button", {"data-info":"More actions are available in the final app.","aria-label":"More"}, [
                  "···"
                ]]
              ]]
            ]]
          ]],
          ["div", {"class":"composer-wrap"}, [
            ["div", {"class":"composer"}, [
              ["div", {"class":"attachments","id":"attachments","aria-label":"Attached files"}, []],
              ["div", {"class":"composer-input"}, [
                ["div", {"id":"commandList","class":"command-list","role":"listbox","aria-label":"Command suggestions","hidden":""}, []],
                ["div", {"class":"completion-ghost","id":"completionGhost","aria-hidden":"true","hidden":""}, [
                  ["span", {"class":"typed","id":"completionTyped"}, []],
                  ["span", {"id":"completionTail"}, []]
                ]],
                ["textarea", {"id":"prompt","aria-label":"Message ISA","aria-describedby":"completionHint","aria-autocomplete":"list","aria-controls":"commandList","aria-expanded":"false","placeholder":"Continue this session..."}, []]
              ]],
              ["div", {"class":"completion-hint","id":"completionHint","role":"status","hidden":""}, []],
              ["input", {"id":"fileInput","type":"file","multiple":"","hidden":""}, []],
              ["div", {"class":"toolbar"}, [
                ["button", {"class":"tool add","id":"addContext","title":"Attach files","aria-label":"Attach files"}, [
                  "＋"
                ]],
                ["div", {"class":"connections-popover","id":"connectionsPopover","hidden":""}, [
                  ["div", {"class":"connections-title"}, [
                    "Connections"
                  ]],
                  ["div", {"class":"connections-list","id":"connectionsList"}, []],
                  ["button", {"type":"button","class":"view-providers","id":"viewProviders"}, [
                    "View all providers ",
                    ["span", {}, [
                      "›"
                    ]]
                  ]]
                ]],
                ["button", {"class":"tool","id":"attachFile","title":"Connections","aria-label":"Connections","aria-expanded":"false","aria-controls":"connectionsPopover"}, [
                  ["svg", {"aria-hidden":"true","width":"15","height":"15","viewBox":"0 0 20 20","fill":"none","stroke":"currentColor","stroke-width":"1.3","stroke-linecap":"round","stroke-linejoin":"round"}, [
                    ["rect", {"x":"3","y":"3","width":"5","height":"5","rx":"0.8"}, []],
                    ["rect", {"x":"11","y":"3","width":"5","height":"5","rx":"0.8"}, []],
                    ["rect", {"x":"3","y":"11","width":"5","height":"5","rx":"0.8"}, []],
                    ["path", {"d":"M11 13.5h5M13.5 11v5"}, []]
                  ]]
                ]],
                ["div", {"class":"approval-control"}, [
                  ["button", {"type":"button","class":"tool approval-trigger","id":"approvalModeButton","aria-haspopup":"menu","aria-expanded":"false","aria-controls":"approvalModeMenu"}, [
                    "♢   ",
                    ["span", {"id":"approvalModeLabel"}, [
                      "Ask every time"
                    ]],
                    "  ⌄"
                  ]],
                  ["div", {"id":"approvalModeMenu","class":"approval-menu","role":"menu","aria-label":"Chat supervision preference","hidden":""}, [
                    ["div", {"class":"approval-menu-heading"}, [
                      "Chat supervision"
                    ]],
                    ["button", {"type":"button","role":"menuitemradio","data-approval-mode":"always"}, [
                      ["strong", {}, [
                        "Ask every time"
                      ]],
                      ["small", {}, [
                        "Review each command, tool or skill action."
                      ]]
                    ]],
                    ["button", {"type":"button","role":"menuitemradio","data-approval-mode":"risk"}, [
                      ["strong", {}, [
                        "Ask occasionally"
                      ]],
                      ["small", {}, [
                        "Review risky or unknown actions."
                      ]]
                    ]],
                    ["button", {"type":"button","role":"menuitemradio","data-approval-mode":"free"}, [
                      ["strong", {}, [
                        "Never ask"
                      ]],
                      ["small", {}, [
                        "Allow actions without a supervision prompt."
                      ]]
                    ]],
                    ["p", {}, [
                      "Supervised manual commands and workspace info require the opt-in local server. Tool loop can be enabled for message sends; Chat MCP and Pool MCP are separate opt-in interactions. CLI approvals remain separate."
                    ]]
                  ]]
                ]],
                ["button", {"type":"button","class":"tool","id":"runCommandButton","aria-haspopup":"dialog","title":"Run a command using the local supervised server"}, [
                  "Run command"
                ]],
                ["button", {"type":"button","class":"tool","id":"workspaceInfoButton","hidden":"","aria-haspopup":"dialog","title":"Inspect the configured local workspace"}, [
                  "Workspace info"
                ]],
                ["button", {"type":"button","class":"tool","id":"mcpToolsButton","hidden":"","aria-haspopup":"dialog","title":"Use a configured local MCP server"}, [
                  "MCP tools"
                ]],
                ["button", {"type":"button","class":"tool","id":"toolLoopButton","hidden":"","aria-haspopup":"dialog","title":"Run a supervised model tool loop"}, [
                  "Tool loop"
                ]],
                ["button", {"type":"button","class":"tool","id":"chatMcpButton","hidden":"","aria-haspopup":"dialog","title":"Use MCP with the selected custom chat provider"}, [
                  "Chat MCP"
                ]],
                ["button", {"type":"button","class":"tool","id":"poolMcpButton","hidden":"","aria-haspopup":"dialog","title":"Use MCP with one selected pool agent"}, [
                  "Pool MCP"
                ]],
                ["button", {"type":"button","class":"tool","id":"actionHistoryButton","hidden":"","aria-haspopup":"dialog","title":"Show recent local action events"}, [
                  "Action history"
                ]],
                ["button", {"type":"button","class":"tool","id":"transactionButton","hidden":"","aria-haspopup":"dialog","title":"Review a multi-file edit transaction"}, [
                  "Edit transaction"
                ]],
                ["button", {"type":"button","class":"tool","id":"undoTransactionButton","hidden":"","aria-haspopup":"dialog","title":"Undo the recent multi-file edit"}, [
                  "Undo transaction"
                ]],
                ["button", {"type":"button","class":"tool","id":"undoEditButton","hidden":"","aria-haspopup":"dialog","title":"Undo a recent supervised edit"}, [
                  "Undo edit"
                ]],
                ["button", {"type":"button","class":"tool","id":"proposeEditButton","hidden":"","aria-haspopup":"dialog","title":"Generate a read-only edit proposal"}, [
                  "Propose edit"
                ]],
                ["div", {"class":"token-control"}, [
                  ["button", {"type":"button","class":"tool","id":"tokensButton","aria-expanded":"false","aria-controls":"tokensMenu"}, [
                    "Tokens"
                  ]],
                  ["div", {"id":"tokensMenu","class":"token-menu","hidden":""}, []]
                ]],
                ["span", {"class":"spacer"}, []],
                ["div", {"class":"routing-control"}, [
                  ["button", {"type":"button","class":"tool routing-button","id":"routingButton","aria-haspopup":"menu","aria-expanded":"false","aria-controls":"routingMenu"}, [
                    "✧ ",
                    ["span", {"id":"routingLabel"}, [
                      "Auto"
                    ]],
                    " ⌄"
                  ]],
                  ["div", {"class":"routing-menu","id":"routingMenu","role":"menu","hidden":""}, []]
                ]],
                ["button", {"type":"button","class":"tool voice-button","id":"voiceInput","title":"Dictate message","aria-label":"Dictate message"}, [
                  ["svg", {"width":"20","height":"20","viewBox":"0 0 24 24","fill":"none","stroke":"currentColor","stroke-width":"1.8","stroke-linecap":"round","stroke-linejoin":"round","aria-hidden":"true"}, [
                    ["rect", {"x":"9","y":"3","width":"6","height":"12","rx":"3"}, []],
                    ["path", {"d":"M5 11a7 7 0 0 0 14 0M12 18v3M9 21h6"}, []]
                  ]]
                ]],
                ["button", {"class":"send","id":"send","aria-label":"Send message"}, [
                  "↑"
                ]]
              ]]
            ]],
            ["div", {"class":"under"}, [
              ["button", {"id":"folderPicker","title":"Choose a folder for this session"}, [
                "▱   Add folder"
              ]],
              ["span", {}, [
                ["i", {"class":"progress"}, []],
                " Local"
              ]]
            ]]
          ]]
        ]],
        ["aside", {"id":"inspector","class":"inspector","aria-label":"Workspace details","hidden":""}, [
          ["div", {"id":"inspectorResize","class":"inspector-resize","role":"separator","aria-label":"Resize side panel","aria-orientation":"vertical","aria-controls":"inspector","aria-valuemin":"0","aria-valuemax":"1200","aria-valuenow":"370","tabindex":"0"}, []],
          ["div", {"class":"inspector-header"}, [
            ["h2", {"id":"inspectorTitle"}, [
              "Task overview"
            ]],
            ["div", {"class":"inspector-actions"}, [
              ["button", {"type":"button","id":"refreshChanges","class":"icon-button","title":"Refresh changes","aria-label":"Refresh changes","hidden":""}, [
                "↻"
              ]],
              ["button", {"type":"button","id":"closeInspector","class":"icon-button","title":"Close panel","aria-label":"Close panel"}, [
                "×"
              ]]
            ]]
          ]],
          ["div", {"id":"inspectorBody","class":"inspector-body"}, []]
        ]]
      ]]
    ]]
  ]],
  ["dialog", {"id":"chatDetailsDialog","class":"chat-details-dialog","aria-labelledby":"chatDetailsTitle"}, [
    ["form", {"id":"chatDetailsForm"}, [
      ["div", {"class":"chat-details-header"}, [
        ["div", {}, [
          ["h2", {"id":"chatDetailsTitle"}, [
            "Conversation details"
          ]],
          ["p", {}, [
            "Personalize this conversation"
          ]]
        ]],
        ["button", {"type":"button","class":"icon-button","id":"closeChatDetails","aria-label":"Close conversation details"}, [
          "×"
        ]]
      ]],
      ["div", {"class":"chat-details-body"}, [
        ["label", {"for":"chatDetailName"}, [
          "Name"
        ]],
        ["input", {"id":"chatDetailName","maxlength":"120","required":"","autocomplete":"off"}, []],
        ["label", {"for":"chatDetailDescription"}, [
          "Description ",
          ["span", {}, [
            "(optional)"
          ]]
        ]],
        ["textarea", {"id":"chatDetailDescription","maxlength":"1000","placeholder":"Add your own description..."}, []],
        ["div", {"class":"chat-avatar-title"}, [
          "Avatar"
        ]],
        ["div", {"id":"chatAvatarChoices","class":"chat-avatar-choices","role":"group","aria-label":"Choose a conversation avatar"}, []],
        ["dl", {"class":"chat-details-facts"}, [
          ["div", {}, [
            ["dt", {"id":"chatCreatedLabel"}, [
              "Created"
            ]],
            ["dd", {"id":"chatCreatedValue"}, []]
          ]],
          ["div", {}, [
            ["dt", {}, [
              "Last active"
            ]],
            ["dd", {"id":"chatLastActiveValue"}, []]
          ]],
          ["div", {}, [
            ["dt", {}, [
              "Messages"
            ]],
            ["dd", {"id":"chatMessageCount"}, []]
          ]]
        ]]
      ]],
      ["div", {"class":"chat-settings-actions"}, [
        ["button", {"type":"button","class":"secondary-action","id":"cancelChatDetails"}, [
          "Cancel"
        ]],
        ["button", {"type":"submit","class":"primary"}, [
          "Save changes"
        ]]
      ]]
    ]]
  ]],
  ["dialog", {"id":"chatSettingsDialog","class":"chat-settings-dialog","aria-labelledby":"chatSettingsTitle"}, [
    ["form", {"id":"chatSettingsForm"}, [
      ["div", {"class":"chat-settings-head"}, [
        ["div", {}, [
          ["h2", {"id":"chatSettingsTitle"}, [
            "Chat settings"
          ]],
          ["p", {}, [
            "Preferences for this conversation"
          ]]
        ]],
        ["button", {"type":"button","class":"icon-button","id":"closeChatSettings","aria-label":"Close chat settings"}, [
          "×"
        ]]
      ]],
      ["div", {"class":"chat-settings-body"}, [
        ["div", {"class":"agent-loop-setting"}, [
          ["strong", {}, [
            "Server and workspace"
          ]],
          ["p", {"id":"serverCapabilityStatus","class":"chat-settings-note","role":"status"}, [
            "Checking server capabilities…"
          ]],
          ["p", {"class":"chat-settings-note"}, [
            "The browser folder is read-only and does not configure either server workspace. Review their paths using the local execution token."
          ]],
          ["button", {"id":"inspectWorkspaces","type":"button","class":"secondary-action"}, [
            "Review server workspaces"
          ]]
        ]],
        ["label", {"for":"chatProvider"}, [
          "Provider"
        ]],
        ["select", {"id":"chatProvider"}, [
          ["option", {"value":"auto"}, [
            "Automatic"
          ]],
          ["option", {"value":"ollama"}, [
            "Ollama · configured model"
          ]]
        ]],
        ["p", {"id":"chatProviderStatus","class":"chat-settings-note"}, [
          "Checking available providers…"
        ]],
        ["div", {"class":"agent-loop-setting"}, [
          ["label", {}, [
            ["input", {"type":"checkbox","id":"chatAgentLoop"}, []],
            " Read-only agent loop"
          ]],
          ["p", {"id":"agentLoopStatus","class":"chat-settings-note"}, [
            "Ollama only. With explicit server setup, ISA may search the configured server workspace and send excerpts to Ollama. No shell, edits, or sandbox. Browser folder selection is not used."
          ]]
        ]],
        ["div", {"class":"agent-loop-setting"}, [
          ["label", {}, [
            ["input", {"type":"checkbox","id":"chatToolLoop"}, []],
            " Supervised tool loop on send"
          ]],
          ["p", {"class":"chat-settings-note"}, [
            "Custom providers and direct local server only. Each message opens the bounded tool loop; tool actions follow chat supervision and each provider request requires separate consent. Only the new message is used; attachments and history are not sent. In never-ask mode tools may run without another approval. Not supported with the read-only Ollama loop."
          ]]
        ]],
        ["label", {"for":"poolSymbolQuery"}, [
          "Pool symbol context (optional)"
        ]],
        ["input", {"id":"poolSymbolQuery","type":"text","maxlength":"100","autocomplete":"off","placeholder":"Symbol name, e.g. parse_request"}, []],
        ["label", {"for":"poolSymbolMode"}, [
          "Pool symbol lookup type"
        ]],
        ["select", {"id":"poolSymbolMode"}, [
          ["option", {"value":"declarations"}, [
            "Declarations (partial name)"
          ]],
          ["option", {"value":"callers"}, [
            "Call sites (exact name, unresolved)"
          ]]
        ]],
        ["p", {"id":"poolSymbolStatus","class":"chat-settings-note"}, [
          "For Agent Pools only. Each send can look up declarations or call sites in the configured server workspace and share matching names and paths with pool providers. Configure ISA_WEB_AGENT_WORKSPACE on the Rust server. This is not the browser folder."
        ]],
        ["div", {"class":"chat-plugin-toggle-row"}, [
          ["div", {}, [
            ["label", {"for":"chatPluginsMode"}, [
              "Plugins"
            ]],
            ["span", {"id":"chatPluginsModeLabel"}, [
              "Automatic · on demand"
            ]]
          ]],
          ["label", {"class":"chat-plugin-switch"}, [
            ["input", {"type":"checkbox","id":"chatPluginsMode","role":"switch","aria-label":"Enable plugins","aria-describedby":"chatPluginsModeLabel"}, []],
            ["span", {"aria-hidden":"true","class":"chat-plugin-switch-track"}, []]
          ]]
        ]],
        ["p", {"class":"chat-settings-note"}, [
          "Automatic uses matching skill excerpts as untrusted reference data. It does not run plugin code."
        ]],
        ["div", {"id":"chatPluginsStatus","class":"chat-settings-note","role":"status"}, [
          "Loading plugins…"
        ]],
        ["div", {"id":"chatPluginList","class":"chat-plugin-list"}, []]
      ]],
      ["div", {"class":"chat-settings-actions"}, [
        ["button", {"type":"button","class":"secondary-action","id":"cancelChatSettings"}, [
          "Cancel"
        ]],
        ["button", {"type":"submit","class":"primary"}, [
          "Save settings"
        ]]
      ]]
    ]]
  ]],
  ["dialog", {"id":"attachmentDialog","class":"attachment-dialog","aria-labelledby":"attachmentTitle"}, [
    ["div", {"class":"attachment-dialog-head"}, [
      ["div", {}, [
        ["h2", {"id":"attachmentTitle"}, [
          "Attachment"
        ]],
        ["p", {"id":"attachmentDetail"}, []]
      ]],
      ["button", {"type":"button","class":"attachment-close","id":"closeAttachment","aria-label":"Close attachment"}, [
        "×"
      ]]
    ]],
    ["div", {"id":"attachmentPreview","class":"attachment-preview"}, []],
    ["div", {"class":"attachment-editor","id":"attachmentEditor","hidden":""}, [
      ["label", {}, [
        "Brush color ",
        ["input", {"id":"brushColor","type":"color","value":"#ff4d4d"}, []]
      ]],
      ["label", {}, [
        "Brush size ",
        ["input", {"id":"brushSize","type":"range","min":"2","max":"40","value":"8"}, []]
      ]],
      ["button", {"type":"button","class":"secondary-action","id":"undoStroke"}, [
        "Undo"
      ]],
      ["button", {"type":"button","class":"secondary-action","id":"clearStrokes"}, [
        "Clear marks"
      ]]
    ]],
    ["p", {"class":"attachment-dialog-hint","id":"attachmentHint"}, []],
    ["div", {"class":"dialog-actions"}, [
      ["button", {"type":"button","class":"secondary-action","id":"downloadAttachment"}, [
        "Download"
      ]],
      ["button", {"type":"button","class":"secondary-action","id":"cancelAttachment"}, [
        "Close"
      ]],
      ["button", {"type":"button","class":"primary","id":"saveAttachment","hidden":""}, [
        "Save marks"
      ]]
    ]]
  ]],
  ["dialog", {"id":"unrestrictedDialog","class":"approval-confirm-dialog","aria-labelledby":"unrestrictedTitle","aria-describedby":"unrestrictedDescription"}, [
    ["div", {"class":"approval-confirm-icon","aria-hidden":"true"}, [
      "♢"
    ]],
    ["h2", {"id":"unrestrictedTitle"}, [
      "Set this chat to never ask?"
    ]],
    ["p", {"id":"unrestrictedDescription"}, [
      "Manual chat commands and workspace info can run without a supervision prompt when the local executor is enabled. They may modify files, expose data or access the network. Model-driven tools and skills are not connected."
    ]],
    ["div", {"class":"approval-confirm-note"}, [
      "Applies to opt-in local manual commands and workspace info. CLI approvals remain separate."
    ]],
    ["div", {"class":"dialog-actions"}, [
      ["button", {"type":"button","class":"secondary-action","id":"cancelUnrestricted","autofocus":""}, [
        "Cancel"
      ]],
      ["button", {"type":"button","class":"primary","id":"confirmUnrestricted"}, [
        "Set never ask"
      ]]
    ]]
  ]],
  ["dialog", {"id":"helpDialog"}, [
    ["h2", {"id":"helpDialogTitle"}, [
      "Help"
    ]],
    ["div", {"id":"helpDialogBody"}, []],
    ["div", {"class":"dialog-actions"}, [
      ["button", {"type":"button","class":"secondary-action","id":"closeHelp"}, [
        "Close"
      ]]
    ]]
  ]],
  ["dialog", {"id":"providersDialog","class":"provider-dialog"}, [
    ["div", {"class":"provider-dialog-head"}, [
      ["h2", {}, [
        "Providers"
      ]],
      ["button", {"type":"button","class":"provider-close","id":"closeProviders","aria-label":"Close providers"}, [
        "×"
      ]]
    ]],
    ["p", {"class":"provider-intro","id":"providerStatus","role":"status"}, [
      "Connections are not available yet."
    ]],
    ["div", {"class":"provider-cards","id":"providerCards"}, []]
  ]],
  ["dialog", {"id":"folderDialog","class":"folder-dialog"}, [
    ["h2", {}, [
      "Workspace folder"
    ]],
    ["p", {"class":"folder-note"}, [
      "Choose a folder, then select individual text files to add to the chat. The folder is available only in this browser session; ISA cannot edit it or send it to an agent yet."
    ]],
    ["div", {"class":"folder-toolbar"}, [
      ["button", {"type":"button","class":"primary","id":"chooseFolder"}, [
        "Choose folder"
      ]],
      ["button", {"type":"button","class":"secondary-action","id":"fallbackFolder"}, [
        "Choose using file picker"
      ]],
      ["button", {"type":"button","class":"secondary-action","id":"removeFolder","hidden":""}, [
        "Remove folder"
      ]]
    ]],
    ["input", {"id":"folderInput","type":"file","webkitdirectory":"","multiple":"","hidden":""}, []],
    ["div", {"class":"folder-path","id":"folderPath"}, [
      "No folder selected"
    ]],
    ["div", {"class":"folder-list","id":"folderList"}, [
      ["div", {"class":"folder-message"}, [
        "Select a folder to browse its files."
      ]]
    ]],
    ["div", {"class":"dialog-actions"}, [
      ["button", {"type":"button","class":"secondary-action","id":"closeFolder"}, [
        "Done"
      ]]
    ]]
  ]],
  ["dialog", {"id":"projectDialog"}, [
    ["form", {"id":"projectForm"}, [
      ["h2", {}, [
        "New project"
      ]],
      ["p", {}, [
        "Give your project a name and an optional description. Projects are saved in this browser and do not connect to a folder or repository."
      ]],
      ["label", {"for":"projectName"}, [
        "Project name"
      ]],
      ["input", {"id":"projectName","maxlength":"80","required":"","placeholder":"My project","autocomplete":"off"}, []],
      ["label", {"for":"projectDescription"}, [
        "Description ",
        ["span", {"class":"optional-label"}, [
          "(optional)"
        ]]
      ]],
      ["textarea", {"id":"projectDescription","maxlength":"240","placeholder":"What are you working on?"}, []],
      ["div", {"class":"dialog-actions"}, [
        ["button", {"class":"secondary-action","id":"cancelProject","type":"button"}, [
          "Cancel"
        ]],
        ["button", {"class":"primary","type":"submit"}, [
          "Create project"
        ]]
      ]]
    ]]
  ]],
  ["div", {"class":"toast","id":"toast","role":"status"}, []]
];
