/// Generates a struct along with a constructor and accessors, where each field
/// can choose how its getter returns: by reference (default), by copy, or by clone.
/// Every field also gets a `set_<field>` setter.
///
/// Example:
/// ```
/// use foundry_worker::getset;
///
/// getset! {
///     #[derive(Default)]
///     pub struct Player {
///         name: String => clone,
///         hp: i32 => copy,
///         inventory: Vec<String>, // no `=>` means default: by reference
///     }
/// }
///
/// let mut player = Player::new("BoBo".to_string(), 10, vec![]);
/// player.set_hp(7);
///
/// assert_eq!(player.get_name(), "BoBo");
/// assert_eq!(player.get_hp(), 7);
/// assert!(player.get_inventory().is_empty());
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
            /// Creates a new instance from every field, in declaration order
            #[allow(clippy::too_many_arguments)]
            pub fn new($($field_name: $field_type),*) -> Self {
                Self { $($field_name),* }
            }

            $(
                $crate::__getter_impl!($field_name, $field_type $(, $mode)?);

                $crate::paste::paste! {
                    pub fn [<set_ $field_name>](&mut self, $field_name: $field_type) {
                        self.$field_name = $field_name
                    }
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
        $crate::paste::paste! {
            pub fn [<get_ $field_name>](&self) -> &$field_type {
                &self.$field_name
            }
        }
    };
    // by value, Copy types
    ($field_name:ident, $field_type:ty, copy) => {
        $crate::paste::paste! {
            pub fn [<get_ $field_name>](&self) -> $field_type {
                self.$field_name
            }
        }
    };
    // by value, via Clone
    ($field_name:ident, $field_type:ty, clone) => {
        $crate::paste::paste! {
            pub fn [<get_ $field_name>](&self) -> $field_type {
                self.$field_name.clone()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    getset! {
        #[derive(Debug, Default, PartialEq)]
        pub struct Sample {
            by_ref: Vec<u32>,
            by_copy: i32 => copy,
            by_clone: String => clone,
        }
    }

    fn sample() -> Sample {
        Sample::new(vec![1, 2], 5, "hello".to_string())
    }

    #[test]
    fn new_assigns_fields_in_order() {
        let value = sample();

        assert_eq!(value.by_ref, vec![1, 2]);
        assert_eq!(value.by_copy, 5);
        assert_eq!(value.by_clone, "hello");
    }

    #[test]
    fn getters_follow_the_requested_mode() {
        let value = sample();

        let borrowed: &Vec<u32> = value.get_by_ref();
        let copied: i32 = value.get_by_copy();
        let cloned: String = value.get_by_clone();

        assert_eq!(borrowed, &vec![1, 2]);
        assert_eq!(copied, 5);
        assert_eq!(cloned, "hello");
    }

    #[test]
    fn setters_overwrite_fields() {
        let mut value = sample();

        value.set_by_ref(vec![9]);
        value.set_by_copy(-1);
        value.set_by_clone("bye".to_string());

        assert_eq!(value, Sample::new(vec![9], -1, "bye".to_string()));
    }

    #[test]
    fn struct_attributes_are_forwarded() {
        assert_eq!(Sample::default(), Sample::new(vec![], 0, String::new()));
    }

    #[test]
    fn cloned_getter_does_not_alias_the_field() {
        let mut value = sample();
        let before = value.get_by_clone();

        value.set_by_clone("changed".to_string());

        assert_eq!(before, "hello");
        assert_eq!(value.get_by_clone(), "changed");
    }
}
