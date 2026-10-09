// Each function's expected cyclomatic complexity is annotated as cc=N.

fn trivial() -> i32 { 1 }                              // cc=1

fn binary_ops(a: bool, b: bool) -> bool { a && b || a }  // cc=3

fn question(x: Result<i32, ()>) -> Result<i32, ()> {     // cc=2
    let v = x?;
    Ok(v)
}

fn outer(a: bool) -> i32 {                               // cc=2 (nested closure excluded)
    let inner = |b: bool| if b { 1 } else { 2 };          // cc=2
    if a { inner(a) } else { 0 }
}

fn matcher(x: Option<i32>) -> i32 {                      // cc=4
    match x {
        Some(v) if v > 0 => 1,
        Some(_) => 2,
        None => 3,
    }
}

impl Widget {
    fn method(&self, x: i32) -> i32 {                     // cc=3
        for _ in 0..x { }
        while x > 0 { }
        0
    }
}

#[cfg(test)]
mod tests {                                             // excluded by default
    use super::*;
    #[test]
    fn checks() { assert_eq!(trivial(), 1); }
    fn test_helper(a: bool) -> bool { a && a }            // cc=2
}
