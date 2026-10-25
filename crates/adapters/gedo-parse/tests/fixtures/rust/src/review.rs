//! Constructs from the review of the syntax model: unsorted use lists, one-element tuples and
//! jump statements.

mod imports {
    use std::{io, fmt, collections::{HashMap, BTreeMap}};

    pub fn table() -> (HashMap<u8, u8>,) {
        (HashMap::new(),)
    }

    fn sorted(map: BTreeMap<u8, u8>) -> Vec<u8> {
        use std::{iter::Iterator, cmp::Ord};
        map.into_keys().collect()
    }
}

fn first(values: &[u8]) -> Option<u8> {
    for value in values {
        if *value > 0 {
            return Some(*value);
        }
    }
    None
}

fn early(flag: bool) -> u8 {
    if flag {
        return 1;
    }
    let (single,) = (2,);
    single
}

fn spin() {
    loop {
        break;
    }
}
