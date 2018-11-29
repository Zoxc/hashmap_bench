
use std::marker::PhantomData;
use std::hash::Hash;
use std::hash::Hasher;
use std::hash::BuildHasher;
use std::mem::{size_of, align_of};
use std::ptr::{Unique, NonNull};
use std::alloc::{Global, Alloc};
use std::collections::hash_map::RandomState;
use std::borrow::Borrow;
use std;

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

// Make Hashtable generic over the Group, so we can have one Group for 32-bit keys, 64-bit values etc.

// Group has a u32 unused. Store metadata there?
// Store a bool if the group is full, so we don't need to find that out
#[repr(align(64), C)]
pub struct Group {
    keys: [u64; ENTRIES_PER_GROUP],
    values: [u64; ENTRIES_PER_GROUP],
}

impl Group {
    #[inline(always)]
    fn search_for_empty(&self, sentinel: u64) -> Option<usize> {
        for i in 0..ENTRIES_PER_GROUP {
            if unsafe { *self.keys.get_unchecked(i) == sentinel } {
                return Some(i)
            }
        }
        None
    }
/*
    #[inline(always)]
    fn search_with(&self, key: u64, sentinel: u64) -> Option<(usize, bool)> {
        // This unrolls
        for i in 0..ENTRIES_PER_GROUP {
            let k = unsafe { *self.keys.get_unchecked(i) };
            if k == key {
                return Some((i, false))
            }
        }
        self.search_for_empty(sentinel).map(|i| (i, true))
    }
*/
    #[inline(always)]
    fn search_with(&self, key: u64, sentinel: u64) -> Option<(usize, bool)> {
        use std::arch::x86_64::*;
        unsafe {
        let keys = _mm256_load_si256(&self.keys as *const _ as *const _);
        let key = _mm256_set1_epi64x(key as i64);
        let eq =  _mm256_cmpeq_epi64(keys, key);
        let mask = _mm256_movemask_epi8(eq) as u32;
        let idx = std::intrinsics::cttz(mask) as usize;
        if idx != 32 {
            Some((idx >> 3, true))
        } else {
            let sentinel = _mm256_set1_epi64x(sentinel as i64);
            let eq =  _mm256_cmpeq_epi64(keys, sentinel);
            let mask = _mm256_movemask_epi8(eq) as u32;
            let idx = std::intrinsics::cttz(mask) as usize;
            if idx != 32 {
                Some((idx >> 3, false))
            } else {
                None
            }
        }
        }
    }

    #[inline(always)]
    fn set(&mut self, pos: usize, key: u64, value: u64) {
        unsafe {
            *self.keys.get_unchecked_mut(pos) = key;
            *self.values.get_unchecked_mut(pos) = value;
        }
    }

    #[inline(always)]
    fn iter<F: FnMut(u64, u64)>(&self, sentinel: u64, f: &mut F) {
        for i in 0..ENTRIES_PER_GROUP {
            unsafe {
                let k = *self.keys.get_unchecked(i);
                if k != sentinel {
                    f(k, *self.values.get_unchecked(i));
                }
            }
        }
    }
}

pub struct Table {
    group_mask: usize,
    size: usize,
    capacity: usize,
    groups: Unique<Group>,
}

#[derive(Debug)]
pub struct RawEntry {
    group: *mut Group,
    pos: usize,
    empty: bool
}

impl Table {
    /// Does not initialize the buckets. The caller should ensure they,
    /// at the very least, set every hash to EMPTY_BUCKET.
    /// Returns an error if it cannot allocate or capacity overflows.
    unsafe fn new_uninitialized(group_count: usize, sentinel: u64) -> Table {
        assert!(size_of::<Group>() == 64);
        let groups: NonNull<Group> = Global.alloc_array(group_count).unwrap();
        let capacity2 = group_count * ENTRIES_PER_GROUP;
        let capacity1 = capacity2 - 1;
        //let capacity = (capacity1 * 10 + 10 - 1) / 11;
        let capacity = (capacity1 * 10 + 10 - 1) / 13;
        //println!("capacity1 {} capacity {}", capacity1, capacity);
        assert!(capacity < capacity2);

        for i in 0..group_count {
            let group = unsafe {
                &mut (*groups.as_ptr().offset(i as isize))
            };
            group.keys = [sentinel; ENTRIES_PER_GROUP];
        }

        Table {
            group_mask: group_count.wrapping_sub(1),
            size: 0,
            capacity,
            groups: Unique::new_unchecked(groups.as_ptr()),
        }
    }

