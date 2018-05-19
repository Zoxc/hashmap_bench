
#[macro_use]
extern crate criterion;

#[macro_use]
extern crate lazy_static;

extern crate bench;

use bench::fx::FxHasher2;

use criterion::black_box;
use criterion::Bencher;
use std::mem::size_of_val;
use criterion::Criterion;

use std::collections::hash_map::RandomState;
use std::collections::hash_map;
use std::collections::HashSet;

use std::time::Duration;

use bench::HashMap;
use bench::map::Map;
use std::fs;
use std::hash::Hash;
use std::hash::Hasher;
use std::hash::BuildHasher;
use std::hash::BuildHasherDefault;

fn new_drop(b: &mut Bencher) {

    b.iter(|| {
        let m: HashMap<i64, i64> = HashMap::default();
        assert_eq!(m.len(), 0);
    })
}

fn new_insert_drop(b: &mut Bencher) {

    b.iter(|| {
        let mut m = HashMap::default();
        m.insert(0i64, 0i64);
        assert_eq!(m.len(), 1);
    })
}

fn find_existing_map(b: &mut Bencher) {

    let mut m = Map::<u64, u64, BuildHasherDefault<bench::fx::FxHasher>>::new();

    for i in 1..100100u64 {
        m.insert(i, i);
    }

    b.iter(|| {
        let mut r = 0;
        for i in 1..100100u64 {
            r += bench::hmt2(&m, i);
        }
        r
        /*let mut r = true;
        for i in 1..100100u64 {
            r = r & m.contains_key(&i);
        }
        r*/
    });
}

fn find_existing(b: &mut Bencher) {
    let mut m = bench::fx::FxHashMap::default();

    for i in 1..1001000u64 {
        m.insert(i, i);
    }

    b.iter(|| {
        let mut r = 0;
        for i in 1..100100u64 {
            r += bench::hmt(&m, i);
        }
        r
    });
}

fn find_nonexisting_map(b: &mut Bencher) {
    let mut m = Map::<u64, u64, BuildHasherDefault<bench::fx::FxHasher>>::new();

    for i in 1..100100u64 {
        m.insert(i, i);
    }

    b.iter(|| {
        for i in 100100..200100 {
            m.contains_key(&i);
        }
    });
}

fn find_nonexisting(b: &mut Bencher) {

    let mut m = bench::fx::FxHashMap::default();

    for i in 1..100100u64 {
        m.insert(i, i);
    }

    b.iter(|| {
        for i in 100100..200100 {
            m.contains_key(&i);
        }
    });
}

lazy_static! {
    static ref SYMBOLS: (Vec<String>, Vec<&'static str>) = { 
        let l = fs::read_to_string("symbols.txt").unwrap();
        let l = l.lines();

        let s: Vec<String> = l.filter_map(|l| if l.starts_with("INTERN:") { Some(l[7..].to_string()) } else { None } ).collect();
            
        let strs: Vec<&'static str> = s.iter().map(|s| unsafe {
            &*(&**s as *const str)
        }).collect();
        (s, strs)
    };
    static ref SYMBOLS2: (Vec<String>, Vec<&'static str>) = {
        let s: Vec<String> = SYMBOLS.0.clone();
        let strs: Vec<&'static str> = s.iter().map(|s| unsafe {
            &*(&**s as *const str)
        }).collect();
        (s, strs)
    };
}

fn syntax_syntex_hit_rate() {
    fn intern(map: &mut HashMap<&'static str, u32>, hits: &mut usize, string: &'static str) -> u32 {
        if let Some(&name) = map.get(string) {
            *hits += 1;
            return name;
        }
        let name = map.len() as u32;
        map.insert(string, name);
        name
    }

    let strs = &SYMBOLS.1;

    let mut hits = 0;
    let mut m = HashMap::default();
    for s in strs {
        intern(&mut m, &mut hits, *s);
    }
    let mut large_8 = 0;
    let mut large_16 = 0;
    let mut large_32 = 0;
    let mut large_64 = 0;
    for (&k, _) in m.iter() {
        if k.len() > 8 {
            large_8 += 1;
        }
        if k.len() > 16 {
            large_16 += 1;
        }
        if k.len() > 32 {
            large_32 += 1;
        }
        if k.len() > 64 {
            large_64 += 1;
        }
    }
    let mut hash_collisions = HashMap::default();
    for (&k, _) in m.iter() {
        let mut hasher = bench::fx::FxHasher::default();
        k.hash(&mut hasher);
        //let h = hasher.finish() % (((m.len() as f64) * (1.0f64 / 0.8f64)) as u64);
        let h = hasher.finish();
        let h = h & 0xFFFFFF;
        //let h = (h ^ (h >> 32)) as u32;
        //let h = (h ^ (h >> 16)) as u16 as u64;
        *hash_collisions.entry(h).or_insert(0) += 1;
    }
    let mut hcd = HashMap::default();
    for (_, &v) in hash_collisions.iter() {
        *hcd.entry(v).or_insert(0) += v;
    }
    let mut chains = 0;
    let mut e: Vec<(usize, usize)> = hcd.into_iter().collect();
    e.sort_by_key(|e| e.0);
    for &(k, v) in e.iter() {
        chains += k * v;
        println!("collisions group {}: {}", k ,v );
    }
    println!("chains {} {}", chains, (chains as f64) / (m.len() as f64));
    println!("hits {} of {}", hits, strs.len());
    println!("large (>8 bytes) keys: {} of {}", large_8, m.len());
    println!("large (>16 bytes) keys: {} of {}", large_16, m.len());
    println!("large (>32 bytes) keys: {} of {}", large_32, m.len());
    println!("large (>64 bytes) keys: {} of {}", large_64, m.len());
}

