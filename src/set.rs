use std::marker::PhantomData;
use std::hash::Hash;
use std::hash::Hasher;
use std::hash::BuildHasher;
use std::hash::BuildHasherDefault;
use std::mem::{self, size_of, align_of};
use std::ptr::{self, Unique, NonNull};
use std::alloc::{Global, Alloc, Layout};
use std::collections::hash_map::RandomState;
use std::borrow::Borrow;
use std::slice;
use std::fmt::Debug;
use fx;

/// A hash that is not zero, since we use a hash of zero to represent empty
/// buckets.
#[derive(PartialEq, Copy, Clone)]
pub struct SafeHash {
    hash: u32,
}

impl SafeHash {
    /// Peek at the hash value, which is guaranteed to be non-zero.
    #[inline(always)]
    pub fn inspect(&self) -> u32 {
        self.hash
    }

    #[inline(always)]
    pub fn new(hash: u32) -> Self {
        // We need to avoid 0 in order to prevent collisions with
        // EMPTY_HASH. We can maintain our precious uniform distribution
        // of initial indexes by unconditionally setting the MSB,
        // effectively reducing the hashes by one bit.
        //
        // Truncate hash to fit in `HashUint`.
        let hash_bits = size_of::<u32>() * 8;
        SafeHash { hash: (1 << (hash_bits - 1)) | hash }
    }
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
const ENTRIES_PER_GROUP: usize = 5;

// Make Hashtable generic over the Group, so we can have one Group for 32-bit keys, 64-bit values etc.

// Group has a u32 unused. Store metadata there?
// Store a bool if the group is full, so we don't need to find that out
#[repr(align(64), C)]
pub struct Group {
    hashes: [u32; ENTRIES_PER_GROUP],
    //padding1: u32,
    size: u32,
    //padding3: u64,
    values: [u64; ENTRIES_PER_GROUP],
}

impl Group {
    #[inline(always)]
    fn search_for_empty(&self) -> Option<usize> {
        if self.size != ENTRIES_PER_GROUP as u32 {
            Some(self.size as usize)
        } else {
            None
        }
        //self.values[..].iter().position(|&v| v == 0)
    }
/*
    #[inline(always)]
    #[target_feature(enable = "avx2")]
    fn search_for_empty(&self, hash: u32) -> Option<usize> {
        let values = _mm256_load_si256(&self.values as *const _ as *const _);
        let empty = _mm256_cmpeq_epi64(values, _mm256_set1_epi64x(0));
        if _mm256_testz_si256(empty) == 
        self.values[..].iter().position(|&v| v == 0)
    }
*/
/* This is based on self.size and doesn't unroll
    #[inline(always)]
    fn search_with<K, F: FnMut(&K) -> bool>(&self, eq: &mut F, hash: u32) -> Option<(usize, bool)> {
        for i in 0..(self.size as usize) {
            let h = unsafe { *self.hashes.get_unchecked(i) };
            if h == hash && eq(unsafe { mem::transmute(self.values.get_unchecked(i)) }) {
                return Some((i, false))
            }
        }
        self.search_for_empty().map(|i| (i, true))
    }
*/

    #[inline(always)]
    fn search_with<K, F: FnMut(&K) -> bool>(&self, eq: &mut F, hash: u32) -> Option<(usize, bool)> {
        // This unrolls
        for i in 0..ENTRIES_PER_GROUP {
            let h = unsafe { *self.hashes.get_unchecked(i) };
            if h == hash && eq(unsafe { mem::transmute(self.values.get_unchecked(i)) }) {
                return Some((i, false))
            }
        }
        self.search_for_empty().map(|i| (i, true))
    }
/*
    //#[inline(never)]
    //#[target_feature(enable = "sse4.1", enable = "avx2", enable = "bmi1")]
    #[inline(always)]
    unsafe fn search_with<K, F: FnMut(&K) -> bool>(&self, eq_test: &mut F, hash: u32) -> Option<(usize, bool)> {
        use std::arch::x86_64::*;
        unsafe {
            //println!("checking for hash {} hashes {:?} values {:?}", hash, self.hashes, self.values);
               /* 
            for (i, &h) in self.hashes[..].iter().enumerate() {
                println!("checking for hash {}: {} val: {}", i, h, self.values[i]);
            }*/

            let hashes = _mm_load_si128(&self.hashes as *const _ as *const _);
            let hash_s = _mm_set1_epi32(hash as i32);
            let eq = _mm_cmpeq_epi32(hash_s, hashes);
            ///println!("eq {:?}", eq);
            //let eq = _mm_packs_epi32(eq, eq);
            //println!("eq1 {:?}", eq);
            //let eq = _mm_cvtsi128_si64(eq);
            //let eq = _mm_packs_epi16(eq, eq);
            //println!("eq2 {:?}", eq);
            //let mut mask = (_mm_movemask_epi8(eq) & 0xF) as u8;
            let mut mask = _mm_movemask_epi8(eq) as u32;
            //println!("mask {} {}", mask, _mm_movemask_epi8(eq));
            let mut i = 0;