    fn search_for_empty(&self, hash: u64, sentinel: u64) -> RawEntry {
        let group_idx = hash as usize;
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
            match unsafe { group.search_for_empty(sentinel) } {
                Some(pos) => return RawEntry {
                    group: group_ptr,
                    pos,
                    empty: true,
                },
                None => (),
            }
            group_idx = (group_idx + 1) & mask;
        }
    }

    fn search_with(&self, hash: u64, sentinel: u64) -> RawEntry {
        let group_idx = hash as usize;
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
            let r = unsafe { group.search_with(hash, sentinel) } ;
            //let r2 = unsafe { group.search_with2(&mut eq, hash as u32) } ;
            //assert_eq!(r, r2);
            //println!("search_with {}: {:?}", group_idx, r);
            match r {
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

    fn iter<F: FnMut(u64, u64)>(&self, sentinel: u64, mut f: F) {
        for i in 0..(self.group_mask + 1) {
            let group = unsafe {
                &(*self.groups.as_ptr().offset(i as isize))
            };
            group.iter(sentinel, &mut f);
        }
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        unsafe {
            Global.dealloc_array(
                NonNull::new_unchecked(self.groups.as_ptr()),
                self.group_mask + 1
            ).unwrap();
        }
    }
}

pub trait Sentinel {
    fn sentinel() -> Self;
}

impl Sentinel for u64 {
    fn sentinel() -> Self {
        -1i64 as u64
    }
}

pub struct Map<K: Eq + Hash + Copy + Sentinel, V, S: BuildHasher = RandomState> {
    hash_builder: S,
    table: Table,
    marker: PhantomData<(K, V)>,
}

impl<K: Eq + Hash + Copy + Sentinel, V, S: Default + BuildHasher> Map<K, V, S> {
    pub fn new() -> Self {
        assert!(size_of::<K>() == 8);
        assert!(align_of::<K>() == 8);
        assert!(size_of::<V>() == 8);
        assert!(align_of::<V>() == 8);
        Map {
            hash_builder: S::default(),
            table: unsafe { Table::new_uninitialized(2, Self::sentinel()) },
            marker: PhantomData,
        }
    }

    pub fn with_capacity(s: usize) -> Self {
        let groups = (s * ENTRIES_PER_GROUP + ENTRIES_PER_GROUP - 1) / ENTRIES_PER_GROUP;
        let groups = groups.checked_next_power_of_two().unwrap();
        assert!(size_of::<K>() == 8);
        assert!(align_of::<K>() == 8);
        assert!(size_of::<V>() == 8);
        assert!(align_of::<V>() == 8);
        Map {
            hash_builder: S::default(),
            table: unsafe { Table::new_uninitialized(groups, Self::sentinel()) },
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

impl<K: Eq + Hash + Copy + Sentinel, V, S: BuildHasher> Map<K, V, S> {
    fn sentinel() -> u64 {
        unsafe {
            *(&K::sentinel() as *const _ as *const u64)
        }
    }

    #[inline(never)]
    #[cold]
    fn expand(&mut self) {
        let mut new_table = unsafe {
            Table::new_uninitialized((self.table.group_mask + 1) << 1, Self::sentinel())
        };
        // Expand the table in place and move only the entries whose mask change
        // We need to move entries within a group in that case, might not be a win
        new_table.size = self.table.size;
        //println!("expanding to {}", (self.table.group_mask + 1) * ENTRIES_PER_GROUP);
        self.table.iter(Self::sentinel(), |k, v| {
            let key = &k as *const _ as *const K;
            //println!("moving {:?} with hash {}", unsafe { &*k }, h);
            let h = make_hash(&self.hash_builder, &key);
            let spot = new_table.search_for_empty(h, Self::sentinel());
            unsafe {
                (*spot.group).set(spot.pos, h, v);
            }
            /*let spot = new_table.search_with::<K, _>(|key| unsafe {key == &*k}, h as u64);
            if !spot.empty {
                println!("NO FOUND {:?} with hash {}", unsafe { &*k }, h);
            } else {
                println!("found {:?} with hash {} at {:?}", unsafe { &*k }, h, spot);
            }*/
        });
        self.table = new_table;
    }

    #[inline(always)]
    fn incr(&mut self) {
        if self.table.size + 1 > self.table.capacity {
            self.expand()
        }
    }

    pub fn len(&self) -> usize {
        self.table.size
    }

    #[inline(never)]
    pub fn insert(&mut self, k: K, v: V) {
        self.incr();
        assert!(k != K::sentinel());
        let hash = make_hash(&self.hash_builder, &k);
        let spot = self.table.search_with(hash, Self::sentinel());
        if spot.empty {
            self.table.size += 1;
        }
        //println!("inserting {:?} with hash {} at {:?}", unsafe { &k }, hash as u32, spot);
        unsafe {
            (*spot.group).set(spot.pos, *(&k as *const _ as *const u64), *(&v as *const _ as *const u64));
        }
    }

    pub fn contains_key(&self, k: &K) -> bool {
        self.get(k).is_some()
    }

    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&K>
        where K: Borrow<Q>,
              Q: Hash + Eq
    {
        let hash = make_hash(&self.hash_builder, value);
        let spot = self.table.search_with(hash, Self::sentinel());
        if spot.empty {
            None
        } else {
            unsafe {
                Some(&*((*spot.group).values.get_unchecked(spot.pos) as *const _ as *const K))
            }
        }
    }
}