fn syntax_syntex_symbols_str(b: &mut Bencher) {
    fn intern(map: &mut HashMap<&'static str, ()>, string: &'static str) {
        if let Some(&name) = map.get(string) {
            return name;
        }
        map.insert(string, ());
    }

    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = HashMap::default();
        for s in strs {
            intern(&mut m, *s);
        }
    });
}

fn syntax_syntex_symbols(b: &mut Bencher) {
    fn intern(map: &mut HashMap<&'static str, u32>, string: &'static str) -> u32 {
        if let Some(&name) = map.get(string) {
            return name;
        }
        let name = map.len() as u32;
        map.insert(string, name);
        name
    }

    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = HashMap::default();
        for s in strs {
            intern(&mut m, *s);
        }
    });
}

fn syntax_syntex_symbols_def(b: &mut Bencher) {
    fn intern(map: &mut hash_map::HashMap<&'static str, u32>, string: &'static str) -> u32 {
        if let Some(&name) = map.get(string) {
            return name;
        }
        let name = map.len() as u32;
        map.insert(string, name);
        name
    }

    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = hash_map::HashMap::default();
        for s in strs {
            intern(&mut m, *s);
        }
    });
}

fn syntax_syntex_hash_symbols_fx(b: &mut Bencher) {
    let strs = &SYMBOLS.1;
    let mut hasher = bench::fx::FxHasher::default();

    b.iter(|| {
        for s in strs {
            (**s).hash(&mut hasher)
        }
    });
}

fn syntax_syntex_hash_symbols_fx2(b: &mut Bencher) {
    let strs = &SYMBOLS.1;
    let mut hasher = bench::fx::FxHasher2::default();

    b.iter(|| {
        for s in strs {
            (**s).hash(&mut hasher)
        }
    });
}

fn syntax_syntex_hash_symbols_dummy(b: &mut Bencher) {
    let strs = &SYMBOLS.1;
    let mut hasher = bench::fx::DummyHasher::default();

    b.iter(|| {
        for s in strs {
            (**s).hash(&mut hasher)
        }
    });
}

fn syntax_syntex_hash_symbols_plain(b: &mut Bencher) {
    let strs = &SYMBOLS.1;
    let mut hasher = bench::fx::PlainHasher::default();

    b.iter(|| {
        for s in strs {
            (**s).hash(&mut hasher)
        }
    });
}

fn str_dummy(b: &mut Bencher) {
    let mut hasher = bench::fx::DummyHasher::default();
    let str = "i";

    b.iter(|| {
        str.hash(&mut hasher);
    });
}

fn str_fx2(b: &mut Bencher) {
    let mut hasher = bench::fx::FxHasher2::default();
    let str = "i";

    b.iter(|| {
        str.hash(&mut hasher);
    });
}

fn streq(b: &mut Bencher) {
    let strs_a = &SYMBOLS.1;
    let strs_b = &SYMBOLS2.1;
    b.iter(|| {
        for (a, b) in strs_a.iter().zip(strs_b.iter()) {
            //println!("{} {:x} {} {:x}", a, a.as_ptr() as usize, b, b.as_ptr() as usize);
            //assert!(a.as_ptr() != b.as_ptr());
            bench::streq_n(a, b);
        }
    });
}

