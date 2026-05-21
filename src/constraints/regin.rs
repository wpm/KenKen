/// Régin's arc-consistency algorithm for the all-different constraint.
///
/// Given a list of domains (one per variable), removes any value from a domain
/// that cannot appear in any solution satisfying all-different. Returns the
/// pruned domains in the same order.
///
/// Algorithm (Régin 1994):
/// 1. Find a maximum bipartite matching between variables and values.
/// 2. Build the residual graph of the matching, augmented with a sink node that turns
///    alternating paths from free (unmatched) values into cycles.
/// 3. Compute SCCs of the residual graph.
/// 4. An edge (variable → value) is arc-consistent iff it is in the matching OR both endpoints
///    lie in the same SCC. Remove all other edges.
///
/// Works for any number of values relative to variables (`#values ≤ #variables`
/// or `>`). When there are more values than variables some values are "free"
/// (unmatched); the sink node links every free value back to the variables, so
/// edges to free values survive whenever they take part in a valid assignment.
/// Without that node a free value would sit in its own SCC and its edges would
/// be wrongly pruned (see issue #30).
use std::collections::HashMap;

use crate::types::{Fill, N};

#[allow(clippy::similar_names)]
pub fn regin(domains: &[Fill]) -> Vec<Fill> {
    let n = domains.len();
    if n == 0 {
        return vec![];
    }

    // Collect all values that appear in at least one domain.
    let all_values: Vec<N> = domains
        .iter()
        .fold(Fill::default(), |acc, d| acc | *d)
        .iter()
        .collect();

    // Map values to compact indices for the graph and matching arrays.
    let value_index: HashMap<N, usize> = all_values
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i))
        .collect();
    let num_values = all_values.len();

    // Pre-convert each domain to a Vec of value indices to avoid repeated
    // hash lookups in the hot augmenting-path search.
    let indexed_domains: Vec<Vec<usize>> = domains
        .iter()
        .map(|d| d.iter().map(|v| value_index[&v]).collect())
        .collect();

    // var_match[i]  = Some(value_idx) if variable i is matched.
    // val_match[j]  = Some(var_idx)   if value j is matched.
    let mut var_match: Vec<Option<usize>> = vec![None; n];
    let mut val_match: Vec<Option<usize>> = vec![None; num_values];
    let mut visited = vec![false; num_values];

    #[allow(unused_results)]
    for var in 0..n {
        visited.fill(false);
        augment(
            var,
            &indexed_domains,
            &mut var_match,
            &mut val_match,
            &mut visited,
        );
    }

    // Build the directed residual graph.
    //
    // Node layout: variables occupy 0..n, values occupy n..n+num_values, and a
    // single sink node occupies n+num_values. Unmatched edges go var → n+val;
    // matched edges are reversed: n+val → var.
    let sink = n + num_values;
    let total_nodes = sink + 1;
    let mut adj: Vec<Vec<usize>> = vec![vec![]; total_nodes];

    for var in 0..n {
        for &vi in &indexed_domains[var] {
            let val_node = n + vi;
            if var_match[var] == Some(vi) {
                adj[val_node].push(var);
            } else {
                adj[var].push(val_node);
            }
        }
    }

    // Sink node: every free (unmatched) value points to the sink, and the sink
    // points back to every variable. This closes the alternating paths that
    // start at a free value into directed cycles, so the SCC test below keeps
    // edges to free values that participate in a valid assignment.
    for (vi, matched) in val_match.iter().enumerate() {
        if matched.is_none() {
            adj[n + vi].push(sink);
        }
    }
    adj[sink] = (0..n).collect();

    let scc = kosaraju_scc(&adj, total_nodes);

    // An unmatched edge (var → value) is consistent iff both endpoints are in
    // the same SCC. Remove all others.
    let mut result: Vec<Fill> = domains.to_vec();

    for var in 0..n {
        let matched_vi = var_match[var];
        result[var] = result[var]
            .iter()
            .filter(|&val_v| {
                let vi = value_index[&val_v];
                matched_vi == Some(vi) || scc[var] == scc[n + vi]
            })
            .collect::<Fill>();
    }

    result
}

/// Tries to find an augmenting path from `var` using DFS.
/// Returns true if the matching was extended.
///
/// Impure: mutates `var_match`, `val_match`, and `visited` in place. Making
/// this pure would require cloning the matching vectors on every recursive
/// call, since each frame reads and writes `val_match` to reassign values
/// along the augmenting path.
#[allow(clippy::similar_names)]
fn augment(
    var: usize,
    indexed_domains: &[Vec<usize>],
    var_match: &mut Vec<Option<usize>>,
    val_match: &mut Vec<Option<usize>>,
    visited: &mut [bool],
) -> bool {
    for &vi in &indexed_domains[var] {
        if visited[vi] {
            continue;
        }
        visited[vi] = true;
        if val_match[vi].is_none_or(|other_var| {
            augment(other_var, indexed_domains, var_match, val_match, visited)
        }) {
            var_match[var] = Some(vi);
            val_match[vi] = Some(var);
            return true;
        }
    }
    false
}

