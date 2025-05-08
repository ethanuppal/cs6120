use build_cfg::{BasicBlockIdx, FunctionCfg, slotmap::SecondaryMap};

pub fn construct_postorder(cfg: &FunctionCfg) -> Vec<BasicBlockIdx> {
    fn helper(
        cfg: &FunctionCfg,
        current: BasicBlockIdx,
        visited: &mut SecondaryMap<BasicBlockIdx, bool>,
        traversal: &mut Vec<BasicBlockIdx>,
    ) {
        visited.insert(current, true);
        for successor in cfg.successors(current) {
            if !visited.contains_key(successor) {
                helper(cfg, successor, visited, traversal);
            }
        }
        traversal.push(current);
    }

    let mut traversal = vec![];
    let mut visited = SecondaryMap::with_capacity(cfg.vertices.capacity());
    helper(cfg, cfg.entry, &mut visited, &mut traversal);
    traversal
}