            for i in 0..4 {
                if (mask & (1 << (4 * i as u32)) != 0) && eq_test(unsafe { mem::transmute(self.values.get_unchecked(i)) }) {
                    return Some((i, false))
                }
            }

            let h = unsafe { *self.hashes.get_unchecked(4) };
            if h == hash && eq_test(unsafe { mem::transmute(self.values.get_unchecked(4)) }) {
                return Some((4, false))
            }
            /*
            loop {
                let skip = std::intrinsics::cttz(mask);
                if skip == 32 {
                    //println!("no hash matched");
                    break;
                }
                i += (skip >> 2) as usize;
                mask = mask >> (skip + 4);
                //println!("testing idx {} rem {} skip {}", i, mask, skip);
                if eq_test(unsafe { mem::transmute(self.values.get_unchecked(i)) }) {
                    //println!("found hash at {}", i);
                    return Some((i, false))
                }
                i += 1;
            }*/
            //println!("did not find hash");

        }
        self.search_for_empty().map(|i| (i, true))
    }
*/
    #[inline(always)]
    fn set(&mut self, pos: usize, hash: u32, value: u64) {
        unsafe {
            *self.hashes.get_unchecked_mut(pos) = hash;
            *self.values.get_unchecked_mut(pos) = value;
        }
    }

    #[inline(always)]
    fn iter<F: FnMut(u32, u64)>(&self, f: &mut F) {
        for i in 0..ENTRIES_PER_GROUP {
            unsafe {
                let h = *self.hashes.get_unchecked(i);
                if h != 0 {
                    f(h, *self.values.get_unchecked(i))
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
    unsafe fn new_uninitialized(group_count: usize) -> Table {
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
            group.hashes = [0; ENTRIES_PER_GROUP];
            group.size = 0;
        }

        Table {
            group_mask: group_count.wrapping_sub(1),
            size: 0,
            capacity,
            groups: Unique::new_unchecked(groups.as_ptr()),
        }
    }

    fn search_for_empty(&self, hash: u64) -> RawEntry {
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
            match unsafe { group.search_for_empty() } {
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
            let r = unsafe { group.search_with(&mut eq, hash as u32) } ;
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

    fn iter<F: FnMut(u32, u64)>(&self, mut f: F) {
        for i in 0..(self.group_mask + 1) {
            let group = unsafe {
                &(*self.groups.as_ptr().offset(i as isize))
            };
            group.iter(&mut f);
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

    pub fn with_capacity(s: usize) -> Self {
        let groups = (s * ENTRIES_PER_GROUP + ENTRIES_PER_GROUP - 1) / ENTRIES_PER_GROUP;
        let groups = groups.checked_next_power_of_two().unwrap();
        assert!(size_of::<K>() == 8);
        Set {
            hash_builder: S::default(),
            table: unsafe { Table::new_uninitialized(groups) },
            marker: PhantomData,
        }
    }
}

#[inline(never)]
pub fn make_hash<T: ?Sized, S>(hash_state: &S, t: &T) -> u64
    where T: Hash,
          S: BuildHasher
{
    let mut state = hash_state.build_hasher();
    t.hash(&mut state);
    SafeHash::new(state.finish() as u32).inspect() as u64
}

impl<K: Eq + Hash + Debug + Copy, S: BuildHasher> Set<K, S> {
    #[inline(never)]
    #[cold]
    fn expand(&mut self) {
        let mut new_table = unsafe {
            Table::new_uninitialized((self.table.group_mask + 1) << 1)
        };
        // Expand the table in place and move only the entries whose mask change
        // We need to move entries within a group in that case, might not be a win
        new_table.size = self.table.size;
        //println!("expanding to {}", (self.table.group_mask + 1) * ENTRIES_PER_GROUP);
        self.table.iter(|h, v| {
            let k = &v as *const _ as *const K;
            //println!("moving {:?} with hash {}", unsafe { &*k }, h);
            let spot = new_table.search_for_empty(h as u64);
            unsafe {
                (*spot.group).size += 1;
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
    pub fn insert(&mut self, k: K) {
        self.incr();
        let hash = make_hash(&self.hash_builder, &k);
        let spot = self.table.search_with::<K, _>(|key| key == &k, hash);
        if spot.empty {
            self.table.size += 1;
            unsafe {
                (*spot.group).size += 1;
            }
        }
        //println!("inserting {:?} with hash {} at {:?}", unsafe { &k }, hash as u32, spot);
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
                (*spot.group).size += 1;
                (*spot.group).set(spot.pos, hash as u32, *(&k as *const _ as *const u64));
            }
            &*((*spot.group).values.get_unchecked(spot.pos) as *const _ as *const K)
        }
    }

    #[inline(never)]
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

#[inline(never)]
pub fn intern_str(map: &mut Set<&'static &'static str, BuildHasherDefault<fx::FxHasher2>>, string: &'static &'static str) -> &'static &'static str {
    map.intern(string)
}
