//! 从日志里的一行读回事件（施工 V-11 从 `event.rs` 挪出来、改写）：外壳一格一格读；读到 `body` 时已经知道种类的直接照那一种读
//! （核心自己写的 `kind` 都在 `body` 前面），不先把 `body` 原样截一份再读一遍：一万条事件的会话重开时，读日志是大头。种类写在
//! `body` 后面的、不认识的种类，照旧先原样截下来。可选字段没有、写成 `null`，都当没有；不认识的字段不管；同一格写了两遍的
//! 报错。
//!
//! 只解要的种类的（施工 V-39 上补，`only.rs`）走同一份外壳的读法，只是 `body` 照种类解多深。

use std::fmt;

use serde::de::{self, DeserializeSeed, Error as _, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::only::{self, Depth};
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

/// 一行事件：`only` 不是空的只解这几种（`only.rs`）。
struct Line<'a> {
    only: Option<&'a [&'a str]>,
}

impl<'de> Visitor<'de> for Line<'_> {
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
        // 不解的：`body` 只过了一遍。
        let mut skipped = false;
        while let Some(field) = map.next_key::<Field>()? {
            match field {
                Field::Seq => once(&mut seq, "seq", map.next_value()?)?,
                Field::At => once(&mut at, "at", map.next_value()?)?,
                Field::Kind => once(&mut kind, "kind", map.next_value()?)?,
                Field::Turn => once(&mut turn, "turn", map.next_value()?)?,
                Field::By => once(&mut by, "by", map.next_value()?)?,
                Field::Cause => once(&mut cause, "cause", map.next_value()?)?,
                Field::Body => {
                    if body.is_some() || raw.is_some() || skipped {
                        return Err(A::Error::duplicate_field("body"));
                    }
                    match &kind {
                        Some(kind) => match only::depth(self.only, kind) {
                            Depth::Full => body = Some(map.next_value_seed(BodySeed(kind))?),
                            Depth::Seen => body = Some(map.next_value_seed(SeenSeed(kind))?),
                            Depth::Skip => {
                                map.next_value::<IgnoredAny>()?;
                                skipped = true;
                            }
                        },
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
            (None, Some(raw)) => match only::depth(self.only, &kind) {
                Depth::Full => Body::read(kind, raw).map_err(A::Error::custom)?,
                Depth::Seen => {
                    let mut d = serde_json::Deserializer::from_str(raw.get());
                    only::seen(&kind, &mut d).map_err(A::Error::custom)?
                }
                Depth::Skip => only::skipped(kind),
            },
            (None, None) if skipped => only::skipped(kind),
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

/// 照已经读到的种类只解几格（`only.rs` 的 [`Depth::Seen`]）。
struct SeenSeed<'k>(&'k EventKind);

impl<'de> DeserializeSeed<'de> for SeenSeed<'_> {
    type Value = Body;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Body, D::Error> {
        only::seen(self.0, d)
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

/// 只解 `only` 里的种类读一行（[`Event::from_line_only`]）：读不成的交回错，调的一方照全解再读一遍。
pub(super) fn line_only(line: &str, only: &[&str]) -> serde_json::Result<Event> {
    let mut d = serde_json::Deserializer::from_str(line);
    let event = d.deserialize_struct("Event", FIELDS, Line { only: Some(only) })?;
    d.end()?;
    Ok(event)
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_struct("Event", FIELDS, Line { only: None })
    }
}
