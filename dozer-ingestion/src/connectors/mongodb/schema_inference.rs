use bson::Bson;
use dozer_types::{log, types::FieldType};

pub(crate) fn value_to_dtype(val: &Bson) -> FieldType {
    match val {
        Bson::Int32(_) | Bson::Int64(_) => FieldType::Int,

        Bson::Double(_) => FieldType::Float,
        Bson::Decimal128(_) => FieldType::Decimal,
        Bson::Boolean(_) => FieldType::Boolean,
        Bson::Array(_) => {
            todo!("arrays are not yet supported by dozer")
        }
        Bson::Document(_) => FieldType::Json,
        Bson::DateTime(_) => todo!("DateTime"),
        Bson::String(_) => FieldType::String,
        Bson::Binary(_) => FieldType::Binary,
        Bson::ObjectId(_) => FieldType::String,
        Bson::Timestamp(_) => FieldType::Timestamp,
        other => {
            log::error!(
                "unable to convert {} to Dozer FieldType, defaulting to String",
                other
            );
            FieldType::String
        }
    }
}

// fn deserialize_all<'a>(bson: &Bson) -> AnyValue<'a> {
//     match bson {
//         Bson::Null | Bson::Undefined => AnyValue::Null,
//         Bson::Boolean(b) => AnyValue::Boolean(*b),
//         Bson::Int32(i) => AnyValue::Int32(*i),
//         Bson::Int64(i) => AnyValue::Int64(*i),
//         Bson::Double(f) => AnyValue::Float64(*f),
//         Bson::String(v) => AnyValue::Utf8Owned(v.into()),
//         Bson::Array(arr) => {
//             let vals: Vec<AnyValue> = arr.iter().map(deserialize_all).collect();
//             let s = Series::new("", vals);
//             AnyValue::List(s)
//         }
//         Bson::Timestamp(v) => AnyValue::Utf8Owned(format!("{v:#?}").into()),
//         Bson::DateTime(dt) => {
//             AnyValue::Datetime(dt.timestamp_millis(), TimeUnit::Milliseconds, &None)
//         }
//         Bson::Binary(b) => {
//             let s = Series::new("", &b.bytes);
//             AnyValue::List(s)
//         }
//         Bson::ObjectId(oid) => AnyValue::Utf8Owned(oid.to_string().into()),
//         Bson::Symbol(s) => AnyValue::Utf8Owned(s.into()),
//         Bson::Document(doc) => {
//             let vals: (Vec<AnyValue>, Vec<Field>) = doc
//                 .into_iter()
//                 .map(|(key, value)| {
//                     let dt = value_to_dtype(value);
//                     let fld = Field::new(key, dt);
//                     let av = deserialize_all(value);
//                     (av, fld)
//                 })
//                 .unzip();

//             AnyValue::StructOwned(Box::new(vals))
//         }
//         v => {
//             log::trace!("unable to properly deserialize {v}, defaulting to string");

//             AnyValue::Utf8Owned(format!("{v:#?}").into())
//         }
//     }
// }
