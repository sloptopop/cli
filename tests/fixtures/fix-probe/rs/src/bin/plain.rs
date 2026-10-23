fn f() -> i32 {
    return 1;
}
fn main() {
    let mut x = f();
    let v: Vec<i32> = Vec::new();
    let n = v.iter().count();
    println!("{x} {n}");
}