fn streq_s(b: &mut Bencher) {
    let strs_a = &SYMBOLS.1;
    let strs_b = &SYMBOLS2.1;
    b.iter(|| {
        for (a, b) in strs_a.iter().zip(strs_b.iter()) {
            //println!("{} {:x} {} {:x}", a, a.as_ptr() as usize, b, b.as_ptr() as usize);
            //assert!(a.as_ptr() != b.as_ptr());
            bench::streq_s(a, b);
        }
    });
}

fn streq_true(b: &mut Bencher) {
    let strs_a = &SYMBOLS.1;
    let strs_b = &SYMBOLS2.1;
    b.iter(|| {
        for (a, b) in strs_a.iter().zip(strs_b.iter()) {
            bench::streq_true(a, b);
        }
    });
}

fn syntax_syntex_hash_symbols_def(b: &mut Bencher) {
    let strs = &SYMBOLS.1;
    let mut hasher = RandomState::new().build_hasher();

    b.iter(|| {
        for s in strs {
            (**s).hash(&mut hasher)
        }
    });
}

fn symbols_indirect_cap(b: &mut Bencher) {
    fn intern(map: &mut HashSet<&'static &'static str, BuildHasherDefault<FxHasher2>>, string: &'static &'static str) -> &'static &'static str {
        if let Some(&name) = map.get(string) {
            return name;
        }
        map.insert(string);
        string
    }

    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = HashSet::with_capacity_and_hasher(strs.len(), Default::default());
        for s in strs {
            intern(&mut m, s);
        }
    });
}

fn symbols_test() {
    fn intern(
        map1: &mut HashSet<&'static &'static str, BuildHasherDefault<FxHasher2>>,
        map2: &mut bench::Set<&'static &'static str, BuildHasherDefault<FxHasher2>>,
    string: &'static &'static str) -> &'static &'static str {
        //eprintln!("getting {}", string);
        //eprintln!("getting test {:?}", map2.get(&"extern"));
        assert_eq!(map1.get(&"extern"), map2.get(&"extern"));
        assert_eq!(map1.get(string), map2.get(string));
        if let Some(&name) = map1.get(string) {
            return name;
        }
        map1.insert(string);
        map2.insert(string);
        //eprintln!("inserting {}", string);
        assert!(map1.get(string) == map2.get(string));
        //eprintln!("getting test after insert {:?}", map2.get(&"extern"));
        assert_eq!(map1.get(&"extern"), map2.get(&"extern"));
        string
    }

    let strs = &SYMBOLS.1;

    let mut m1 = bench::Set::new();
    let mut m2 = HashSet::default();
    for s in strs {
        intern(&mut m2, &mut m1, s);
    }
}

fn symbols_indirect(b: &mut Bencher) {
    fn intern(map: &mut HashSet<&'static &'static str, BuildHasherDefault<FxHasher2>>, string: &'static &'static str) -> &'static &'static str {
        if let Some(&name) = map.get(string) {
            return name;
        }
        map.insert(string);
        string
    }

    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = HashSet::default();
        for s in strs {
            intern(&mut m, s);
        }
    });
}

fn symbols_indirect_simple(b: &mut Bencher) {
    fn intern(map: &mut HashSet<&'static &'static str, BuildHasherDefault<bench::fx::DummyHasher>>, string: &'static &'static str) -> &'static &'static str {
        if let Some(&name) = map.get(string) {
            return name;
        }
        map.insert(string);
        string
    }

    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = HashSet::default();
        for s in strs {
            intern(&mut m, s);
        }
    });
}

fn symbols_indirect_set_cap(b: &mut Bencher) {
    fn intern(map: &mut bench::Set<&'static &'static str, BuildHasherDefault<FxHasher2>>, string: &'static &'static str) -> &'static &'static str {
        if let Some(&name) = map.get(string) {
            return name;
        }
        map.insert(string);
        string
    }

    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = bench::Set::with_capacity(strs.len());
        for s in strs {
            intern(&mut m, s);
        }
    });
}

fn symbols_indirect_set(b: &mut Bencher) {
    fn intern(map: &mut bench::Set<&'static &'static str, BuildHasherDefault<FxHasher2>>, string: &'static &'static str) -> &'static &'static str {
        if let Some(&name) = map.get(string) {
            return name;
        }
        map.insert(string);
        string
    }

    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = bench::Set::new();
        for s in strs {
            intern(&mut m, s);
        }
    });
}

fn symbols_indirect_set_intern_simple(b: &mut Bencher) {
    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = bench::Set::<&'static &'static str, BuildHasherDefault<bench::fx::DummyHasher>>::new();
        for s in strs {
            m.intern(s);
        }
    });
}

#[derive(Hash, Copy, Clone, Debug)]
struct StrCmp(&'static &'static str);

