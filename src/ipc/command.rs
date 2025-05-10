use {
    super::property::{PropValue, Property},
    serde::Serialize,
    std::marker::PhantomData,
};

pub(super) trait Command {
    type R: Serialize;
    fn json_values(&self) -> Self::R;
    fn to_command_json(&self) -> CommandJson<Self::R> {
        CommandJson {
            command: self.json_values(),
        }
    }
}

pub(super) struct ObserveProperty<T>(pub(super) PhantomData<T>);

impl<T: Property> Command for ObserveProperty<T> {
    type R = [serde_json::Value; 3];
    fn json_values(&self) -> Self::R {
        ["observe_property".into(), 1.into(), T::NAME.into()]
    }
}

pub(super) struct AudioAdd<'a>(pub(super) &'a str);

impl Command for AudioAdd<'_> {
    type R = [serde_json::Value; 2];

    fn json_values(&self) -> Self::R {
        ["audio-add".into(), self.0.into()]
    }
}

pub(super) struct AudioRemove(pub(super) u64);

impl Command for AudioRemove {
    type R = [serde_json::Value; 2];

    fn json_values(&self) -> Self::R {
        ["audio-remove".into(), self.0.into()]
    }
}

#[derive(Serialize)]
pub(super) struct CommandJson<T: Serialize> {
    command: T,
}

pub(super) struct SetProperty<P: Property>(pub(super) P::Value);

impl<P: Property> Command for SetProperty<P>
where
    P::Value: PropValue,
{
    type R = [serde_json::Value; 3];
    fn json_values(&self) -> Self::R {
        ["set_property".into(), P::NAME.into(), self.0.to_json()]
    }
}
