// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::collections::{HashMap, HashSet};
use std::default::Default;
use std::hash::{Hasher, Hash, BuildHasherDefault};
use std::ops::BitXor;
use std;
use std::mem::size_of;
use std::slice;

pub type FxHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FxHashSet<V> = HashSet<V, BuildHasherDefault<FxHasher>>;

#[allow(non_snake_case)]
pub fn FxHashMap<K: Hash + Eq, V>() -> FxHashMap<K, V> {
    HashMap::default()
}

#[allow(non_snake_case)]
pub fn FxHashSet<V: Hash + Eq>() -> FxHashSet<V> {
    HashSet::default()
}

/// A speedy hash algorithm for use within rustc. The hashmap in liballoc
/// by default uses SipHash which isn't quite as speedy as we want. In the
/// compiler we're not really worried about DOS attempts, so we use a fast
/// non-cryptographic hash.
///
/// This is the same as the algorithm used by Firefox -- which is a homespun
/// one not based on any widely-known algorithm -- though modified to produce
/// 64-bit hash values instead of 32-bit hash values. It consistently
/// out-performs an FNV-based hash within rustc itself -- the collision rate is
/// similar or slightly worse than FNV, but the speed of the hash function
/// itself is much higher because it works on up to 8 bytes at a time.
pub struct FxHasher {
    hash: usize
}

#[cfg(target_pointer_width = "32")]
const K: usize = 0x9e3779b9;
#[cfg(target_pointer_width = "64")]
const K: usize = 0x517cc1b727220a95;

impl Default for FxHasher {
    #[inline]
    fn default() -> FxHasher {
        FxHasher { hash: 0 }
    }
}

impl FxHasher {
    #[inline]
    fn add_to_hash(&mut self, i: usize) {
        self.hash = self.hash.rotate_left(5).bitxor(i).wrapping_mul(K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            let i = *byte;
            self.add_to_hash(i as usize);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as usize);
    }

    #[cfg(target_pointer_width = "32")]
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
        self.add_to_hash((i >> 32) as usize);
    }

    #[cfg(target_pointer_width = "64")]
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash as u64
    }
}

#[derive(Copy, Clone)]
pub struct FxHasher2 {
    hash: usize
}

impl Default for FxHasher2 {
    #[inline]
    fn default() -> FxHasher2 {
        FxHasher2 { hash: 0 }
    }
}

impl FxHasher2 {
    #[inline]
    fn add_to_hash(&mut self, i: usize) {
        self.hash = self.hash.rotate_left(5).bitxor(i).wrapping_mul(K);
    }

    pub fn write2(&mut self, mut bytes: &[u8]) {
        let split = bytes.len() & !7;
        let (first, rest) =  bytes.split_at(split);
        let first: &[usize] = unsafe { 
            std::slice::from_raw_parts(first.as_ptr() as *const usize, first.len() / 8)
        };
        for word in first {
            self.add_to_hash(*word);
        }
        for byte in rest {
            let i = *byte;
            self.add_to_hash(i as usize);
        }
    }

    pub fn write(&mut self, mut bytes: &[u8]) {
        unsafe {
            while bytes.len() >= size_of::<usize>() {
                self.add_to_hash(*(bytes.as_ptr() as *const usize));
                bytes = &bytes[size_of::<usize>()..];
            }
            #[cfg(target_pointer_width = "64")]
            {
                if bytes.len() >= 4 {
                    self.add_to_hash(*(bytes.as_ptr() as *const u32) as usize);
                    bytes = &bytes[4..];
                }
            }
            if bytes.len() >= 2 {
                self.add_to_hash(*(bytes.as_ptr() as *const u16) as usize);
                bytes = &bytes[2..];
            }
            if bytes.len() >= 1 {
                self.add_to_hash(bytes[0] as usize);
            }
        }
    }
}

