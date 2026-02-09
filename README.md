# rust-streaming-quotes

Проект с двумя приложениями в одном Cargo workspace:
- `quote-server`: генерирует котировки и стримит их клиентам.
- `quote-client`: подписывается на тикеры, принимает котировки и отправляет keep-alive ping.

Общие типы и протокол вынесены в `quote-core`.

## Структура

- `quote-core`:
  - доменные типы (`StockQuote`)
  - парсинг командного протокола (`STREAM ...`)
  - UDP wire-формат (`UdpPacketV1`, версия + бинарный payload)
  - парсинг/чтение тикеров
- `quote-server`:
  - TCP listener для команд
  - поток генерации котировок
  - поток приёма UDP ping
  - отдельная сессия на каждого клиента
- `quote-client`:
  - отправка `STREAM` по TCP
  - приём котировок по UDP
  - отдельный поток периодического ping

## Требования

- Rust stable (рекомендуется актуальная версия)
- Cargo

## Сборка и тесты

```bash
cargo build
cargo test
```

## Запуск (с debug-логами)

Открой два терминала в корне проекта.

### 1) Сервер

```bash
RUST_LOG=debug cargo run -p quote-server -- \
  --tcp-bind 127.0.0.1:5555 \
  --udp-bind 127.0.0.1:5556 \
  --tickers-file quote-server/assets/tickers.txt
```

Альтернатива: задать тикеры строкой:

```bash
RUST_LOG=debug cargo run -p quote-server -- \
  --tickers "AAPL,TSLA,GOOG,MSFT"
```

### 2) Клиент

```bash
RUST_LOG=debug cargo run -p quote-client -- \
  --server 127.0.0.1:5555 \
  --udp-port 6001 \
  --bind-ip 127.0.0.1 \
  --tickers-file quote-server/assets/tickers.txt
```

Для подписки на часть тикеров:

```bash
RUST_LOG=debug cargo run -p quote-client -- \
  --server 127.0.0.1:5555 \
  --udp-port 6001 \
  --tickers "AAPL,TSLA"
```

## CLI аргументы

### `quote-server`

- `--tcp-bind <IP:PORT>`: TCP-адрес для команд (`STREAM`)
- `--udp-bind <IP:PORT>`: UDP-адрес сервера (приём ping, отправка котировок)
- `--tickers-file <PATH>`: файл тикеров (по одному на строку, поддержка `#` комментариев)
- `--tickers <CSV|multiline>`: тикеры строкой (альтернатива `--tickers-file`)

### `quote-client`

- `--server <HOST:PORT>`: TCP-адрес сервера
- `--udp-port <PORT>`: локальный UDP-порт для приёма котировок
- `--bind-ip <IP>`: IP, который клиент рекламирует серверу в `udp://IP:PORT`
- `--tickers-file <PATH>`: файл тикеров
- `--tickers <CSV>`: тикеры строкой (альтернатива файлу)

## Протокол (кратко)

### TCP команда

Клиент отправляет:

```text
STREAM udp://<client_ip>:<client_port> <TICKER1,TICKER2,...>
```

Сервер отвечает:
- `OK`
- или `ERR <причина>`

### UDP данные

Используется wire-протокол `quote-core::wire::UdpPacketV1`:
- `Quote(StockQuote)` — котировки
- `Ping` — keep-alive

## Keep-alive

- Клиент отправляет `Ping` раз в 2 секунды.
- Сервер ожидает ping не дольше 5 секунд.
- Если ping не приходит, сервер завершает стрим для этого клиента.

Проверка вручную:
1. Запусти сервер и клиент.
2. Останови клиент (`Ctrl+C`).
3. На сервере в логах должен появиться timeout и завершение сессии клиента.

## Формат файла тикеров

Пример:

```text
AAPL
GOOGL
TSLA
# комментарий
MSFT
```

Правила:
- пустые строки игнорируются
- комментарии (`#`) игнорируются
- тикеры нормализуются в uppercase
- дубликаты удаляются
