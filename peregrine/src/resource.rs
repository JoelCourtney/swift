use crate::History;
use type_reg::untagged::TypeReg;

pub trait ResourceHistoryPlugin: Sync {
    fn label(&self) -> String;
    fn write_type_string(&self) -> String;

    fn ser<'h>(&self, history: &'h History, type_map: &'h mut type_reg::untagged::TypeMap<String>);

    fn register(&self, type_reg: &mut TypeReg<String>);
    fn de<'h>(
        &self,
        history: &'h mut History,
        type_reg: &'h mut type_reg::untagged::TypeMap<String>,
    );
}

#[macro_export]
macro_rules! resource {
    ($vis:vis $name:ident: $ty:ty) => {
        #[derive(Debug, $crate::reexports::serde::Serialize, $crate::reexports::serde::Deserialize)]
        #[serde(crate = "peregrine::reexports::serde")]
        #[allow(non_camel_case_types)]
        $vis enum $name {
            Unit
        }

        impl<'h> $crate::Resource<'h> for $name {
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

            fn ser<'h>(&self, history: &'h $crate::History, type_map: &'h mut $crate::reexports::type_reg::untagged::TypeMap<String>) {
                if let Some(h) = history.get_sub_history::<$crate::history::CopyHistory<$ty>>() {
                    type_map.insert(self.write_type_string(), h.clone());
                }
            }

            fn register(&self, type_reg: &mut $crate::reexports::type_reg::untagged::TypeReg<String>) {
                type_reg.register::<$crate::history::CopyHistory<$ty>>(self.write_type_string());
            }
            fn de<'h>(&self, history: &'h mut $crate::History, type_map: &'h mut $crate::reexports::type_reg::untagged::TypeMap<String>) {
                match type_map.remove(&self.write_type_string()) {
                    Some(sub) => {
                        let sub_history = sub.into_inner().downcast::<$crate::history::CopyHistory<$ty>>();
                        match sub_history {
                            Ok(downcasted) => {
                                history.insert_sub_history(*downcasted);
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

        impl<'h> $crate::Resource<'h> for $name {
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

            fn ser<'h>(&self, history: &'h $crate::History, type_map: &'h mut $crate::reexports::type_reg::untagged::TypeMap<String>) {
                if let Some(h) = history.get_sub_history::<$crate::history::DerefHistory<$ty>>() {
                    type_map.insert(self.write_type_string(), h.clone());
                }
            }

            fn register(&self, type_reg: &mut $crate::reexports::type_reg::untagged::TypeReg<String>) {
                type_reg.register::<$crate::history::DerefHistory<$ty>>(self.write_type_string());
            }
            fn de<'h>(&self, history: &'h mut $crate::History, type_map: &'h mut $crate::reexports::type_reg::untagged::TypeMap<String>) {
                match type_map.remove(&self.write_type_string()) {
                    Some(sub) => {
                        let sub_history = sub.into_inner().downcast::<$crate::history::DerefHistory<$ty>>();
                        match sub_history {
                            Ok(downcasted) => {
                                history.insert_sub_history(*downcasted);
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
