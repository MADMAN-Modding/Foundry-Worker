/// Pushes `item` unless it's already present. Returns whether it was added.
pub fn add_unique<T: PartialEq>(list: &mut Vec<T>, item: T) -> bool {
    if list.contains(&item) {
        return false;
    }

    list.push(item);
    true
}

/// Removes every occurrence of `item`. Returns whether anything was removed.
pub fn remove_item<T: PartialEq>(list: &mut Vec<T>, item: &T) -> bool {
    let before = list.len();
    list.retain(|existing| existing != item);

    list.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_unique_appends_new_items() {
        let mut list = vec![1, 2];

        assert!(add_unique(&mut list, 3));
        assert_eq!(list, vec![1, 2, 3]);
    }

    #[test]
    fn add_unique_rejects_duplicates() {
        let mut list = vec![1, 2];

        assert!(!add_unique(&mut list, 2));
        assert_eq!(list, vec![1, 2]);
    }

    #[test]
    fn add_unique_works_on_empty_lists_and_other_types() {
        let mut list: Vec<String> = Vec::new();

        assert!(add_unique(&mut list, "a".to_string()));
        assert!(!add_unique(&mut list, "a".to_string()));
    }

    #[test]
    fn remove_item_removes_all_occurrences() {
        let mut list = vec![1, 2, 1, 3];

        assert!(remove_item(&mut list, &1));
        assert_eq!(list, vec![2, 3]);
    }

    #[test]
    fn remove_item_reports_missing_items() {
        let mut list = vec![1, 2];

        assert!(!remove_item(&mut list, &9));
        assert_eq!(list, vec![1, 2]);
    }
}
