use std::collections::HashSet;

use build_cfg::{BasicBlockIdx, FunctionCfg, slotmap::SecondaryMap};
use dataflow::construct_postorder;

pub fn compute_dominators(
    cfg: &FunctionCfg,
) -> SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>> {
    let mut reverse_postorder = construct_postorder(cfg);
    reverse_postorder.reverse();
    reverse_postorder.retain(|idx| *idx != cfg.entry);

    let all_blocks = cfg.vertices.keys().collect::<HashSet<_>>();
    let mut dominators = SecondaryMap::new();
    for block_idx in cfg.vertices.keys() {
        dominators.insert(block_idx, all_blocks.clone());
    }
    dominators[cfg.entry] = HashSet::from_iter([cfg.entry]);

    let mut needs_update = true;
    while needs_update {
        needs_update = false;
        for block_idx in reverse_postorder.iter().copied() {
            let previous = dominators[block_idx].clone();
            let mut new = HashSet::new();
            for (i, pred_idx) in
                cfg.predecessors(block_idx).iter().copied().enumerate()
            {
                if i == 0 {
                    new = dominators[pred_idx].clone();
                } else {
                    new = new
                        .intersection(&dominators[pred_idx])
                        .copied()
                        .collect();
                }
            }
            new.insert(block_idx);
            if new != previous {
                needs_update = true;
            }
            dominators[block_idx] = new;
        }
    }

    dominators
}

pub fn compute_dominator_tree(
    dominators: &SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>>,
) -> SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>> {
    let mut rev = SecondaryMap::<_, HashSet<_>>::new();
    for (idx, edge) in dominators.iter() {
        for dest_idx in edge {
            let entry = rev.entry(*dest_idx).unwrap().or_default();
            if idx != *dest_idx {
                entry.insert(idx);
            }
        }
    }

    let mut tree = SecondaryMap::<_, HashSet<_>>::new();

    for (idx, mut dominated) in rev.clone() {
        for (other_idx, other_dominated) in &rev {
            if other_idx != idx && !other_dominated.contains(&idx) {
                dominated.retain(|dominated_idx| {
                    !other_dominated.contains(dominated_idx)
                });
            }
        }
        tree.insert(idx, dominated);
    }

    tree
}

pub fn compute_dominance_frontiers(
    cfg: &FunctionCfg,
    dominators: SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>>,
) -> SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>> {
    let mut rev = SecondaryMap::<_, HashSet<_>>::new();
    for (idx, edge) in dominators {
        for dest_idx in edge {
            let entry = rev.entry(dest_idx).unwrap().or_default();
            if idx != dest_idx {
                entry.insert(idx);
            }
        }
    }

    let mut frontiers = SecondaryMap::<_, HashSet<_>>::new();
    for (idx, dominated) in rev {
        let mut successors = HashSet::new();

        for dominated_idx in &dominated {
            successors.extend(cfg.successors(*dominated_idx));
        }

        // don't forget that a node dominates itself, so we also
        // check its own successors (we removed
        // this for convenience when constructing rev)
        successors.extend(cfg.successors(idx));

        successors.retain(|idx| !dominated.contains(idx));
        frontiers.insert(idx, successors);
    }

    frontiers
}
