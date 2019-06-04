#[macro_use]
extern crate arraycollect;
use arraycollect::IntoArray;

fn main() {
    //let array = arraycollect!((0..10).map(Box::new) => [Box<usize>; 10]);
    let array = (0..10).map(Box::new).array_collect::<[_; 10]>();
    assert_eq!(
        array,
        Ok([
            Box::new(0),
            Box::new(1),
            Box::new(2),
            Box::new(3),
            Box::new(4),
            Box::new(5),
            Box::new(6),
            Box::new(7),
            Box::new(8),
            Box::new(9)
        ])
    );
}
