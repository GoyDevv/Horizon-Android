use crate::target::Targets;

#[derive(Debug)]
pub struct Modifiers<T> {
    entries: Vec<(Targets, i32, T)>,
}

impl<T> Default for Modifiers<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> Modifiers<T> {
    pub fn push(&mut self, targets: Targets, priority: i32, modifier: T) {
        self.entries.push((targets, priority, modifier));
    }

    pub fn resolve(&self, id: &str, in_tag: &dyn Fn(&str) -> bool) -> Option<&T> {
        let mut best: Option<&(Targets, i32, T)> = None;
        for entry in &self.entries {
            if entry.0.matches(id, in_tag) && best.is_none_or(|b| entry.1 > b.1) {
                best = Some(entry);
            }
        }
        best.map(|e| &e.2)
    }

    pub fn into_resolved(self, id: &str, in_tag: &dyn Fn(&str) -> bool) -> Option<T> {
        let mut best: Option<(i32, T)> = None;
        for (targets, priority, modifier) in self.entries {
            if targets.matches(id, in_tag) && best.as_ref().is_none_or(|b| priority > b.0) {
                best = Some((priority, modifier));
            }
        }
        best.map(|b| b.1)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|e| &e.2)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn targets(v: serde_json::Value) -> Targets {
        Targets::parse(Some(&v), None).unwrap()
    }

    #[test]
    fn the_highest_priority_modifier_that_targets_an_id_wins() {
        let mut set = Modifiers::default();
        set.push(targets(json!(".*")), 0, "everything");
        set.push(targets(json!("minecraft:stone")), 5, "stone");
        set.push(targets(json!("minecraft:stone")), -3, "stone, low");
        assert_eq!(set.resolve("minecraft:stone", &|_| false), Some(&"stone"));
        assert_eq!(
            set.resolve("minecraft:dirt", &|_| false),
            Some(&"everything")
        );
        assert_eq!(set.len(), 3);
        assert_eq!(set.iter().count(), 3);
    }

    #[test]
    fn on_a_tie_the_one_read_first_is_kept() {
        let mut set = Modifiers::default();
        set.push(targets(json!("minecraft:stone")), 1, "first");
        set.push(targets(json!("minecraft:stone")), 1, "second");
        assert_eq!(set.resolve("minecraft:stone", &|_| false), Some(&"first"));
    }

    #[test]
    fn tags_are_answered_by_the_caller_and_misses_resolve_to_nothing() {
        let mut set = Modifiers::default();
        set.push(targets(json!("#minecraft:leaves")), 0, "leaves");
        assert_eq!(
            set.resolve("minecraft:oak_leaves", &|t| t == "minecraft:leaves"),
            Some(&"leaves")
        );
        assert_eq!(set.resolve("minecraft:oak_leaves", &|_| false), None);
        assert!(Modifiers::<u8>::default().is_empty());
    }
}
