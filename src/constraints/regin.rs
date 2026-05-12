#![allow(dead_code)]

use crate::types::{N, Values};
/// Régin's arc-consistency algorithm for the all-different constraint.
///
/// Given a list of domains (one per variable), removes any value from a domain
/// that cannot appear in any solution satisfying all-different. Returns the
/// pruned domains in the same order.
///
/// Algorithm (Régin 1994):
/// 1. Find a maximum bipartite matching between variables and values.
/// 2. Build the residual graph of the matching.
/// 3. Compute SCCs of the residual graph.
/// 4. An edge (variable → value) is arc-consistent iff it is in the matching
///    OR both endpoints lie in the same SCC. Remove all other edges.
///
/// # Precondition
///
/// The union of the domains must contain at most `domains.len()` values
/// (equivalently, every feasible assignment is a perfect matching that uses
/// every value). When there are more values than variables, some values are
/// "free" (unmatched) and the SCC-only residual graph used here isolates them
/// in their own component, causing edges to those values to be pruned even
/// when they participate in valid assignments. The full Régin construction
/// fixes this by adding a sink node connected to free values; this
/// implementation omits that step. See issue #30.
///
/// The only current caller (`AllDifferent::value_filter`) always passes
/// a full row or column of an `n`×`n` grid with values `{1..=n}`, so the
/// precondition holds. New callers must ensure it before using `regin`.
use std::collections::HashMap;

#[must_use]
#[allow(clippy::similar_names)]
pub fn regin(domains: &[Values]) -> Vec<Values> {
    let n = domains.len();
    if n == 0 {
        return vec![];
    }

    // Collect all values that appear in at least one domain.
    let all_values: Vec<N> = domains
        .iter()
        .fold(Values::default(), |acc, d| acc | *d)
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
    // Node layout: variables occupy 0..n, values occupy n..n+num_values.
    // Unmatched edges go var → n+val; matched edges are reversed: n+val → var.
    let total_nodes = n + num_values;
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

    let scc = kosaraju_scc(&adj, total_nodes);

    // An unmatched edge (var → value) is consistent iff both endpoints are in
    // the same SCC. Remove all others.
    let mut result: Vec<Values> = domains.to_vec();

    for var in 0..n {
        let matched_vi = var_match[var];
        result[var] = result[var]
            .iter()
            .filter(|&val_v| {
                let vi = value_index[&val_v];
                matched_vi == Some(vi) || scc[var] == scc[n + vi]
            })
            .collect::<Values>();
    }

    result
}

/// Tries to find an augmenting path from `var` using DFS.
/// Returns true if the matching was extended.
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
            dfs_finish(start, adj, &mut visited, &mut finish_order);
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

fn dfs_finish(start: usize, adj: &[Vec<usize>], visited: &mut [bool], order: &mut Vec<usize>) {
    // Iterative to avoid stack overflow on large graphs.
    let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
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
            stack.pop();
        }
    }
}

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
        let domains = vec![Values::new([1]), Values::new([2]), Values::new([3])];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }

    #[test]
    fn prunes_impossible_value() {
        // Var0:{1,2}, Var1:{2}, Var2:{1,3}
        // Var1 must be 2, so 2 is pruned from Var0, leaving Var0:{1}.
        // Var0 must then be 1, so 1 is pruned from Var2, leaving Var2:{3}.
        // Valid assignment: Var0=1, Var1=2, Var2=3.
        let domains = vec![Values::new([1, 2]), Values::new([2]), Values::new([1, 3])];
        let result = regin(&domains);
        assert_eq!(result[0], Values::new([1]));
        assert_eq!(result[1], Values::new([2]));
        assert_eq!(result[2], Values::new([3]));
    }

    #[test]
    fn full_overlap_no_pruning() {
        // 3 variables each with domain {1,2,3}: any permutation is valid, no pruning.
        let domains = vec![
            Values::new([1, 2, 3]),
            Values::new([1, 2, 3]),
            Values::new([1, 2, 3]),
        ];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }

    #[test]
    fn fixed_third_var_no_pruning_on_pair() {
        // Var2 is fixed to 3, not in Var0/Var1 domains — no pruning occurs.
        let domains = vec![Values::new([1, 2]), Values::new([1, 2]), Values::new([3])];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }

    #[test]
    fn all_values_reachable_no_pruning() {
        // Var0:{1,3}, Var1:{2,3}, Var2:{1,2}
        // Valid: 0=3,1=2,2=1; 0=1,1=3,2=2. All values participate in some solution.
        let domains = vec![
            Values::new([1, 3]),
            Values::new([2, 3]),
            Values::new([1, 2]),
        ];
        let result = regin(&domains);
        assert_eq!(result, domains);
    }
}
