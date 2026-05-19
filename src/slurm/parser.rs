use regex::Regex;
use serde::{de, forward_to_deserialize_any};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error {
            message: msg.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error {
            message: msg.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn from_str<'de, T: de::Deserialize<'de>>(input: &'de str) -> Result<T> {
    let deserializer = SlurmDeserializer::from_str(input);
    T::deserialize(deserializer)
}

pub struct SlurmDeserializer<'de> {
    input: &'de str,
}

impl<'de> SlurmDeserializer<'de> {
    pub fn from_str(input: &'de str) -> Self {
        SlurmDeserializer { input }
    }
}

impl<'de> de::Deserializer<'de> for SlurmDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let records: Vec<&str> = self
            .input
            .split("\n\n")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if records.len() == 1 {
            self.deserialize_map(visitor)
        } else {
            self.deserialize_seq(visitor)
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let records: Vec<&str> = self
            .input
            .split("\n\n")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        visitor.visit_seq(RecordSeq {
            records,
            current: 0,
        })
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let record = self
            .input
            .split("\n\n")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .next()
            .ok_or_else(|| de::Error::custom("no record found"))?;
        let mut map = HashMap::new();
        let key_regex = Regex::new(r"(?:^|[\s])([a-zA-Z0-9_/\-:.]+)=")
            .map_err(|e| de::Error::custom(e.to_string()))?;

        let matches: Vec<_> = key_regex.find_iter(record).collect();

        for i in 0..matches.len() {
            let matched = matches[i];
            let key_capture = key_regex
                .captures(matched.as_str())
                .and_then(|captures| captures.get(1))
                .ok_or_else(|| de::Error::custom("invalid slurm key capture"))?;
            let key = key_capture.as_str();

            let val_start = matched.end();
            let val_end = if i + 1 < matches.len() {
                matches[i + 1].start()
            } else {
                record.len()
            };

            let value = record[val_start..val_end].trim();
            if value.is_empty() || value == "(null)" || value == "None" || value == "N/A" {
                continue;
            }

            match map.entry(key) {
                Entry::Occupied(mut entry) => {
                    let slot = entry.get_mut();
                    match slot {
                        SlurmValue::Single(existing) => {
                            let first = *existing;
                            *slot = SlurmValue::Repeated(vec![first, value]);
                        }
                        SlurmValue::Repeated(values) => values.push(value),
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(SlurmValue::Single(value));
                }
            }
        }

        let items: Vec<(&str, SlurmValue<'de>)> = map.into_iter().collect();
        visitor.visit_map(SlurmRecord { items, current: 0 })
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct tuple tuple_struct enum
        identifier ignored_any struct option
    }
}

struct RecordSeq<'de> {
    records: Vec<&'de str>,
    current: usize,
}

impl<'de> de::SeqAccess<'de> for RecordSeq<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        if self.current >= self.records.len() {
            return Ok(None);
        }
        let record = self.records[self.current];
        self.current += 1;
        seed.deserialize(SlurmDeserializer::from_str(record))
            .map(Some)
    }
}

struct SlurmRecord<'de> {
    items: Vec<(&'de str, SlurmValue<'de>)>,
    current: usize,
}

impl<'de> de::MapAccess<'de> for SlurmRecord<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        if self.current >= self.items.len() {
            return Ok(None);
        }
        seed.deserialize(de::value::StrDeserializer::new(self.items[self.current].0))
            .map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        let value = self.items[self.current].1.clone();
        self.current += 1;
        seed.deserialize(value)
    }
}

