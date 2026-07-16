# MCP Server

`alacritty mcp` — Model Context Protocol сервер для LLM-интеграции. Без внешних зависимостей, только `serde_json`.

## Использование

```bash
alacritty mcp
```

Сервер читает JSON-RPC из stdin, отвечает в stdout. Работает как subprocess.

## Конфигурация Claude Desktop / Cursor

```json
{
  "mcpServers": {
    "alacritty": {
      "command": "alacritty",
      "args": ["mcp"]
    }
  }
}
```

## Доступные инструменты (15)

| Инструмент | Параметры | Возвращает |
|---|---|---|
| `tab.create` | `command: string[]`, `cwd?`, `no_switch?` | `ok` |
| `tab.list` | — | `data: [...]` |
| `tab.close` | `index: integer` | `ok` |
| `tab.select` | `index: integer` | `ok` |
| `tab.next` | — | `ok` |
| `tab.previous` | — | `ok` |
| `pane.split` | `direction: "right"\|"down"` | `ok` |
| `pane.close` | — | `ok` |
| `pane.focus` | `direction: "left"\|"right"\|"up"\|"down"` | `ok` |
| `pane.zoom` | — | `ok` |
| `session.save` | — | `ok` |
| `session.list` | — | `data: [...]` |
| `config.get` | — | `data: {...}` |
| `tree` | — | `data: {...}` |
| `bell` | — | `ok` |

## Тестирование вручную

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | alacritty mcp
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tab.list","arguments":{}}}' | alacritty mcp
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tab.create","arguments":{"command":["htop"]}}}' | alacritty mcp
echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"pane.split","arguments":{"direction":"right"}}}' | alacritty mcp
```
