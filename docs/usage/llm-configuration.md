# LLM Model Configuration

By default, each LLM backend uses its own default model. You can override the model per provider using the CLI.

## Setting a model override

```bash
sessfind llm-model-set claude sonnet
sessfind llm-model-set opencode anthropic/claude-sonnet-4-6
```

Model names depend on the provider — each tool uses its own naming convention.

## Removing an override

```bash
sessfind llm-model-unset claude
```

This reverts the provider back to its built-in default model.

## Checking current configuration

```bash
sessfind stats
```

The `stats` command shows active LLM backends and any model overrides.

## Config file location

Configuration is stored at:

=== "macOS / Linux"

    ```
    ~/.config/sessfind/config.json
    ```

=== "Windows"

    ```
    %APPDATA%\sessfind\config.json
    ```

## TUI badge display

The search mode badge in the TUI reflects the active model:

- `LLM (claude)` — using the tool's default model
- `LLM (claude:sonnet)` — using a custom model override

!!! tip
    You can set different models for different providers simultaneously. Only the backend you are currently using in the TUI is affected.
