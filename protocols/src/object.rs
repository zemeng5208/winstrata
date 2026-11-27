use std::{fmt, marker::PhantomData};

use serde::de::{
    DeserializeSeed, IntoDeserializer, MapAccess, Visitor, value::MapAccessDeserializer,
};
use serde::{Deserialize, Deserializer};

// Serde 的派生实现也能从位置数组读取结构体。协议要求对象字段有名称。
pub(crate) struct Object<T>(pub(crate) T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        required(deserializer).map(Object)
    }
}

struct ObjectVisitor<T, const TAGGED: bool>(PhantomData<T>);

impl<'de, T: Deserialize<'de>, const TAGGED: bool> Visitor<'de> for ObjectVisitor<T, TAGGED> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object with named fields")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        if TAGGED {
            let fields = TaggedFields {
                map,
                reading_kind: false,
            };
            T::deserialize(MapAccessDeserializer::new(fields))
        } else {
            T::deserialize(MapAccessDeserializer::new(map))
        }
    }
}

// 缓冲后的枚举标签也可能被 Serde 解释为数字序号。仅 kind 必须读取为字符串。
struct TaggedFields<A> {
    map: A,
    reading_kind: bool,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for TaggedFields<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, A::Error> {
        let Some(key) = self.map.next_key::<String>()? else {
            return Ok(None);
        };
        self.reading_kind = key == "kind";
        seed.deserialize(key.into_deserializer()).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, A::Error> {
        if self.reading_kind {
            let kind = self.map.next_value::<String>()?;
            seed.deserialize(kind.into_deserializer())
        } else {
            self.map.next_value_seed(seed)
        }
    }
}

pub(crate) fn required<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    deserializer.deserialize_map(ObjectVisitor::<T, false>(PhantomData))
}

pub(crate) fn tagged<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    deserializer.deserialize_map(ObjectVisitor::<T, true>(PhantomData))
}

pub(crate) fn optional<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<Object<T>>::deserialize(deserializer).map(|value| value.map(|object| object.0))
}