impl PartialEq for StrCmp {
    fn eq(&self, other: &StrCmp) -> bool {
        unsafe { bench::streq_sr(*self.0, *other.0) }
    }
}

impl Eq for StrCmp {}

fn symbols_indirect_set_intern_strcmp(b: &mut Bencher) {
    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = bench::Set::<StrCmp, BuildHasherDefault<FxHasher2>>::new();
        for s in strs {
            m.intern(StrCmp(s));
        }
    });
}

fn symbols_indirect_set_intern(b: &mut Bencher) {
    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = bench::Set::<&'static &'static str, BuildHasherDefault<FxHasher2>>::new();
        for s in strs {
            m.intern(s);
        }
    });
}

fn symbols_indirect_set_intern_cap(b: &mut Bencher) {
    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = bench::Set::<&'static &'static str, BuildHasherDefault<FxHasher2>>::with_capacity(strs.len());
        for s in strs {
            m.intern(s);
        }
    });
}

fn set_test() {
    let mut set = bench::Set::<&'static &'static str>::new();
    //set.insert(&"hello");
    //set.insert(&"hi");
    set.insert(&"abc");
    set.intern(&"abc");
    set.insert(&"abc");
    println!("set {}", set.len());
}

fn criterion_benchmark(c: &mut Criterion) {
    /*
    bench::streq_n("a", "b");
    bench::streq_true("a", "b");
    bench::streq_s("a", "b");
     bench::fx::hash_dummy(&[]);
    c.bench_function("streq", streq);
    c.bench_function("streq_s", streq_s);
    //c.bench_function("streq_true", streq_true);
    /*let mut m = bench::Set::<&'static &'static str, BuildHasherDefault<FxHasher2>>::new();
    bench::intern_str(&mut m, &"h");
     set_test();
   // c.bench_function("new_drop", new_drop);
   // c.bench_function("new_insert_drop", new_insert_drop);
    //c.bench_function("grow_by_insertion", grow_by_insertion);
    syntax_syntex_hit_rate();
    symbols_test();
    c.bench_function("symbols_indirect", symbols_indirect);
    c.bench_function("symbols_indirect_set", symbols_indirect_set);*/
    /*c.bench_function("syntax_syntex_hash_symbols_plain", syntax_syntex_hash_symbols_plain);
    c.bench_function("syntax_syntex_hash_symbols_dummy", syntax_syntex_hash_symbols_dummy);
    c.bench_function("syntax_syntex_hash_symbols_fx2", syntax_syntex_hash_symbols_fx2);
    c.bench_function("str_dummy", str_dummy);
    c.bench_function("str_fx2", str_fx2);*/

    //c.bench_function("symbols_indirect", symbols_indirect);
    //c.bench_function("symbols_indirect_simple", symbols_indirect_simple);
    c.bench_function("symbols_indirect_set_intern_strcmp", symbols_indirect_set_intern_strcmp);
    c.bench_function("symbols_indirect_set_intern", symbols_indirect_set_intern);
    //c.bench_function("symbols_indirect_set_intern_simple", symbols_indirect_set_intern_simple);
    /*c.bench_function("symbols_indirect_cap", symbols_indirect_cap);
    c.bench_function("symbols_indirect_set_cap", symbols_indirect_set_cap);
    c.bench_function("symbols_indirect_set_intern_cap", symbols_indirect_set_intern_cap);*/
    /*c.bench_function("syntax_syntex_hash_symbols_fx", syntax_syntex_hash_symbols_fx);
    c.bench_function("syntax_syntex_hash_symbols_def", syntax_syntex_hash_symbols_def);
    c.bench_function("syntax_syntex_symbols", syntax_syntex_symbols);
    c.bench_function("syntax_syntex_symbols_str", syntax_syntex_symbols_str);
    c.bench_function("syntax_syntex_symbols_def", syntax_syntex_symbols_def);
    */
    */
    c.bench_function("find_existing", find_existing);
    c.bench_function("find_existing_map", find_existing_map);
    c.bench_function("find_nonexisting", find_nonexisting);
    
    c.bench_function("find_nonexisting_map", find_nonexisting_map);
}
/*pub fn benches() {
    let mut criterion: Criterion = Criterion::default()
        .configure_from_args();
    $(
        $target(&mut criterion);
    )+
}*/
criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(5)
        .warm_up_time(Duration::new(1, 0))
        .measurement_time(Duration::new(1, 0));
    targets = criterion_benchmark);
criterion_main!(benches);
/*
fn main() {
    criterion::init_logging();
    benches();
    criterion::Criterion::default()
        .configure_from_args()
        .sample_size(5)
        .final_summary();
}*/