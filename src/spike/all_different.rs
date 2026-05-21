//! All-different constraints and their two interchangeable propagators.
//!
//! The [`AllDifferentPropagator`] trait is the spike's most concrete
//! demonstration that the trait layer does real work: it lets the production
//! candidate ([`ReginAllDifferent`]) and the verification oracle
//! ([`BruteForceAllDifferent`]) be swapped for property testing.
//!
//! Unlike the production `regin()` (which omits the free-value sink and so is
//! only correct when values ≤ variables — see issue #30), [`ReginAllDifferent`]
//! here implements **full Régin**: it additionally keeps every edge reachable
//! from a free (unmatched) value, achieving true GAC even when values exceed
//! variables. The property test below confirms parity with the brute-force
//! oracle across that full regime.

use std::collections::HashMap;

use crate::{
    Cell, Fill,
    spike::{
        constraint::{Constraint, Outcome, PropagationCtx},
        store::Narrowed,
        variable::Variable,
    },
    types::N,
};

/// A swappable all-different filtering algorithm: given one domain per variable,
/// return the GAC-pruned domains in the same order.
pub trait AllDifferentPropagator {
    fn prune(&self, domains: &[Fill]) -> Vec<Fill>;
}

/// Full Régin (matching + SCC + free-value reachability). The production candidate.
pub struct ReginAllDifferent;

/// Enumerate-and-project oracle: GAC by definition, tractable only on small
/// instances.
pub struct BruteForceAllDifferent;

impl AllDifferentPropagator for ReginAllDifferent {
    fn prune(&self, domains: &[Fill]) -> Vec<Fill> {
        regin_gac(domains)
    }
}

impl AllDifferentPropagator for BruteForceAllDifferent {
    fn prune(&self, domains: &[Fill]) -> Vec<Fill> {
        brute_force_gac(domains)
    }
}

/// An all-different constraint over a fixed set of cells (a row or a column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllDiffDef {
    cells: Vec<Cell>,
}

impl AllDiffDef {
    pub fn row(n: usize, row: usize) -> Self {
        Self {
            cells: (0..n).map(|column| Cell::new(row, column)).collect(),
        }
    }

    pub fn column(n: usize, column: usize) -> Self {
        Self {
            cells: (0..n).map(|row| Cell::new(row, column)).collect(),
        }
    }
}

impl Constraint<Cell> for AllDiffDef {
    fn variables(&self) -> &[Cell] {
        &self.cells
    }

    fn propagate(&self, ctx: &mut PropagationCtx<Cell>) -> Outcome {
        let domains: Vec<Fill> = self.cells.iter().map(|c| ctx.store.get(c.id())).collect();
        let pruned = ReginAllDifferent.prune(&domains);
        let mut outcome = Outcome::Unchanged;
        for (cell, domain) in self.cells.iter().zip(pruned) {
            match ctx.store.intersect(cell.id(), domain) {
                Narrowed::Empty => return Outcome::Contradiction,
                Narrowed::Changed => outcome = Outcome::Changed,
                Narrowed::Unchanged => {}
            }
        }
        outcome
    }
}

/// Brute-force GAC: a value is kept for a variable iff some complete assignment
/// of distinct values (one per variable, each within its domain) uses it.
fn brute_force_gac(domains: &[Fill]) -> Vec<Fill> {
    let mut support = vec![Fill::default(); domains.len()];
    let mut current = vec![0u8; domains.len()];
    extend(0, domains, 0u16, &mut current, &mut support);
    support
}

fn extend(i: usize, domains: &[Fill], used: u16, current: &mut [N], support: &mut [Fill]) {
    if i == domains.len() {
        for (slot, &value) in support.iter_mut().zip(current.iter()) {
            *slot = *slot | Fill::new([value]);
        }
        return;
    }
    for value in domains[i].iter() {
        let bit = 1u16 << value;
        if used & bit == 0 {
            current[i] = value;
            extend(i + 1, domains, used | bit, current, support);
        }
    }
}

/// Full Régin GAC for all-different.
#[allow(clippy::similar_names)]
fn regin_gac(domains: &[Fill]) -> Vec<Fill> {
    let n = domains.len();
    if n == 0 {
        return vec![];
    }

    let all_values: Vec<N> = domains
        .iter()
        .fold(Fill::default(), |acc, d| acc | *d)
        .iter()
        .collect();
    let num_values = all_values.len();
    let value_index: HashMap<N, usize> = all_values
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i))
        .collect();
    let indexed_domains: Vec<Vec<usize>> = domains
        .iter()
        .map(|d| d.iter().map(|v| value_index[&v]).collect())
        .collect();

    // Maximum bipartite matching via augmenting paths.
    let mut var_match: Vec<Option<usize>> = vec![None; n];
    let mut val_match: Vec<Option<usize>> = vec![None; num_values];
    let mut visited = vec![false; num_values];
    for var in 0..n {
        visited.fill(false);
        let _ = augment(
            var,
            &indexed_domains,
            &mut var_match,
            &mut val_match,
            &mut visited,
        );
    }

    // An unmatched variable means no system of distinct representatives exists:
    // the constraint is unsatisfiable, so every domain empties.
    if var_match.iter().any(Option::is_none) {
        return vec![Fill::default(); n];
    }

    // Residual digraph. Node layout: variables 0..n, values n..n+num_values.
    // Orientation: matched edges var → val, unmatched edges val → var. (The full
    // reverse of the production orientation, chosen so a directed walk from a
    // free value is exactly an alternating path from it.)
    let total = n + num_values;
    let mut adj: Vec<Vec<usize>> = vec![vec![]; total];
    for var in 0..n {
        for &vi in &indexed_domains[var] {
            let val_node = n + vi;
            if var_match[var] == Some(vi) {
                adj[var].push(val_node);
            } else {
                adj[val_node].push(var);
            }
        }
    }

    let scc = kosaraju_scc(&adj, total);

    // Mark every node reachable from a free (unmatched) value. An unmatched edge
    // (var, val) lies on an alternating path from a free value iff its value
    // node is reachable here — this is the step the production regin() omits.
    let mut reachable = vec![false; total];
    let mut stack: Vec<usize> = (0..num_values)
        .filter(|&vi| val_match[vi].is_none())
        .map(|vi| n + vi)
        .collect();
    for &node in &stack {
        reachable[node] = true;
    }
    while let Some(node) = stack.pop() {
        for &next in &adj[node] {
            if !reachable[next] {
                reachable[next] = true;
                stack.push(next);
            }
        }
    }

    // Keep edge (var, val) iff it is matched, lies in an alternating cycle
    // (same SCC), or lies on an alternating path from a free value.
    let mut result = vec![Fill::default(); n];
    for var in 0..n {
        let matched = var_match[var];
        result[var] = indexed_domains[var]
            .iter()
            .filter(|&&vi| matched == Some(vi) || scc[var] == scc[n + vi] || reachable[n + vi])
            .map(|&vi| all_values[vi])
            .collect();
    }
    result
}

