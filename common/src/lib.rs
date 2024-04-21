// This is auto-generated code by cargo, replace this with your own code
// To make sure that other projects can access your code, everything must be publically exported from THIS file:
// - Either you have `pub` methods here (like `add`), or you have public module declarations (`pub mod $WHATEVER`)

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
