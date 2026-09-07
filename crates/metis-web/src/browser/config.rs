use moirai_pal::wasm::WebDocument;
use std::io;

pub(super) struct BridgeConfig {
    pub(super) endpoint: String,
    pub(super) process_id: u32,
    pub(super) principal: [u8; 16],
}

pub(super) fn read_bridge_config(document: &WebDocument) -> io::Result<Option<BridgeConfig>> {
    let Some(endpoint) = optional_value(document, "metis-websocket-endpoint") else {
        return Ok(None);
    };
    if endpoint.trim().is_empty() {
        return Ok(None);
    }
    if !endpoint.starts_with("ws://") && !endpoint.starts_with("wss://") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Browser endpoint must use ws:// or wss://",
        ));
    }
    let process_id = optional_value(document, "metis-process-id")
        .ok_or_else(|| config_error("Browser process identifier is missing"))?
        .parse::<u32>()
        .map_err(|_| config_error("Browser process identifier is not a positive integer"))?;
    if process_id == 0 {
        return Err(config_error("Browser process identifier must be nonzero"));
    }
    let principal = optional_value(document, "metis-principal")
        .ok_or_else(|| config_error("Browser principal is missing"))?;
    Ok(Some(BridgeConfig {
        endpoint,
        process_id,
        principal: parse_principal(&principal)?,
    }))
}

fn optional_value(document: &WebDocument, id: &str) -> Option<String> {
    document
        .get_element_by_id(id)
        .and_then(|element| element.value())
}

fn parse_principal(value: &str) -> io::Result<[u8; 16]> {
    let bytes = value.as_bytes();
    if bytes.len() != 32 {
        return Err(config_error("Browser principal must contain 32 hex digits"));
    }
    let mut principal = [0; 16];
    for (index, slot) in principal.iter_mut().enumerate() {
        let high = hex_digit(bytes[index * 2])?;
        let low = hex_digit(bytes[index * 2 + 1])?;
        *slot = (high << 4) | low;
    }
    if principal == [0; 16] {
        return Err(config_error("Browser principal must be nonzero"));
    }
    Ok(principal)
}

fn hex_digit(value: u8) -> io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(config_error("Browser principal contains a non-hex digit")),
    }
}

fn config_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