/// Tries to find an augmenting path from `var`; extends the matching if found.
#[allow(clippy::similar_names)]
fn augment(
    var: usize,
    indexed_domains: &[Vec<usize>],
    var_match: &mut [Option<usize>],
    val_match: &mut [Option<usize>],
    visited: &mut [bool],
) -> bool {
    for &vi in &indexed_domains[var] {
        if visited[vi] {
            continue;
        }
        visited[vi] = true;
        if val_match[vi]
            .is_none_or(|other| augment(other, indexed_domains, var_match, val_match, visited))
        {
            var_match[var] = Some(vi);
            val_match[vi] = Some(var);
            return true;
        }
    }
    false
}

/// Kosaraju's SCC labelling: two nodes share a label iff they are in the same SCC.
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
#[allow(clippy::unwrap_used)]
mod tests {
    use rand::{RngExt, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    use super::*;

    fn sorted(fills: &[Fill]) -> Vec<Vec<N>> {
        fills.iter().map(|f| f.iter().collect()).collect()
    }

    #[test]
    fn regin_empty_input() {
        assert!(regin_gac(&[]).is_empty());
    }

    #[test]
    fn regin_prunes_forced_chain() {
        // Var0:{1,2}, Var1:{2}, Var2:{1,3} → 0=1, 1=2, 2=3.
        let domains = vec![Fill::new([1, 2]), Fill::new([2]), Fill::new([1, 3])];
        assert_eq!(
            sorted(&regin_gac(&domains)),
            vec![vec![1], vec![2], vec![3]]
        );
    }

    #[test]
    fn regin_infeasible_empties_all() {
        // Two variables, one shared value: no distinct assignment exists.
        let domains = vec![Fill::new([1]), Fill::new([1])];
        assert_eq!(regin_gac(&domains), vec![Fill::default(), Fill::default()]);
    }

    #[test]
    fn regin_free_value_kept_full_regin() {
        // One variable, two candidate values. Both belong to some assignment, so
        // full Régin keeps both. The production regin() would wrongly prune one.
        let domains = vec![Fill::new([1, 2])];
        assert_eq!(sorted(&regin_gac(&domains)), vec![vec![1, 2]]);
    }

    #[test]
    fn brute_force_matches_known_cases() {
        assert!(brute_force_gac(&[]).is_empty());
        assert_eq!(
            sorted(&brute_force_gac(&[Fill::new([1, 2]), Fill::new([2])])),
            vec![vec![1], vec![2]]
        );
        assert_eq!(
            brute_force_gac(&[Fill::new([1]), Fill::new([1])]),
            vec![Fill::default(), Fill::default()]
        );
    }

    fn random_domains(rng: &mut ChaCha8Rng, max_vars: usize, max_values: u8) -> Vec<Fill> {
        let n_vars = rng.random_range(1..=max_vars);
        let n_values = rng.random_range(1..=max_values);
        (0..n_vars)
            .map(|_| {
                loop {
                    let mut fill = Fill::default();
                    for value in 1..=n_values {
                        if rng.random_range(0u8..2) == 1 {
                            fill = fill | Fill::new([value]);
                        }
                    }
                    if !fill.is_empty() {
                        break fill;
                    }
                }
            })
            .collect()
    }

    /// The pre-registered correctness-parity property test (issue #106): across
    /// a few thousand random small instances spanning the full ≤8-variable /
    /// ≤8-value regime (including value > variable, where the production
    /// `regin()` is wrong), full Régin must agree with the brute-force GAC oracle.
    #[test]
    fn regin_matches_brute_force_oracle() {
        let mut rng = ChaCha8Rng::seed_from_u64(0x5151_2026);
        let mut saw_free_value_case = false;
        // Swap the two impls through the trait — the carve-out the trait layer
        // exists to enable.
        let regin = ReginAllDifferent;
        let oracle = BruteForceAllDifferent;
        for _ in 0..5000 {
            let domains = random_domains(&mut rng, 8, 8);
            let values: Fill = domains.iter().fold(Fill::default(), |acc, d| acc | *d);
            if values.len() > domains.len() {
                saw_free_value_case = true;
            }
            assert_eq!(
                regin.prune(&domains),
                oracle.prune(&domains),
                "Régin and brute force disagree on {domains:?}"
            );
        }
        // Confirm the distribution actually exercised the free-value regime that
        // distinguishes full Régin from the production implementation.
        assert!(saw_free_value_case);
    }
}