impl Hasher for FxHasher2 {
    fn write(&mut self, mut bytes: &[u8]) {
        use byteorder::{ByteOrder, NativeEndian};

        #[cfg(target_pointer_width = "32")]
        let read_usize = |bytes| NativeEndian::read_u32(bytes);
        #[cfg(target_pointer_width = "64")]
        let read_usize = |bytes| NativeEndian::read_u64(bytes);

        /*let split = bytes.len() & !7;
        let (first, rest) =  bytes.split_at(split);
        let first: &[usize] = unsafe { 
            std::slice::from_raw_parts(first.as_ptr() as *const usize, first.len() / 8)
        };
        for word in first {
            self.add_to_hash(*word);
        }
        for byte in rest {
            let i = *byte;
            self.add_to_hash(i as usize);
        }*/
    
        let mut hash = *self;
        assert!(size_of::<usize>() <= 8);
        while bytes.len() >= size_of::<usize>() {
            hash.add_to_hash(read_usize(bytes) as usize);
            bytes = &bytes[size_of::<usize>()..];
        }
        if (size_of::<usize>() > 4) && (bytes.len() >= 4) {
            hash.add_to_hash(NativeEndian::read_u32(bytes) as usize);
            bytes = &bytes[4..];
        }
        if (size_of::<usize>() > 2) && bytes.len() >= 2 {
            hash.add_to_hash(NativeEndian::read_u16(bytes) as usize);
            bytes = &bytes[2..];
        }
        if (size_of::<usize>() > 1) && bytes.len() >= 1 {
            hash.add_to_hash(bytes[0] as usize);
        }
        *self = hash;
            /*while bytes.len() >= 1 {
                self.add_to_hash(bytes[0] as usize);
                bytes = &bytes[1..];
            }
            for &byte in bytes {
                self.add_to_hash(byte as usize);
            }*/
            /*#[cfg(target_pointer_width = "64")]
            {
                if bytes.len() >= 4 {
                    self.add_to_hash(*(bytes.as_ptr() as *const u32) as usize);
                    bytes = &bytes[4..];
                }
            }
            if bytes.len() >= 2 {
                self.add_to_hash(*(bytes.as_ptr() as *const u16) as usize);
                bytes = &bytes[2..];
            }
            if bytes.len() >= 1 {
                self.add_to_hash(bytes[0] as usize);
            }*/
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as usize);
    }

    #[cfg(target_pointer_width = "32")]
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
        self.add_to_hash((i >> 32) as usize);
    }

    #[cfg(target_pointer_width = "64")]
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash as u64
    }
}

pub struct DummyHasher {
    hash: usize
}

impl Default for DummyHasher {
    #[inline]
    fn default() -> DummyHasher {
        DummyHasher { hash: 0 }
    }
}

impl DummyHasher {
    #[inline]
    fn add_to_hash(&mut self, i: usize) {
        self.hash = self.hash.rotate_left(5).bitxor(i).wrapping_mul(K);
    }
}

pub fn hash_dummy(bytes: &[u8]) -> u64 {
    let mut d = DummyHasher::default();
    d.write(bytes);
    d.finish()
}

impl Hasher for DummyHasher {
    /*
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut bytes = bytes;
        let mut a = FxHasher2::default();
        let mut b = FxHasher2::default();
        while bytes.len() >= 16 {
            unsafe {
                a.add_to_hash(*(bytes.get_unchecked(0) as *const _ as *const usize));
                b.add_to_hash(*(bytes.get_unchecked(8) as *const _ as *const usize));
                bytes = slice::from_raw_parts(bytes.get_unchecked(16), bytes.len() - 16);
            }
        }
        self.hash = a.finish() as usize;
        self.add_to_hash(b.finish() as usize);
        for byte in bytes {
            let i = *byte;
            self.add_to_hash(i as usize);
        }
    }
*/
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        use std::arch::x86_64::*;
        let mut bytes = bytes;
        unsafe {
            let mut state = _mm_set1_epi8(0);
            let k = _mm_set1_epi64x(0x517cc1b727220a95);
            while bytes.len() >= 16 {
                let data = _mm_loadu_si128(bytes.get_unchecked(0) as *const _ as *const _);
                state = _mm_xor_si128(state, data);
                state = _mm_bslli_si128(state, 5);
                state = _mm_mullo_epi16(state, k);
                state = _mm_add_epi64(state, data);
                bytes = slice::from_raw_parts(bytes.get_unchecked(16), bytes.len() - 16);
            }
            state = _mm_add_epi64(_mm_unpackhi_epi64(state, state), state);
            self.hash = _mm_cvtsi128_si64(state) as usize;
        }
        for byte in bytes {
            let i = *byte;
            self.add_to_hash(i as usize);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as usize);
    }

    #[cfg(target_pointer_width = "32")]
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
        self.add_to_hash((i >> 32) as usize);
    }

    #[cfg(target_pointer_width = "64")]
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash as u64
    }
}

pub struct PlainHasher {
    hash: usize
}

impl Default for PlainHasher {
    #[inline]
    fn default() -> PlainHasher {
        PlainHasher { hash: 0 }
    }
}

impl PlainHasher {
    #[inline]
    fn add_to_hash(&mut self, i: usize) {
        self.hash += i;
    }
}

impl Hasher for PlainHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let split = bytes.len() & !7;
        let (first, rest) =  bytes.split_at(split);
        let first: &[usize] = unsafe { 
            std::slice::from_raw_parts(first.as_ptr() as *const usize, first.len() / 8)
        };
        for word in first {
            self.add_to_hash(*word);
        }
        for byte in rest {
            let i = *byte;
            self.add_to_hash(i as usize);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(i as usize);
    }

    #[cfg(target_pointer_width = "32")]
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
        self.add_to_hash((i >> 32) as usize);
    }

    #[cfg(target_pointer_width = "64")]
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i as usize);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash as u64
    }
}
