#![feature(alloc)]
#![feature(ptr_internals)]
#![feature(allocator_api)]

extern crate alloc;

use std::marker::PhantomData;
use std::hash::Hash;
use std::hash::Hasher;
use std::hash::BuildHasher;
use std::mem::{self, size_of, align_of};
use std::ptr::{self, Unique, NonNull};
use std::alloc::{Global, Alloc, Layout};
use std::collections::hash_map::RandomState;
use std::borrow::Borrow;
use std::slice;

pub mod fx;

pub type HashMap<K, V> = fx::FxHashMap<K, V>;

pub fn test(a: &str, b: &str) -> bool {
    a == b
}
/*
const ENTRIES_PER_GROUP: usize = 5;

// Group has a u32 unused. Store metadata there?
#[repr(align(64), C)]
pub struct Group {
    hashes: [u32; ENTRIES_PER_GROUP],
    padding: u32,
    values: [u64; ENTRIES_PER_GROUP],
}
*/
const ENTRIES_PER_GROUP: usize = 4;

// Group has a u32 unused. Store metadata there?
#[repr(align(64), C)]
pub struct Group {
    hashes: [u32; ENTRIES_PER_GROUP],
    padding1: u32,
    padding2: u32,
    padding3: u64,
    values: [u64; ENTRIES_PER_GROUP],
}

impl Group {
    fn search_with<K, F: FnMut(&K) -> bool>(&self, eq: &mut F, hash: u32) -> Option<(usize, bool)> {
        for (i, &h) in self.hashes[..].iter().enumerate() {
            //println!("checking entry {}: {} for {} val: {}", i, h, hash, self.values[i]);
            if h == hash && eq(unsafe { mem::transmute(self.values.get_unchecked(i)) }) {
                return Some((i, false))
            }
        }
        let empty = self.values[..].iter().position(|&v| v == 0);
        empty.map(|i| (i, true))
    }

    fn set(&mut self, pos: usize, hash: u32, value: u64) {
        self.hashes[pos] = hash;
        self.values[pos] = value;
    }

    fn iter<F: FnMut(u32, u64)>(&self, f: &mut F) {
        for (h, v) in self.hashes[..].iter().cloned().zip(self.values[..].iter().cloned()) {
            f(h, v)
        }
    }
}

pub struct Table {
    group_mask: usize,
    size: usize,
    capacity: usize,
    groups: Unique<Group>,
}

pub struct RawEntry {
    group: *mut Group,
    pos: usize,
    empty: bool
}

impl Table {
    /// Does not initialize the buckets. The caller should ensure they,
    /// at the very least, set every hash to EMPTY_BUCKET.
    /// Returns an error if it cannot allocate or capacity overflows.
    unsafe fn new_uninitialized(group_count: usize) -> Table {
        assert!(size_of::<Group>() == 64);
        let groups: NonNull<Group> = Global.alloc_array(group_count).unwrap();
        let capacity2 = group_count * ENTRIES_PER_GROUP;
        let capacity1 = capacity2 - 1;
        let capacity = (capacity1 * 10 + 10 - 1) / 11;
        //println!("capacity1 {} capacity {}", capacity1, capacity);
        assert!(capacity < capacity2);

        for i in 0..group_count {
            let group = unsafe {
                &mut (*groups.as_ptr().offset(i as isize))
            };
            group.values = [0; ENTRIES_PER_GROUP];
        }

        Table {
            group_mask: group_count.wrapping_sub(1),
            size: 0,
            capacity,
            groups: Unique::new_unchecked(groups.as_ptr()),
        }
    }

    fn search_with<K, F: FnMut(&K) -> bool>(&self, mut eq: F, hash: u64) -> RawEntry {
        //let group_idx = (hash >> 32) as usize;
        let group_idx = hash as u32 as usize;
        let mask = self.group_mask;
        let mut group_idx = group_idx & mask;

        loop {
            //println!("checking group {}", group_idx);
            let group_ptr = unsafe {
                self.groups.as_ptr().offset(group_idx as isize)
            };
            let group = unsafe {
                &(*group_ptr)
            };
            match group.search_with(&mut eq, hash as u32) {
                Some((pos, empty)) => return RawEntry {
                    group: group_ptr,
                    pos,
                    empty,
                },
                None => (),
            }
            group_idx = (group_idx + 1) & mask;
        }
    }

    fn iter<F: FnMut(u32, u64)>(&self, mut f: F) {
        for i in 0..(self.group_mask + 1) {
            let group = unsafe {
                &(*self.groups.as_ptr().offset(i as isize))
            };
            group.iter(&mut f);
        }
    }
}

pub struct Set<K: Eq + Hash, S = RandomState> {
    hash_builder: S,
    table: Table,
    marker: PhantomData<K>,
}

impl<K: Eq + Hash, S: Default> Set<K, S> {
    pub fn new() -> Self {
        assert!(size_of::<K>() == 8);
        Set {
            hash_builder: S::default(),
            table: unsafe { Table::new_uninitialized(2) },
            marker: PhantomData,
        }
    }
}

pub fn make_hash<T: ?Sized, S>(hash_state: &S, t: &T) -> u64
    where T: Hash,
          S: BuildHasher
{
    let mut state = hash_state.build_hasher();
    t.hash(&mut state);
    state.finish()
}

impl<K: Eq + Hash, S: BuildHasher> Set<K, S> {
    fn incr(&mut self) {
        if self.table.size + 1 > self.table.capacity {
            let mut new_table = unsafe {
                Table::new_uninitialized((self.table.group_mask + 1) << 1)
            };
            new_table.size = self.table.size;
            self.table.iter(|h, v| {
                let spot = new_table.search_with::<u64, _>(|_| false, h as u64);
                unsafe {
                    (*spot.group).set(spot.pos, h, v);
                }
            });
            self.table = new_table;
        }
    }

    pub fn insert(&mut self, k: K) {
        self.incr();
        let hash = make_hash(&self.hash_builder, &k);
        let spot = self.table.search_with::<K, _>(|key| key == &k, hash);
        if spot.empty {
            self.table.size += 1;
        }
        unsafe {
            (*spot.group).set(spot.pos, hash as u32, *(&k as *const _ as *const u64));
        }
    }

    pub fn intern(&mut self, k: K) -> &K {
        self.incr();
        let hash = make_hash(&self.hash_builder, &k);
        let spot = self.table.search_with::<K, _>(|key| key == &k, hash);
        unsafe {
            if spot.empty {
                self.table.size += 1;
                (*spot.group).set(spot.pos, hash as u32, *(&k as *const _ as *const u64));
            }
            &*((*spot.group).values.get_unchecked(spot.pos) as *const _ as *const K)
        }
    }

    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&K>
        where K: Borrow<Q>,
              Q: Hash + Eq
    {
        let hash = make_hash(&self.hash_builder, value);
        let spot = self.table.search_with::<K, _>(|k| value.eq(k.borrow()), hash);
        if spot.empty {
            None
        } else {
            unsafe {
                Some(&*((*spot.group).values.get_unchecked(spot.pos) as *const _ as *const K))
            }
        }
    }
}

#[test]
fn find_existing() {

    let mut m = HashMap::default();

    for i in 1..1001i64 {
        m.insert(i, i);
    }

    for i in 1..1001i64 {
        m.contains_key(&i);
    }
}

#[test]
fn find_nonexisting() {

    let mut m = HashMap::default();

    for i in 1..1001i64 {
        m.insert(i, i);
    }

    for i in 1001..2001 {
        m.contains_key(&i);
    }
}
