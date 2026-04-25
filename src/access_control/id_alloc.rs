use std::collections::HashSet;

const SYSTEM_ID_START: u32 = 1;
const SYSTEM_ID_END: u32 = 999;
const USER_ID_START: u32 = 1000;
const MAX_ID: u32 = 65534;

pub enum IdKind {
    System,
    User,
}

pub struct IdAllocator {
    used_ids: HashSet<u32>,
}

impl IdAllocator {
    pub fn new() -> Self {
        IdAllocator {
            used_ids: HashSet::new(),
        }
    }

    pub fn with_existing(ids: impl IntoIterator<Item = u32>) -> Self {
        IdAllocator {
            used_ids: ids.into_iter().collect(),
        }
    }

    pub fn allocate(&mut self, kind: IdKind) -> Result<u32, Box<dyn std::error::Error>> {
        let range = match kind {
            IdKind::System => SYSTEM_ID_START..=SYSTEM_ID_END,
            IdKind::User => USER_ID_START..=MAX_ID,
        };

        for id in range {
            if !self.used_ids.contains(&id) {
                self.used_ids.insert(id);
                return Ok(id);
            }
        }

        Err("no available IDs in range".into())
    }

    pub fn reserve(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        if self.used_ids.contains(&id) {
            return Err(format!("id {} is already in use", id).into());
        }
        self.used_ids.insert(id);
        Ok(())
    }

    pub fn release(&mut self, id: u32) {
        self.used_ids.remove(&id);
    }

    pub fn is_used(&self, id: u32) -> bool {
        self.used_ids.contains(&id)
    }
}
