// A wise man one said that you shouldn't need comments; just write self-explanatory code.
// That man never wrote a rust macro.

#[macro_export]
macro_rules! model {
    (
        $model_vis:vis struct $model:ident {
            $(
                $res:ident: $ty:ty $(= $def:expr)?
            ),*
        }
    ) => {
        $model_vis struct $model;

        $crate::reexports::swift_macros::extras_module! {
            #[allow(non_snake_case)]
            $model_vis mod $model {
                use super::*;

                impl $crate::Model for $model {
                    type History = History;
                    type OperationTimelines = OperationTimelines;
                }

                #[derive(Default)]
                pub struct History {
                    $(
                        pub(crate) $res: $crate::history::History<$ty>,
                    )*
                }

                pub struct OperationTimelines {
                    $(
                        pub(crate) $res: $crate::operation::OperationTimeline<super::$model, $crate::reexports::swift_macros::get_resource_type_tag!($res)>,
                    )*
                }

                impl Default for OperationTimelines {
                    fn default() -> Self {
                        OperationTimelines {
                            $(
                                $res: $crate::operation::OperationTimeline::init($crate::model!(@internal-default $ty $(= $def)?)),
                            )*
                        }
                    }
                }

                $(
                    $crate::reexports::swift_macros::generate_resource_type_tag! {
                        $res:$ty
                    }
                )*
            }
        }
    };

    (@internal-default $ty:ty) => {
        <$ty>::default()
    };

    (@internal-default $ty:ty = $def:expr) => {
        $def
    };
}

#[macro_export]
macro_rules! impl_activity {
    (
        for $act:ident in $model:ident {
            $(
                $when:expr => $do:tt
            )*
        }
    ) => {
        impl Activity for $act {
            type Model = $model;

            fn decompose(self, start: Duration) -> Vec<$crate::GroundedOperationBundle<$model>> {
                let duration = self.duration();
                let end = start + duration;

                vec![
                    $(($crate::reexports::swift_macros::identity!($when), Box::new($crate::reexports::swift_macros::operation!($act,$model => $do))),)*
                ]
            }
        }
    };
}
