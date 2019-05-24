#[macro_use]
extern crate arraycollect;

fn main() {
    let array = arraycollect!(0..10 => [usize; 10]);

    assert_eq!(array, Ok([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]));
}
