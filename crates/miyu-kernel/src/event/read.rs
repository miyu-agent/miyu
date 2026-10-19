//! 从日志里的一行读回事件（施工 V-11 从 `event.rs` 挪出来、改写）：外壳一格一格读；读到 `body` 时已经知道种类的直接照那一种读
//! （核心自己写的 `kind` 都在 `body` 前面），不先把 `body` 原样截一份再读一遍：一万条事件的会话重开时，读日志是大头。种类写在
//! `body` 后面的、不认识的种类，照旧先原样截下来。可选字段没有、写成 `null`，都当没有；不认识的字段不管；同一格写了两遍的
//! 报错。

use std::fmt;

use serde::de::{self, DeserializeSeed, Error as _, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::{Body, Event};
use crate::id::{CommandId, EventKind, Seq, TurnId};
use crate::origin::By;
use crate::raw::RawJson;
use crate::time::Timestamp;

/// 外壳的几格。
const FIELDS: &[&str] = &["seq", "at", "kind", "turn", "by", "cause", "body"];

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum Field {
    Seq,
    At,
    Kind,
    Turn,
    By,
    Cause,
    Body,
    #[serde(other)]
    Other,
}

/// 照已经读到的种类读 `body`。
struct BodySeed<'k>(&'k EventKind);

impl<'de> DeserializeSeed<'de> for BodySeed<'_> {
    type Value = Body;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Body, D::Error> {
        Body::read_from(self.0, d)
    }
}

/// 一行事件。
struct Line;

impl<'de> Visitor<'de> for Line {
    type Value = Event;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an event")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Event, A::Error> {
        let mut seq: Option<Seq> = None;
        let mut at: Option<Timestamp> = None;
        let mut kind: Option<EventKind> = None;
        let mut turn: Option<Option<TurnId>> = None;
        let mut by: Option<By> = None;
        let mut cause: Option<Option<CommandId>> = None;
        let mut body: Option<Body> = None;
        let mut raw: Option<RawJson> = None;
        while let Some(field) = map.next_key::<Field>()? {
            match field {
                Field::Seq => once(&mut seq, "seq", map.next_value()?)?,
                Field::At => once(&mut at, "at", map.next_value()?)?,
                Field::Kind => once(&mut kind, "kind", map.next_value()?)?,
                Field::Turn => once(&mut turn, "turn", map.next_value()?)?,
                Field::By => once(&mut by, "by", map.next_value()?)?,
                Field::Cause => once(&mut cause, "cause", map.next_value()?)?,
                Field::Body => {
                    if body.is_some() || raw.is_some() {
                        return Err(A::Error::duplicate_field("body"));
                    }
                    match &kind {
                        Some(kind) => body = Some(map.next_value_seed(BodySeed(kind))?),
                        None => raw = Some(map.next_value()?),
                    }
                }
                Field::Other => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        let kind = kind.ok_or_else(|| A::Error::missing_field("kind"))?;
        let body = match (body, raw) {
            (Some(body), _) => body,
            (None, Some(raw)) => Body::read(kind, raw).map_err(A::Error::custom)?,
            (None, None) => return Err(A::Error::missing_field("body")),
        };
        Ok(Event {
            seq: seq.ok_or_else(|| A::Error::missing_field("seq"))?,
            at: at.ok_or_else(|| A::Error::missing_field("at"))?,
            turn: turn.flatten(),
            by: by.ok_or_else(|| A::Error::missing_field("by"))?,
            cause: cause.flatten(),
            body,
        })
    }
}

/// 一格只许写一遍。
fn once<T, E: de::Error>(slot: &mut Option<T>, name: &'static str, value: T) -> Result<(), E> {
    if slot.is_some() {
        return Err(E::duplicate_field(name));
    }
    *slot = Some(value);
    Ok(())
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_struct("Event", FIELDS, Line)
    }
}
