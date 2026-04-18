---
name: wiki
description: Generate github wiki for kazeterm project
---

## When to Use

DO NOT USE THIS SKILL WITH AGENTS. IT'S INTENTED TO BE USED BY HUMANS TO GENERATE WIKI CONTENT.

## Wiki Pages to generate

### Configurations

It uses `examples/configuration.md` as template for the content of this page.

It should use the source code as source of truth for the content. It should describe the configuration options for kazeterm by sections, for each section it should uses markdown `h3` to start the section, then give a breif introduction of the section, then list the configuration options in a table with four columns: name, type, default value and description. The description should provide a clear explanation of the option's purpose and usage, and if applicable, include examples or possible values.

Here is one example of configuration section

```
### Tab Behavior

It controls the behavior of tabs in kazeterm.

| Key               | Type | Default | Description                                                        |
|-------------------|------|---------|--------------------------------------------------------------------|
| vertical_tabs     | bool | false   | Render tabs in a vertical sidebar instead of horizontal bar        |
| close_on_last_tab | bool | true    | Close the app when the last tab is closed. When false, a new tab is created instead |
| tab_switcher_popup| bool | true    | Show a popup when switching tabs with Ctrl+Tab. When false, tabs switch directly |
```

## Where to save the results

The generated pages should be saved to `/wiki` folder in the repository, with the name `CONFIGURATION.md` for the configuration page. The content should be in markdown format and should be well-structured and easy to read.
