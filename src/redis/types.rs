use std::fmt;
use std::io::IoError;
use std::from_str::from_str;
use std::str::from_utf8;
use std::collections::{HashMap, Collection};


#[deriving(PartialEq, Eq, Clone, Show)]
pub enum ErrorKind {
    ResponseError,
    TypeError,
    ExecAbortError,
    BusyLoadingError,
    NoScriptError,
    ExtensionError(String),
    InternalIoError(IoError),
}


#[deriving(PartialEq, Eq, Clone)]
pub enum Value {
    Nil,
    Int(i64),
    Data(Vec<u8>),
    Bulk(Vec<Value>),
    Okay,
    Status(String),
}

impl fmt::Show for Value {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Nil => write!(fmt, "nil"),
            &Int(val) => write!(fmt, "int({})", val),
            &Data(ref val) => {
                match from_utf8(val[]) {
                    Some(x) => write!(fmt, "string-data('{}')", x.escape_default()),
                    None => write!(fmt, "binary-data({})", val),
                }
            },
            &Bulk(ref values) => {
                try!(write!(fmt, "bulk("));
                let mut is_first = true;
                for val in values.iter() {
                    if !is_first {
                        try!(write!(fmt, ", "));
                    }
                    try!(write!(fmt, "{}", val));
                    is_first = false;
                }
                write!(fmt, ")")
            },
            &Okay => write!(fmt, "ok"),
            &Status(ref s) => write!(fmt, "status({})", s),
        }
    }
}


#[deriving(PartialEq, Eq, Clone, Show)]
pub struct Error {
    pub kind: ErrorKind,
    pub desc: &'static str,
    pub detail: Option<String>,
}


pub type RedisResult<T> = Result<T, Error>;


impl Error {

    pub fn simple(kind: ErrorKind, desc: &'static str) -> Error {
        Error {
            kind: kind,
            desc: desc,
            detail: None,
        }
    }
}


pub trait ToRedisArg {
    fn to_redis_arg(&self) -> Vec<u8>;
}


macro_rules! invalid_type_error(
    ($t:ty, $v:expr, $det:expr) => ({
        return Err(Error {
            kind: TypeError,
            desc: "Response was of incompatible type",
            detail: Some(format!("{} (response was {})", $det, $v)),
         });
    })
)

macro_rules! format_for_redis(
    ($v:expr) => ({
        let mut rv = vec![];
        let b = $v;
        rv.push_all(format!("${}\r\n", b.len()).as_bytes());
        rv.push_all(b[]);
        rv.push_all(b"\r\n");
        rv
    })
)

macro_rules! string_based_to_redis_impl(
    ($t:ty) => (
        impl ToRedisArg for $t {
            fn to_redis_arg(&self) -> Vec<u8> {
                let s = self.to_string();
                format_for_redis!(s.as_bytes())
            }
        }
    )
)


string_based_to_redis_impl!(bool)
string_based_to_redis_impl!(i32)
string_based_to_redis_impl!(u32)
string_based_to_redis_impl!(i64)
string_based_to_redis_impl!(u64)
string_based_to_redis_impl!(f32)
string_based_to_redis_impl!(f64)
string_based_to_redis_impl!(int)
string_based_to_redis_impl!(uint)


impl ToRedisArg for String {
    fn to_redis_arg(&self) -> Vec<u8> {
        format_for_redis!(self.as_bytes())
    }
}

impl<'a> ToRedisArg for &'a str {
    fn to_redis_arg(&self) -> Vec<u8> {
        format_for_redis!(self.as_bytes())
    }
}

impl ToRedisArg for Vec<u8> {
    fn to_redis_arg(&self) -> Vec<u8> {
        format_for_redis!(self.to_vec())
    }
}

impl<'a> ToRedisArg for &'a [u8] {
    fn to_redis_arg(&self) -> Vec<u8> {
        format_for_redis!(self.to_vec())
    }
}


/// This trait is used to convert a redis value into a more appropriate
/// type.  While a redis `Value` can represent any response that comes
/// back from the redis server, usually you want to map this into something
/// that works better in rust.  For instance you might want to convert the
/// return value into a `String` or an integer.
///
/// This trait is well supported throughout the library and you can
/// implement it for your own types if you want.
pub trait FromRedisValue {
    /// Given a redis `Value` this attempts to convert it into the given
    /// destination type.  If that fails because it's not compatible an
    /// appropriate error is generated.
    fn from_redis_value(v: &Value) -> RedisResult<Self>;

    /// This only exists internally as a workaround for the lack of
    /// specialization.
    #[doc(hidden)]
    fn from_byte_vec(_vec: &[u8]) -> Option<Vec<Self>> {
        None
    }
}

macro_rules! from_redis_value_for_num_internal(
    ($t:ty, $v:expr) => (
        {
            let v = $v;
            match v {
                &Int(val) => Ok(val as $t),
                &Data(ref bytes) => {
                    match from_utf8(bytes[]) {
                        Some(s) => match from_str(s.as_slice()) {
                            Some(rv) => Ok(rv),
                            None => invalid_type_error!($t, v,
                                "Could not convert from string.")
                        },
                        None => invalid_type_error!($t, v,
                            "Invalid UTF-8 string."),
                    }
                },
                _ => invalid_type_error!($t, v,
                    "Response type not convertible to numeric.")
            }
        }
    )
)

macro_rules! from_redis_value_for_num(
    ($t:ty) => (
        impl FromRedisValue for $t {
            fn from_redis_value(v: &Value) -> RedisResult<$t> {
                from_redis_value_for_num_internal!($t, v)
            }
        }
    )
)

