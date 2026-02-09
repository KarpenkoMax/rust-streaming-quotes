use thiserror::Error;

/// Верхнеуровневый тип ошибок крейта
#[derive(Debug, Error)]
pub enum QuoteCoreError {
    /// Ошибки протокола
    #[error(transparent)]
    Protocol(#[from] ProtocolError),

    /// Ошибки сериализации
    #[error(transparent)]
    Wire(#[from] WireError),
}

/// Ошибки протокола
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// пустая команда
    #[error("empty command")]
    EmptyCommand,

    /// Отсутствует название команды
    #[error("missing command name")]
    MissingCommand,

    /// Неизвестная команда
    #[error("unknown command: {0}")]
    UnknownCommand(String),

    /// Не передан UDP адрес клиента
    #[error("missing udp target")]
    MissingUdpTarget,

    /// Неверный формат UPD
    #[error("udp target must start with udp://")]
    BadUdpScheme,

    /// Неверный формат UPD
    #[error("invalid udp address: {0}")]
    InvalidUdpAddress(String),

    /// Не передан список тикеров
    #[error("missing tickers list")]
    MissingTickers,

    /// Список тикеров пуст
    #[error("tickers list is empty")]
    EmptyTickers,

    /// Лишние аргументы
    #[error("unexpected extra arguments")]
    ExtraArgs,
}

/// Ошибки сериализации
#[derive(Debug, Error)]
pub enum WireError {
    /// Пакер слишком короткий (не соотв. заявленной длине)
    #[error("packet too short")]
    PacketTooShort,

    /// Неверная версия протокола
    #[error("unsupported wire version: {0}")]
    UnsupportedWireVersion(u8),

    /// Ошибка сериализации/десериализации
    #[error("postcard encode/decode error: {0}")]
    Postcard(#[from] postcard::Error),
}
