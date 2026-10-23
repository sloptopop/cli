fn f() -> i32 {
    1
}
fn main() {
    let _t = "é😀"; let x = f();
    let v: Vec<i32> = Vec::new();
    let n = v.len();
    println!("{x} {n}");
}