#[derive(Clone)]
enum SlurmValue<'de> {
    Single(&'de str),
    Repeated(Vec<&'de str>),
}

macro_rules! impl_num_visitor {
    {$($type:ident)*} => {
        paste::paste! {
            $(fn [<deserialize_ $type>]<V>(self, visitor: V) -> Result<V::Value>
            where
                V: de::Visitor<'de>,
            {
                match self {
                    SlurmValue::Single(s) => visitor.[<visit_ $type>](
                        s.parse().map_err(|_| de::Error::custom(format!("invalid number: {s}")))?
                    ),
                    SlurmValue::Repeated(_) => self.deserialize_any(visitor),
                }
            })*
        }
    }
}

impl<'de> de::Deserializer<'de> for SlurmValue<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self {
            SlurmValue::Single(s) => visitor.visit_borrowed_str(s),
            SlurmValue::Repeated(values) => visitor.visit_seq(ValueSeq { values, current: 0 }),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self {
            SlurmValue::Single(s) => {
                if s == "1" || s.eq_ignore_ascii_case("true") {
                    visitor.visit_bool(true)
                } else if s == "0" || s.eq_ignore_ascii_case("false") {
                    visitor.visit_bool(false)
                } else {
                    Err(de::Error::custom(format!("expected bool, got {s}")))
                }
            }
            SlurmValue::Repeated(_) => self.deserialize_any(visitor),
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self {
            SlurmValue::Single(s) => visitor.visit_seq(ValueSeq {
                values: s.split(',').collect(),
                current: 0,
            }),
            SlurmValue::Repeated(values) => visitor.visit_seq(ValueSeq { values, current: 0 }),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let items = match self {
            SlurmValue::Single(s) => s
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    s.trim()
                        .split_once('=')
                        .ok_or_else(|| de::Error::custom(format!("invalid key-value pair: {s}")))
                })
                .collect::<Result<Vec<_>>>()?,
            SlurmValue::Repeated(_) => return self.deserialize_any(visitor),
        };
        visitor.visit_map(ValueMap { items, current: 0 })
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self {
            SlurmValue::Single(s) => visitor.visit_borrowed_str(s),
            SlurmValue::Repeated(values) => visitor.visit_borrowed_str(values[0]),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self {
            SlurmValue::Single(s) => visitor.visit_enum(ValueEnum { value: s }),
            SlurmValue::Repeated(_) => self.deserialize_any(visitor),
        }
    }

    impl_num_visitor! {
        u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64
    }

    forward_to_deserialize_any! {
        char string str bytes byte_buf unit unit_struct newtype_struct ignored_any
    }
}

struct ValueEnum<'de> {
    value: &'de str,
}

impl<'de> de::EnumAccess<'de> for ValueEnum<'de> {
    type Variant = EnumVariant;
    type Error = Error;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(SlurmValue::Single(self.value))
            .map(|value| (value, EnumVariant))
    }
}

struct EnumVariant;

impl<'de> de::VariantAccess<'de> for EnumVariant {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        Err(Error::custom("newtype variant not supported"))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::custom("tuple variant not supported"))
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::custom("struct variant not supported"))
    }
}

struct ValueSeq<'de> {
    values: Vec<&'de str>,
    current: usize,
}

impl<'de> de::SeqAccess<'de> for ValueSeq<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        if self.current >= self.values.len() {
            return Ok(None);
        }
        let value = self.values[self.current];
        self.current += 1;
        seed.deserialize(SlurmValue::Single(value)).map(Some)
    }
}

struct ValueMap<'de> {
    items: Vec<(&'de str, &'de str)>,
    current: usize,
}

impl<'de> de::MapAccess<'de> for ValueMap<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        if self.current >= self.items.len() {
            return Ok(None);
        }
        let key = self.items[self.current].0;
        seed.deserialize(SlurmValue::Single(key)).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        let value = self.items[self.current].1;
        self.current += 1;
        seed.deserialize(SlurmValue::Single(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[test]
    fn parses_node_record() {
        let input = "NodeName=node4504 Arch=x86_64 CoresPerSocket=32
CPUAlloc=0 CPUEfctv=64 CPUTot=64 CPULoad=0.04
OS=Linux 4.18.0 #1 SMP Tue May 10 14:48:47 UTC 2022
AllocTRES=
State=IDLE";

        #[allow(non_snake_case)]
        #[derive(Deserialize, Debug, PartialEq)]
        struct Node {
            NodeName: String,
            Arch: String,
            OS: String,
            CoresPerSocket: u32,
            CPULoad: f32,
            AllocTRES: Option<String>,
            State: String,
        }

        let node: Node = from_str(input).unwrap();
        assert_eq!(node.NodeName, "node4504");
        assert_eq!(node.CoresPerSocket, 32);
        assert_eq!(node.CPULoad, 0.04);
        assert_eq!(node.AllocTRES, None);
        assert_eq!(node.State, "IDLE");
    }

    #[test]
    fn parses_multi_records() {
        #[allow(non_snake_case)]
        #[derive(Deserialize, Debug, PartialEq)]
        struct Node {
            NodeName: String,
            State: String,
        }

        let nodes: Vec<Node> =
            from_str("NodeName=node1 State=IDLE\n\nNodeName=node2 State=ALLOCATED").unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[1].NodeName, "node2");
    }
}