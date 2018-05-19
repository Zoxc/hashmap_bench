#![feature(alloc)]
#![feature(ptr_internals)]
#![feature(allocator_api)]
#![feature(core_intrinsics)]


extern crate alloc;
extern crate criterion;

#[cfg(test)]
#[macro_use]
extern crate quickcheck;

use std::hash::BuildHasherDefault;

pub mod set;
pub mod map;

pub use set::Set;

pub mod fx;

pub type HashMap<K, V> = fx::FxHashMap<K, V>;

pub fn hmt(m: &fx::FxHashMap<u64, u64>, i: u64) -> u64 {
    if let Some(&i) = m.get(&i) {
        i
    } else {
        0
    }
}

pub fn hmt2(m: &map::Map<u64, u64, BuildHasherDefault<fx::FxHasher>>, i: u64) -> u64 {
    if let Some(&i) = m.get(&i) {
        i
    } else {
        0
    }
}

//#[inline]
pub fn streq_sr(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    unsafe {
        let a = a.as_bytes();
        let b = b.as_bytes();
        for i in 0..a.len() {
            if *a.get_unchecked(0) != *b.get_unchecked(0) {
                return false;
            }
        }
    }
    true
}

/*
#[inline]
pub fn same_page(addr: usize, size: usize) -> bool {
    const PAGE_MASK: usize = !(0x1000 - 1);
    addr & PAGE_MASK == (addr + size - 1) & PAGE_MASK
}

const CHUNK_SIZE: usize = 16;

#[inline]
pub unsafe fn eq_chunk(a: *const u8, b: *const u8, offset: &mut usize, len: usize) -> Option<bool> {
     use std::arch::x86_64::*;
    debug_assert!(len > 0);
    let a = a.offset(*offset as isize);
    let b = b.offset(*offset as isize);
    if !same_page(a as usize, CHUNK_SIZE) || !same_page(b as usize, CHUNK_SIZE) {
        return None;
    }

    for i in 0..std::cmp::min(len, CHUNK_SIZE) {
        if *a.offset(i as isize) != *b.offset(i as isize) {
            return Some(false);
        }
    }
    *offset += CHUNK_SIZE;
    Some(true)
}

#[inline]
pub unsafe fn eq_chunk(a: *const u8, b: *const u8, offset: &mut usize, len: usize) -> Option<bool> {
     use std::arch::x86_64::*;
    debug_assert!(len > 0);
    let a = a.offset(*offset as isize);
    let b = b.offset(*offset as isize);
    if !same_page(a as usize, CHUNK_SIZE)/* || !same_page(b as usize, CHUNK_SIZE)*/ {
        return None;
    }
    let a = _mm_loadu_si128(a as *const _);
    let b = _mm_loadu_si128(b as *const _);
    let eq = _mm_cmpeq_epi8(a, b);
    let mask = _mm_movemask_epi8(eq) as u16;
    let mask_offset = if len > CHUNK_SIZE { 0 } else { CHUNK_SIZE - len };
    // FIXME: Use bit extract instructions here? BMI2
    // Could maybe do this in SIMD by using shuffle instructions?
    let mask = mask.wrapping_shl(mask_offset as u32);
    let mask = mask >> mask_offset;
    if mask == 0 {
        return Some(false);
    }
    *offset += CHUNK_SIZE;
    Some(true)
}

#[inline]
pub unsafe fn streq_sr(a: &str, b: &str) -> bool {
    unsafe {
        if a.len() != b.len() {
            return false;
        }
        let mut offset = 0;
        let mut len = a.len();
        let a = a.as_bytes().as_ptr();
        let b = b.as_bytes().as_ptr();
        if a == b || len == 0 {
            return true;
        }
        debug_assert!(b as usize & 0xF == 0);

        match eq_chunk(a, b, &mut offset, len) {
            Some(true) => {
                if len <= CHUNK_SIZE {
                    return true;
                }
                len -= CHUNK_SIZE;
                match eq_chunk(a, b, &mut offset, len) {
                    Some(true) => {
                        if len <= CHUNK_SIZE {
                            return true;
                        }
                        len -= CHUNK_SIZE;
                    }
                    Some(false) => return false,
                    None => (),
                }
            }
            Some(false) => return false,
            None => (),
        }
        slice::from_raw_parts(a.offset(offset as isize), len) ==
            slice::from_raw_parts(b.offset(offset as isize), len)
    }
}
*/
#[inline(never)]
pub fn streq_s(a: &str, b: &str) -> bool {
    unsafe {
        streq_sr(a, b)
    }
}

#[inline(never)]
pub fn streq_n(a: &str, b: &str) -> bool {
    a == b
}

#[inline(never)]
pub fn streq_true(a: &str, b: &str) -> bool {
    false
}

pub fn test(a: &str, b: &str) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;
  quickcheck! {
      fn prop1(a: String) -> bool {
          let b = a.clone();
          streq_s(&a, &b)
      }
      fn prop2(a: String, b: String) -> bool {
          streq_s(&a, &b) == (a == b)
      }
  }
}