/// Kosaraju's SCC algorithm. Returns a component label per node such that two
/// nodes share a label iff they are in the same SCC.
fn kosaraju_scc(adj: &[Vec<usize>], n: usize) -> Vec<usize> {
    let mut visited = vec![false; n];
    let mut finish_order: Vec<usize> = Vec::with_capacity(n);
    for start in 0..n {
        if !visited[start] {
            finish_order.extend(dfs_finish(start, adj, &mut visited));
        }
    }

    let mut radj: Vec<Vec<usize>> = vec![vec![]; n];
    for (u, neighbors) in adj.iter().enumerate().take(n) {
        for &v in neighbors {
            radj[v].push(u);
        }
    }

    let mut comp = vec![usize::MAX; n];
    let mut label = 0usize;
    for &start in finish_order.iter().rev() {
        if comp[start] == usize::MAX {
            dfs_assign(start, label, &radj, &mut comp);
            label += 1;
        }
    }

    comp
}

fn dfs_finish(start: usize, adj: &[Vec<usize>], visited: &mut [bool]) -> Vec<usize> {
    // Iterative to avoid stack overflow on large graphs.
    let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
    let mut order: Vec<usize> = vec![];
    visited[start] = true;
    while let Some((u, idx)) = stack.last_mut() {
        let u = *u;
        if *idx < adj[u].len() {
            let v = adj[u][*idx];
            *idx += 1;
            if !visited[v] {
                visited[v] = true;
                stack.push((v, 0));
            }
        } else {
            order.push(u);
            let _ = stack.pop();
        }
    }
    order
}

/// Impure: writes component labels into `comp` in place. The caller owns
/// `comp` and passes it mutably so all SCC passes share one allocation.
fn dfs_assign(start: usize, label: usize, radj: &[Vec<usize>], comp: &mut [usize]) {
    let mut stack = vec![start];
    comp[start] = label;
    while let Some(u) = stack.pop() {
        for &v in &radj[u] {
            if comp[v] == usize::MAX {
                comp[v] = label;
                stack.push(v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert_eq!(regin(&[]), vec![]);
    }

    #[test]
    fn singleton_domains_unchanged() {
        let domains = vec![Fill::new([1]), Fill::new([2]), Fill::new([3])];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }

    #[test]
    fn prunes_impossible_value() {
        // Var0:{1,2}, Var1:{2}, Var2:{1,3}
        // Var1 must be 2, so 2 is pruned from Var0, leaving Var0:{1}.
        // Var0 must then be 1, so 1 is pruned from Var2, leaving Var2:{3}.
        // Valid assignment: Var0=1, Var1=2, Var2=3.
        let domains = vec![Fill::new([1, 2]), Fill::new([2]), Fill::new([1, 3])];
        let result = regin(&domains);
        assert_eq!(result[0], Fill::new([1]));
        assert_eq!(result[1], Fill::new([2]));
        assert_eq!(result[2], Fill::new([3]));
    }

    #[test]
    fn full_overlap_no_pruning() {
        // 3 variables each with domain {1,2,3}: any permutation is valid, no pruning.
        let domains = vec![
            Fill::new([1, 2, 3]),
            Fill::new([1, 2, 3]),
            Fill::new([1, 2, 3]),
        ];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }

    #[test]
    fn fixed_third_var_no_pruning_on_pair() {
        // Var2 is fixed to 3, not in Var0/Var1 domains — no pruning occurs.
        let domains = vec![Fill::new([1, 2]), Fill::new([1, 2]), Fill::new([3])];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }

    #[test]
    fn all_values_reachable_no_pruning() {
        // Var0:{1,3}, Var1:{2,3}, Var2:{1,2}
        // Valid: 0=3,1=2,2=1; 0=1,1=3,2=2. All values participate in some solution.
        let domains = vec![Fill::new([1, 3]), Fill::new([2, 3]), Fill::new([1, 2])];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }

    #[test]
    fn unmatched_free_values_are_kept() {
        // #30: two variables sharing {2,3,4}. With more values than variables,
        // value 4 is "free" under any maximum matching but still participates in
        // valid assignments (e.g. var0=4, var1=2), so nothing may be pruned.
        let domains = vec![Fill::new([2, 3, 4]), Fill::new([2, 3, 4])];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }

    #[test]
    fn more_values_than_variables_still_prunes() {
        // Var1 is fixed to 2, so 2 cannot appear in Var0 even though there are
        // more values (1,2,3) than variables. The free value 3 must survive.
        let domains = vec![Fill::new([1, 2, 3]), Fill::new([2])];
        let result = regin(&domains);
        assert_eq!(result[0], Fill::new([1, 3]));
        assert_eq!(result[1], Fill::new([2]));
    }

    #[test]
    fn singleton_forces_pruning_with_free_value() {
        // Var0 fixed to 1 forces 1 out of Var1; the surplus value 3 stays.
        let domains = vec![Fill::new([1]), Fill::new([1, 2, 3])];
        let result = regin(&domains);
        assert_eq!(result[0], Fill::new([1]));
        assert_eq!(result[1], Fill::new([2, 3]));
    }
}
