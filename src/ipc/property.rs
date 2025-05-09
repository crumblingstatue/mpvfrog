pub trait Property {
    const NAME: &'static str;
    type Value;
}

pub trait PropValue {
    fn to_json(&self) -> serde_json::Value;
}

impl PropValue for f64 {
    fn to_json(&self) -> serde_json::Value {
        (*self).into()
    }
}

impl PropValue for u64 {
    fn to_json(&self) -> serde_json::Value {
        (*self).into()
    }
}

impl PropValue for bool {
    fn to_json(&self) -> serde_json::Value {
        (*self).into()
    }
}

impl PropValue for Option<f64> {
    fn to_json(&self) -> serde_json::Value {
        match self {
            &Some(num) => num.into(),
            None => "no".into(),
        }
    }
}

impl PropValue for Option<&'static str> {
    fn to_json(&self) -> serde_json::Value {
        match self {
            &Some(val) => val.into(),
            None => "no".into(),
        }
    }
}

impl PropValue for String {
    fn to_json(&self) -> serde_json::Value {
        serde_json::Value::from(self.clone())
    }
}

macro_rules! decl_property {
    ($tyname:ident, $attrname:literal, $valty:ty) => {
        pub enum $tyname {}

        impl Property for $tyname {
            const NAME: &'static str = $attrname;
            type Value = $valty;
        }
    };
}

macro_rules! decl_properties {
    ($($tyname:ident, $attrname:literal, $valty:ty;)*) => {
        $(
            decl_property!($tyname, $attrname, $valty);
        )*
    };
}

decl_properties! {
    Volume, "volume", f64;
    Speed, "speed", f64;
    Pause, "pause", bool;
    TimePos, "time-pos", f64;
    Duration, "duration", f64;
    Video, "vid", Option<&'static str>;
    AbLoopA, "ab-loop-a", Option<f64>;
    AbLoopB, "ab-loop-b", Option<f64>;
    LavfiComplex, "lavfi-complex", String;
    Aid, "aid", u64;
}
