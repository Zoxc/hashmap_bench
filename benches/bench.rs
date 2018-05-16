
#[macro_use]
extern crate criterion;

#[macro_use]
extern crate lazy_static;

extern crate bench;

use bench::fx::FxHasher2;

use criterion::black_box;
use criterion::Bencher;

use criterion::Criterion;

use std::collections::hash_map::RandomState;
use std::collections::hash_map;
use std::collections::HashSet;

use std::time::Duration;

use bench::HashMap;
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

fn find_existing(b: &mut Bencher) {

    let mut m = HashMap::default();

    for i in 1..1001i64 {
        m.insert(i, i);
    }

    b.iter(|| {
        for i in 1..1001i64 {
            m.contains_key(&i);
        }
    });
}

fn find_nonexisting(b: &mut Bencher) {

    let mut m = HashMap::default();

    for i in 1..1001i64 {
        m.insert(i, i);
    }

    b.iter(|| {
        for i in 1001..2001 {
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

fn syntax_syntex_hash_symbols_def(b: &mut Bencher) {
    let strs = &SYMBOLS.1;
    let mut hasher = RandomState::new().build_hasher();

    b.iter(|| {
        for s in strs {
            (**s).hash(&mut hasher)
        }
    });
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

fn symbols_indirect_set_intern(b: &mut Bencher) {
    let strs = &SYMBOLS.1;

    b.iter(|| {
        let mut m = bench::Set::<&'static &'static str, BuildHasherDefault<FxHasher2>>::new();
        for s in strs {
            m.intern(s);
        }
    });
}

fn criterion_benchmark(c: &mut Criterion) {
   // c.bench_function("new_drop", new_drop);
   // c.bench_function("new_insert_drop", new_insert_drop);
    //c.bench_function("grow_by_insertion", grow_by_insertion);
    syntax_syntex_hit_rate();
    c.bench_function("symbols_indirect", symbols_indirect);
    c.bench_function("symbols_indirect_set", symbols_indirect_set);
    c.bench_function("symbols_indirect_set_intern", symbols_indirect_set_intern);
    c.bench_function("syntax_syntex_hash_symbols_dummy", syntax_syntex_hash_symbols_dummy);
    c.bench_function("syntax_syntex_hash_symbols_fx2", syntax_syntex_hash_symbols_fx2);
    c.bench_function("syntax_syntex_hash_symbols_fx", syntax_syntex_hash_symbols_fx);
    c.bench_function("syntax_syntex_hash_symbols_def", syntax_syntex_hash_symbols_def);
    c.bench_function("syntax_syntex_symbols", syntax_syntex_symbols);
    c.bench_function("syntax_syntex_symbols_str", syntax_syntex_symbols_str);
    c.bench_function("syntax_syntex_symbols_def", syntax_syntex_symbols_def);
    c.bench_function("find_existing", find_existing);
    c.bench_function("find_nonexisting", find_nonexisting);
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