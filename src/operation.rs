use crate::types::{Cell, LatinSquare, Operation, Value};

/// Assigns the most constraining operation for a set of cells given the Latin square solution.
///
/// Rules:
/// - k=1: `Given(v)`
/// - k=2: `Div(big/small)` if exactly divisible, else `Sub(|v0-v1|)`
/// - k≥3: `Mul(product)` if product ≤ n², else `Add(sum)`
pub fn assign_operation(cells: &[Cell], ls: &LatinSquare) -> Operation {
    let values: Vec<Value> = cells.iter().map(|&cell| ls.get(cell)).collect();
    match values.as_slice() {
        [v] => Operation::Given(*v),
        [a, b] => {
            let (big, small) = if a >= b {
                (*a as u32, *b as u32)
            } else {
                (*b as u32, *a as u32)
            };
            if small != 0 && big % small == 0 {
                Operation::Div(big / small)
            } else {
                Operation::Sub(big - small)
            }
        }
        _ => {
            let product: u32 = values.iter().map(|&v| v as u32).product();
            let n_squared = (ls.n * ls.n) as u32;
            if product <= n_squared {
                Operation::Mul(product)
            } else {
                let sum: u32 = values.iter().map(|&v| v as u32).sum();
                Operation::Add(sum)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_latin_square(n: usize, grid: Vec<Vec<Value>>) -> LatinSquare {
        LatinSquare { n, grid }
    }

    #[test]
    fn singleton_gives_given_op() {
        let ls = make_latin_square(3, vec![vec![3, 1, 2], vec![1, 2, 3], vec![2, 3, 1]]);
        assert_eq!(assign_operation(&[(0, 0)], &ls), Operation::Given(3));
    }

    #[test]
    fn two_cell_divisible_gives_div() {
        // values [2,6] → Div(3)
        let ls = make_latin_square(
            6,
            vec![
                vec![2, 6, 1, 3, 4, 5],
                vec![1, 2, 3, 4, 5, 6],
                vec![3, 4, 5, 6, 1, 2],
                vec![4, 5, 6, 1, 2, 3],
                vec![5, 6, 1, 2, 3, 4],
                vec![6, 1, 2, 3, 4, 5],
            ],
        );
        assert_eq!(assign_operation(&[(0, 0), (0, 1)], &ls), Operation::Div(3));
    }

    #[test]
    fn two_cell_non_divisible_gives_sub() {
        // values [2,5] → Sub(3)
        let ls = make_latin_square(
            5,
            vec![
                vec![2, 5, 1, 3, 4],
                vec![1, 2, 3, 4, 5],
                vec![3, 4, 5, 1, 2],
                vec![4, 1, 2, 5, 3],
                vec![5, 3, 4, 2, 1],
            ],
        );
        assert_eq!(assign_operation(&[(0, 0), (0, 1)], &ls), Operation::Sub(3));
    }

    #[test]
    fn three_cell_small_product_gives_mul() {
        // values [1,2,3], n=4 → product=6 ≤ 16 → Mul(6)
        let ls = make_latin_square(
            4,
            vec![
                vec![1, 2, 3, 4],
                vec![2, 1, 4, 3],
                vec![3, 4, 1, 2],
                vec![4, 3, 2, 1],
            ],
        );
        assert_eq!(
            assign_operation(&[(0, 0), (0, 1), (0, 2)], &ls),
            Operation::Mul(6)
        );
    }

    #[test]
    fn three_cell_large_product_gives_add() {
        // values [3,4,5], n=5 → product=60 > 25 → Add(12)
        let ls = make_latin_square(
            5,
            vec![
                vec![3, 4, 5, 1, 2],
                vec![1, 2, 3, 4, 5],
                vec![2, 3, 4, 5, 1],
                vec![4, 5, 1, 2, 3],
                vec![5, 1, 2, 3, 4],
            ],
        );
        assert_eq!(
            assign_operation(&[(0, 0), (0, 1), (0, 2)], &ls),
            Operation::Add(12)
        );
    }
}
