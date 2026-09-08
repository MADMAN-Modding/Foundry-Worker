/// Generates a struct along with getter methods, where each field can choose
/// how it's returned: by reference (default), by copy, or by clone.
///
/// Example:
/// ```
/// setget! {
///     pub struct Player {
///         name: String => clone,
///         hp: i32 => copy,
///         inventory: Vec<String>, // no `=>` means default: by reference
///     }
/// }
/// ```
#[macro_export]
macro_rules! getset {
    (
        $(#[$struct_attr:meta])*
        $struct_visibility:vis struct $struct_name:ident {
            $(
                $(#[$field_attr:meta])*
                $field_visibility:vis $field_name:ident : $field_type:ty $(=> $mode:ident)?
            ),* $(,)?
        }
    ) => {
        $(#[$struct_attr])*
        $struct_visibility struct $struct_name {
            $(
                $(#[$field_attr])*
                $field_visibility $field_name: $field_type,
            )*
        }

        impl $struct_name {
            $(
                $crate::__getter_impl!($field_name, $field_type $(, $mode)?);

                paste::paste! {
                    pub fn [<set_ $field_name>](&mut self, $field_name: $field_type) {self.$field_name = $field_name}
                }
            )*
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __getter_impl {
    // default: by reference
    ($field_name:ident, $field_type:ty) => {
        paste::paste! {
            pub fn [<get_ $field_name>](&self) -> &$field_type {
                &self.$field_name
            }
        }
    };
    // by value, Copy types
    ($field_name:ident, $field_type:ty, copy) => {
        paste::paste! {
            pub fn [<get_ $field_name>](&self) -> $field_type {
                self.$field_name
            }
        }
    };
    // by value, via Clone
    ($field_name:ident, $field_type:ty, clone) => {
        paste::paste! {
            pub fn [<get_ $field_name>](&self) -> $field_type {
                self.$field_name.clone()
            }
        }
    };
}
