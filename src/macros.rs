#![doc(hidden)]

// A wise man one said that you shouldn't need comments; just write self-explanatory code.
// That man never wrote a rust macro.

/// Used to implement the [Model][crate::Model] trait.
///
/// ## Usage
///
/// For now the macro only supports defining resource types. Just create
/// a struct definition with one field for each resource:
///
/// ```
/// # use swift::model;
/// # fn main() {}
/// model! {
///     pub struct MyModel {
///         my_int: u32,
///         my_string: String
///     }
/// }
/// ```
///
/// Each resource must implement [Resource][crate::Resource], which requires [Default].
/// Unless otherwise specified, the initial condition of each resource will be its
/// default. To change that, you can write:
///
/// ```
/// # use swift::model;
/// # fn main() {}
/// model! {
///     pub struct MyModel {
///         my_int: u32 = 5,
///         my_string: String = "Hello world!".to_string()
///     }
/// }
/// ```
///
/// This will generate a model `MyModel`, which you can use to create a [Session][crate::Session].
///
/// ## Caveats
///
/// The model type generated doesn't actually have the fields you specified. If you
/// want a struct like the one you wrote, the associated type `<MyModel as Model>::State`
/// is generated for you. It implements [Default], [Serialize][serde::Serialize], and [Deserialize][serde::Deserialize].
///
/// The macro needs to create a lot of types, so it generates a module named
/// `MyModel_extras_module` to avoid polluting the namespace. There's nothing in there
/// that you should need to use directly.
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

        #[doc(hidden)]
        $crate::reexports::swift_macros::extras_module! {
            #[allow(non_snake_case)]
            $model_vis mod $model {
                use super::*;
                use $crate::reexports::serde::{Serialize, Deserialize};

                impl $crate::Model for $model {
                    type History = History;
                    type OperationTimelines = OperationTimelines;
                    type State = State;
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

                #[derive(Serialize, Deserialize)]
                pub struct State {
                    $($res: $ty,)*
                }

                impl Default for State {
                    fn default() -> State {
                        State {
                            $(
                                $res: $crate::model!(@internal-default $ty $(= $def)?),
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

/// Used to implement an [Activity][crate::Activity] type.
///
/// ## Usage
///
/// First, create a struct for your activity that contains your activity
/// arguments as fields, and implements [Durative][crate::Durative]:
///
/// ```
/// # use swift::*;
/// # use serde::{Serialize, Deserialize};
/// #[derive(Serialize, Deserialize, Durative)]
/// #[duration = "Duration(5)"]
/// pub struct MyActivity {
///     pub my_argument: f32,
/// }
/// ```
///
/// Each of the argument types much implement [Resource][crate::Resource].
///
/// The duration can depend on the arguments; if this results in a large unreadable
/// expression, you can implement [Durative][crate::Durative] manually to format it better.
///
/// Next, call [impl_activity][crate::impl_activity]. It takes the following form:
///
/// ```
/// # use swift::*;
/// # use serde::{Serialize, Deserialize};
/// # fn main() {}
/// # model! {
/// #    pub struct MyModel {
/// #        my_int: u32,
/// #        my_string: String
/// #    }
/// # }
/// # #[derive(Serialize, Deserialize, Durative, Clone)]
/// # #[duration = "Duration(5)"]
/// # pub struct MyActivity {
/// #     pub my_argument: f32,
/// # }
/// #
/// impl_activity! {
///     for MyActivity in MyModel {
///         start => {
///             :my_string = ?my_int.to_string();
///         }
///
///         end - Duration(3) => {
///             ?:my_int *= 2;
///         }
///     }
/// }
/// ```
///
/// This is what is going on, step-by-step:
/// 1. `for MyActivity in MyModel` tells the macro what activity type and model its generating for.
/// 2. Next is a list of **operations**. These are the fundamental atoms of what Swift simulates.
/// 3. `start =>` and `end - Duration(3) =>` specify when their respective operations happen, relative
///    to the placement and duration of the activity. It can be any expression that evaluates to a [Duration][crate::Duration],
///    although it will be an error to give anything outside the declared bounds of the activity.
/// 4. Next is a code block containing arbitrary operation code. These do not execute in a shared
///    environment; any variables declared in one will not be accessible in another.
/// 5. Resources are accessed with `?` for reading and `:` for writing. (`:?` and `?:` indicate read-write operations.)
///    The macro scans your code for these symbols and generates the appropriate code to read and
///    write from the simulation.
/// 6. Activity arguments can be accessed through `args.my_argument`. Modification is not allowed.
///
/// Be careful not to use symbols to indicate operations that you don't do:
/// - Applying `?` to a value you don't read or `:` to a value you don't write to will slow down simulation.
/// - Reading from a value that you only declared as a write will *currently* return the type's default.
///   This will hopefully be a hard error in the future.
/// - Writing to a value that you only declared as a read will do nothing and lose your calculation.
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

            fn decompose(self, start: Duration) -> Vec<$crate::operation::GroundedOperationBundle<$model>> {
                let duration = self.duration();
                let end = start + duration;

                let _self_arc = std::sync::Arc::new(self);

                vec![
                    $(($crate::reexports::swift_macros::identity!($when), Box::new($crate::reexports::swift_macros::operation!($act,$model => $do))),)*
                ]
            }
        }
    };
}
