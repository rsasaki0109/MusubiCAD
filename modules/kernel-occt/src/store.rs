use std::collections::HashMap;

#[cfg(feature = "occt")]
use cadrum::Edge;

#[cfg(feature = "occt")]
use cadrum::Solid;

/// Internal storage for opaque kernel handles.
#[derive(Default)]
pub struct KernelStore {
    next_id: u64,
    #[cfg(feature = "occt")]
    pub face_ref_tags: HashMap<u64, HashMap<u64, String>>,
    #[cfg(feature = "occt")]
    pub wires: HashMap<u64, Vec<Edge>>,
    #[cfg(feature = "occt")]
    pub bodies: HashMap<u64, Solid>,
    #[cfg(feature = "occt")]
    pub compound_members: HashMap<u64, Vec<u64>>,
}

impl KernelStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    #[cfg(feature = "occt")]
    pub fn insert_wire(&mut self, edges: Vec<Edge>) -> u64 {
        let id = self.alloc_id();
        self.wires.insert(id, edges);
        id
    }

    #[cfg(feature = "occt")]
    pub fn insert_body(&mut self, solid: Solid) -> u64 {
        let id = self.alloc_id();
        self.bodies.insert(id, solid);
        id
    }

    #[cfg(feature = "occt")]
    pub fn insert_compound(&mut self, member_ids: Vec<u64>) -> u64 {
        let id = self.alloc_id();
        self.compound_members.insert(id, member_ids);
        id
    }

    #[cfg(feature = "occt")]
    pub fn compound_member_ids(&self, id: u64) -> Option<&[u64]> {
        self.compound_members.get(&id).map(Vec::as_slice)
    }

    #[cfg(feature = "occt")]
    pub fn wire(&self, id: u64) -> Option<&[Edge]> {
        self.wires.get(&id).map(Vec::as_slice)
    }

    #[cfg(feature = "occt")]
    pub fn body(&self, id: u64) -> Option<&Solid> {
        self.bodies.get(&id)
    }

    #[cfg(feature = "occt")]
    pub fn tag_face_ref(&mut self, body_id: u64, kernel_face_id: u64, ref_id: String) {
        self.face_ref_tags
            .entry(body_id)
            .or_default()
            .insert(kernel_face_id, ref_id);
    }

    #[cfg(feature = "occt")]
    pub fn face_ref_tags(&self, body_id: u64) -> Option<&HashMap<u64, String>> {
        self.face_ref_tags.get(&body_id)
    }
}
