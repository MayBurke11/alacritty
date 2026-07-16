# IPC-модуль

## Было

IPC-типы (`IpcRequest`, `IpcResponse`, `SocketReply`) и форматтер (`format_tree_ascii`) жили в `polling/ipc.rs` вместе с логикой слушателя сокетов. Файл раздулся до 533 строк, смешивая транспорт и типы данных.

## Стало

Типы и форматтер вынесены в `alacritty/src/ipc_types.rs` (70 строк). `polling/ipc.rs` ре-экспортит их для обратной совместимости:

```
polling/ipc.rs (466 строк)  → слушатель + отправка
ipc_types.rs  (70 строк)    → IpcRequest, IpcResponse, SocketReply, format_tree_ascii
```

## Результат

- `polling/ipc.rs`: -67 строк, чище
- Типы IPC доступны через `crate::ipc_types::*`
- Обратная совместимость через `pub use` ре-экспорт
- 104 теста, сборка без ошибок
