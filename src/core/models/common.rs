use uuid::Uuid;
use std::collections::HashMap;

pub trait NameMatches {
    fn name_matches(&self, name: &str) -> bool;
}

pub fn get_id_by_name<T: NameMatches>(map: &HashMap<Uuid, T>, name: &str) -> Option<Uuid> {
    map.iter()
        .find(|(_, item)| item.name_matches(name))
        .map(|(id, _)| *id)
}