impl FromRedisValue for u8 {
    fn from_redis_value(v: &Value) -> RedisResult<u8> {
        from_redis_value_for_num_internal!(u8, v)
    }

    fn from_byte_vec(vec: &[u8]) -> Option<Vec<u8>> {
        Some(vec.to_vec())
    }
}

from_redis_value_for_num!(i16)
from_redis_value_for_num!(u16)
from_redis_value_for_num!(i32)
from_redis_value_for_num!(u32)
from_redis_value_for_num!(i64)
from_redis_value_for_num!(u64)
from_redis_value_for_num!(f32)
from_redis_value_for_num!(f64)
from_redis_value_for_num!(int)
from_redis_value_for_num!(uint)

impl FromRedisValue for bool {
    fn from_redis_value(v: &Value) -> RedisResult<bool> {
        match v {
            &Nil => Ok(false),
            &Int(val) => Ok(val != 0),
            &Status(ref s) => {
                if s.as_slice() == "1" { Ok(true) }
                else if s.as_slice() == "0" { Ok(false) }
                else {
                    invalid_type_error!(bool, v,
                        "Response status not valid boolean");
                }
            }
            &Okay => Ok(true),
            _ => invalid_type_error!(bool, v,
                "Response type not bool compatible."),
        }
    }
}

impl FromRedisValue for String {
    fn from_redis_value(v: &Value) -> RedisResult<String> {
        match v {
            &Data(ref bytes) => {
                match from_utf8(bytes[]) {
                    Some(s) => Ok(s.to_string()),
                    None => invalid_type_error!(String, v,
                        "Invalid UTF-8 string."),
                }
            },
            &Okay => Ok("OK".to_string()),
            &Status(ref val) => Ok(val.to_string()),
            _ => invalid_type_error!(String, v,
                "Response type not string compatible."),
        }
    }
}

impl<T: FromRedisValue> FromRedisValue for Vec<T> {
    fn from_redis_value(v: &Value) -> RedisResult<Vec<T>> {
        match v {
            // this hack allows us to specialize Vec<u8> to work with
            // binary data whereas all others will fail with an error.
            &Data(ref bytes) => {
                match FromRedisValue::from_byte_vec(bytes.as_slice()) {
                    Some(x) => Ok(x),
                    None => invalid_type_error!(Vec<T>, v,
                        "Response type not vector compatible.")
                }
            },
            &Bulk(ref items) => {
                let mut rv = vec![];
                for item in items.iter() {
                    match FromRedisValue::from_redis_value(item) {
                        Ok(val) => rv.push(val),
                        Err(_) => {},
                    }
                }
                Ok(rv)
            }
            &Nil => {
                Ok(vec![])
            },
            _ => invalid_type_error!(Vec<T>, v,
                "Response type not vector compatible.")
        }
    }
}

impl FromRedisValue for Value {
    fn from_redis_value(v: &Value) -> RedisResult<Value> {
        Ok(v.clone())
    }
}

impl FromRedisValue for () {
    fn from_redis_value(_v: &Value) -> RedisResult<()> {
        Ok(())
    }
}

pub struct InfoDict {
    map: HashMap<String, Value>,
}

/// This type provides convenient access to key/value data returned by
/// the "INFO" command.  It acts like a regular mapping but also has
/// a convenience method `get` which can return data in the appropriate
/// type.
///
/// For instance this can be used to query the server for the role it's
/// in (master, slave) etc:
///
/// ```rust,no_run
/// # let client = redis::Client::open("redis://127.0.0.1/").unwrap();
/// # let con = client.get_connection().unwrap();
/// let info : redis::InfoDict = redis::cmd("INFO").query(&con).unwrap();
/// let role : String = info.get("role").unwrap();
/// ```
impl InfoDict {
    /// Creates a new info dictionary from a string in the response of
    /// the INFO command.  Each line is a key, value pair with the
    /// key and value separated by a colon (`:`).  Lines starting with a
    /// hash (`#`) are ignored.
    pub fn new(kvpairs: &str) -> InfoDict {
        let mut map = HashMap::new();
        for line in kvpairs.lines_any() {
            if line.len() == 0 || line.starts_with("#") {
                continue;
            }
            let mut p = line.splitn(1, ':');
            let key = p.next();
            let value = p.next();
            if value.is_some() {
                map.insert(key.unwrap().to_string(),
                           Status(value.unwrap().to_string()));
            }
        }
        InfoDict { map: map }
    }

    /// Fetches a value by key and converts it into the given type.
    /// Typical types are `String`, `bool` and integer types.
    pub fn get<T: FromRedisValue>(&self, key: &str) -> Option<T> {
        match self.find(&key) {
            Some(ref x) => FromRedisValue::from_redis_value(*x).ok(),
            None => None,
        }
    }
}

impl FromRedisValue for InfoDict {
    fn from_redis_value(v: &Value) -> RedisResult<InfoDict> {
        let s : String = try!(FromRedisValue::from_redis_value(v));
        Ok(InfoDict::new(s.as_slice()))
    }
}

impl<'a> Map<&'a str, Value> for InfoDict {

    fn find<'x>(&'x self, key: &&str) -> Option<&'x Value> {
        self.map.find_equiv(key)
    }

    fn contains_key<'x>(&'x self, key: &&str) -> bool {
        self.find(key).is_some()
    }
}

impl Collection for InfoDict {
    fn len(&self) -> uint {
        self.map.len()
    }
}
