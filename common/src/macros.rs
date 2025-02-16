// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#[cfg(test)]
macro_rules! serialized_size_enum {
    ($t:ty) => {
        let vec: Vec<_> = <$t>::iter().collect();
        let array: [$t; std::mem::variant_count::<$t>()] = vec.try_into().unwrap();
        let t = stringify!($t);
        println!(
            "{t} is {} bytes",
            bitcode::serialize(&array).unwrap().len() as f32 / array.len() as f32
        );
        println!(
            "{t} is {:.1} bits",
            kodiak_common::encode_buffer(&[array; 8]).unwrap().len() as f32 / array.len() as f32
        );
    };
}

#[cfg(test)]
macro_rules! serialized_size_value {
    ($t:literal, $v:expr) => {
        let t = $t;
        let v = $v;
        println!("{t} is {} bytes", bitcode::serialize(&v).unwrap().len());
        println!(
            "{t} is {:.1} bits",
            kodiak_common::encode_buffer(&[(); 8].map(|_| &v))
                .unwrap()
                .len()
        );
    };
}

#[cfg(test)]
macro_rules! size_of {
    ($t:ty) => {
        println!("{} is {} bytes", stringify!($t), std::mem::size_of::<$t>())
    };
}
