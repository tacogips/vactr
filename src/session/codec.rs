//! The JSON codec of the session protocol (design 14.5.6).
//!
//! Decoding never panics. It classifies every malformed frame:
//! - unparsable JSON, a non-object frame, or a frame over 1 MiB with
//!   `bad-json`/`bad-body`;
//! - a `v` other than 1 with `unsupported-version`;
//! - a `kind` outside the direction's closed list with `unknown-kind`;
//! - anything else wrong in the envelope or the body with `bad-body`.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value as Json};

use crate::session::protocol::{
    ClientMsg, Envelope, ErrorCode, ProtocolError, ServerMsg, PROTOCOL_VERSION,
};

/// The largest frame accepted, bytes.
pub const MAX_FRAME: usize = 1 << 20;

/// A message family with a closed list of kinds.
pub trait Message: Serialize + DeserializeOwned {
    /// Every valid kind of this direction.
    fn kinds() -> &'static [&'static str];
}

impl Message for ClientMsg {
    fn kinds() -> &'static [&'static str] {
        &ClientMsg::KINDS
    }
}

impl Message for ServerMsg {
    fn kinds() -> &'static [&'static str] {
        &ServerMsg::KINDS
    }
}

fn err(code: ErrorCode, message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(code, message)
}

/// Decodes one client frame.
///
/// # Errors
/// A `ProtocolError` with the code of the first problem found.
pub fn decode(text: &str) -> Result<Envelope<ClientMsg>, ProtocolError> {
    decode_as(text)
}

/// Decodes one frame of either direction.
///
/// # Errors
/// A `ProtocolError` with the code of the first problem found.
pub fn decode_as<M: Message>(text: &str) -> Result<Envelope<M>, ProtocolError> {
    if text.len() > MAX_FRAME {
        return Err(err(
            ErrorCode::BadBody,
            format!("the frame is larger than {MAX_FRAME} bytes"),
        ));
    }
    let json: Json =
        serde_json::from_str(text).map_err(|e| err(ErrorCode::BadJson, e.to_string()))?;
    let Json::Object(mut obj) = json else {
        return Err(err(ErrorCode::BadJson, "a frame is a JSON object"));
    };
    let v = obj
        .get("v")
        .ok_or_else(|| err(ErrorCode::BadBody, "missing `v`"))?;
    if v.as_u64() != Some(u64::from(PROTOCOL_VERSION)) {
        return Err(err(
            ErrorCode::UnsupportedVersion,
            format!("unsupported protocol version {v}"),
        ));
    }
    let seq = obj
        .get("seq")
        .and_then(Json::as_u64)
        .ok_or_else(|| err(ErrorCode::BadBody, "`seq` must be a non-negative integer"))?;
    let re = match obj.get("re") {
        None | Some(Json::Null) => None,
        Some(r) => Some(
            r.as_u64()
                .ok_or_else(|| err(ErrorCode::BadBody, "`re` must be a non-negative integer"))?,
        ),
    };
    let kind = match obj.get("kind") {
        Some(Json::String(k)) => k.clone(),
        _ => return Err(err(ErrorCode::BadBody, "`kind` must be a string")),
    };
    if !M::kinds().contains(&kind.as_str()) {
        return Err(err(
            ErrorCode::UnknownKind,
            format!("unknown message kind `{kind}`"),
        ));
    }
    let body = obj
        .remove("body")
        .unwrap_or_else(|| Json::Object(Map::new()));
    let mut tagged = Map::new();
    tagged.insert("kind".to_string(), Json::String(kind.clone()));
    tagged.insert("body".to_string(), body);
    let msg: M = serde_json::from_value(Json::Object(tagged))
        .map_err(|e| err(ErrorCode::BadBody, format!("`{kind}` body: {e}")))?;
    Ok(Envelope {
        v: PROTOCOL_VERSION,
        seq,
        re,
        body: msg,
    })
}

/// Encodes one frame of either direction.
#[must_use]
pub fn encode<M: Message>(env: &Envelope<M>) -> String {
    let mut obj = Map::new();
    obj.insert("v".to_string(), Json::from(env.v));
    obj.insert("seq".to_string(), Json::from(env.seq));
    if let Some(re) = env.re {
        obj.insert("re".to_string(), Json::from(re));
    }
    // An adjacently tagged enum always serializes to `{kind, body}`.
    if let Ok(Json::Object(tagged)) = serde_json::to_value(&env.body) {
        for (k, v) in tagged {
            obj.insert(k, v);
        }
    }
    Json::Object(obj).to_string()
}
