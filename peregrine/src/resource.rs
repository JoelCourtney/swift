use crate::history::HistoryAdapter;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use type_map::concurrent::TypeMap;
use type_reg::untagged::TypeReg;

#[macro_export]
macro_rules! resource {
    ($vis:vis $name:ident: $ty:ty) => {
        #[derive(Debug, $crate::reexports::serde::Serialize, $crate::reexports::serde::Deserialize)]
        #[serde(crate = "peregrine::reexports::serde")]
        #[allow(non_camel_case_types)]
        $vis enum $name {
            Unit
        }

        impl<'h> $crate::resource::Resource<'h> for $name {
            const STATIC: bool = true;
            type Read = $ty;
            type Write = $ty;
            type History = $crate::history::CopyHistory<$ty>;
        }

        impl $crate::resource::ResourceHistoryPlugin for $name {
            fn label(&self) -> String {
                $crate::reexports::peregrine_macros::code_to_str!($name).to_string()
            }

            fn write_type_string(&self) -> String {
                $crate::reexports::peregrine_macros::code_to_str!($ty).to_string()
            }

            fn ser<'h>(&self, input: &'h mut $crate::reexports::type_map::concurrent::TypeMap, type_map: &'h mut $crate::reexports::type_reg::untagged::TypeMap<String>) {
                if let Some(h) = input.remove::<$crate::history::CopyHistory<$ty>>() {
                    type_map.insert(self.write_type_string(), h);
                }
            }

            fn register(&self, type_reg: &mut $crate::reexports::type_reg::untagged::TypeReg<String>) {
                type_reg.register::<$crate::history::CopyHistory<$ty>>(self.write_type_string());
            }
            fn de<'h>(&self, output: &'h mut $crate::reexports::type_map::concurrent::TypeMap, type_map: &'h mut $crate::reexports::type_reg::untagged::TypeMap<String>) {
                match type_map.remove(&self.write_type_string()) {
                    Some(sub) => {
                        let sub_history = sub.into_inner().downcast::<$crate::history::CopyHistory<$ty>>();
                        match sub_history {
                            Ok(downcasted) => {
                                output.insert(*downcasted);
                            }
                            Err(_) => unreachable!()
                        }
                    }
                    None => {}
                }
            }
        }

        $crate::reexports::inventory::submit!(&$name::Unit as &dyn $crate::resource::ResourceHistoryPlugin);
    };

    ($vis:vis ref $name:ident: $ty:ty) => {
        #[derive(Debug, $crate::reexports::serde::Serialize, $crate::reexports::serde::Deserialize)]
        #[serde(crate = "peregrine::reexports::serde")]
        #[allow(non_camel_case_types)]
        $vis enum $name {
            Unit
        }

        impl<'h> $crate::resource::Resource<'h> for $name {
            const STATIC: bool = true;
            type Read = &'h <$ty as std::ops::Deref>::Target;
            type Write = $ty;
            type History = $crate::history::DerefHistory<$ty>;
        }

        impl $crate::resource::ResourceHistoryPlugin for $name {
            fn label(&self) -> String {
                $crate::reexports::peregrine_macros::code_to_str!($name).to_string()
            }

            fn write_type_string(&self) -> String {
                $crate::reexports::peregrine_macros::code_to_str!($ty).to_string()
            }

            fn ser<'h>(&self, input: &'h mut $crate::reexports::type_map::concurrent::TypeMap, type_map: &'h mut $crate::reexports::type_reg::untagged::TypeMap<String>) {
                if let Some(h) = input.remove::<$crate::history::DerefHistory<$ty>>() {
                    type_map.insert(self.write_type_string(), h);
                }
            }

            fn register(&self, type_reg: &mut $crate::reexports::type_reg::untagged::TypeReg<String>) {
                type_reg.register::<$crate::history::DerefHistory<$ty>>(self.write_type_string());
            }
            fn de<'h>(&self, output: &'h mut $crate::reexports::type_map::concurrent::TypeMap, type_map: &'h mut $crate::reexports::type_reg::untagged::TypeMap<String>) {
                match type_map.remove(&self.write_type_string()) {
                    Some(sub) => {
                        let sub_history = sub.into_inner().downcast::<$crate::history::DerefHistory<$ty>>();
                        match sub_history {
                            Ok(downcasted) => {
                                output.insert(*downcasted);
                            }
                            Err(_) => unreachable!()
                        }
                    }
                    None => {}
                }
            }
        }

        $crate::reexports::inventory::submit!(&$name::Unit as &dyn $crate::resource::ResourceHistoryPlugin);
    };
}

/// Marks a type as a resource label.
///
/// Resources are not part of a model, the model is a selection of existing resources. This allows
/// activities, which are also not part of a model, to be applied to any model that has the relevant
/// resources.
///
/// ## Reading & Writing
///
/// Resources are not represented one data type, but two, one for reading and one for writing.
/// For simple [Copy] resources these two types will be the same, and you won't have to worry about it.
/// For more complex resources they may be different but related types, like [String] and [&str][str].
/// This is for performance reasons, to avoid unnecessary cloning of heap-allocated data.
pub trait Resource<'h>: 'static + Sync {
    /// Whether the resource represents a value that can vary even when not actively written to by
    /// an operation. This is used for cache invalidation.
    const STATIC: bool;

    /// The type that is read from history.
    type Read: 'h + Copy + Send + Sync + Serialize;

    /// The type that is written from operations to history.
    type Write: 'h + From<Self::Read> + Clone + Debug + Serialize + DeserializeOwned + Send + Sync;

    /// The type of history container to use to store instances of the `Write` type, currently
    /// either [CopyHistory] or [DerefHistory]. See [Resource] for details.
    type History: 'static + HistoryAdapter<Self::Write, Self::Read> + Default + Send + Sync;
}

pub trait ResourceHistoryPlugin: Sync {
    fn label(&self) -> String;
    fn write_type_string(&self) -> String;

    fn ser<'h>(
        &self,
        input: &'h mut TypeMap,
        type_map: &'h mut type_reg::untagged::TypeMap<String>,
    );

    fn register(&self, type_reg: &mut TypeReg<String>);
    fn de<'h>(
        &self,
        output: &'h mut TypeMap,
        type_reg: &'h mut type_reg::untagged::TypeMap<String>,
    );
}
