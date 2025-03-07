use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use bril_rs::{EffectOps, Instruction, Type, ValueOps};
use build_cfg::{slotmap::SecondaryMap, BasicBlockIdx, FunctionCfg};
use snafu::{OptionExt, Whatever};

pub struct DefinitionSites(pub BTreeMap<String, (Type, Vec<BasicBlockIdx>)>);

/// The basic blocks a given variable was assigned in along with its type.
pub fn compute_definition_sites(cfg: &FunctionCfg) -> DefinitionSites {
    DefinitionSites(
        cfg.vertices
            .iter()
            .map(|(block_idx, block)| {
                block.instructions.iter().filter_map(move |instruction| {
                    match instruction {
                        bril_rs::Instruction::Constant {
                            dest: definition,
                            const_type: ty,
                            ..
                        }
                        | bril_rs::Instruction::Value {
                            dest: definition,
                            op_type: ty,
                            ..
                        } => Some((definition.clone(), ty.clone(), block_idx)),
                        _ => None,
                    }
                })
            })
            .fold(BTreeMap::new(), |mut definitions, some_definitions| {
                for (definition, ty, block_idx) in some_definitions {
                    definitions
                        .entry(definition)
                        .or_insert_with(|| (ty, Vec::default()))
                        .1
                        .push(block_idx);
                }
                definitions
            }),
    )
}

pub struct PhiInsertionPoints(
    pub BTreeMap<String, (Type, BTreeSet<BasicBlockIdx>)>,
);

/// For each variable, its type and a set of basic blocks that need a Phi node
/// for it.
pub fn determine_phi_insertion_points(
    definition_sites: DefinitionSites,
    dominance_frontiers: SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>>,
) -> PhiInsertionPoints {
    let mut insertion_points = BTreeMap::new();

    for (variable, (ty, mut definition_blocks)) in definition_sites.0 {
        while let Some(block_idx) = definition_blocks.pop() {
            for frontier_idx in &dominance_frontiers[block_idx] {
                let ty = ty.clone();
                insertion_points
                    .entry(variable.clone())
                    .or_insert_with(move || {
                        (ty, BTreeSet::<BasicBlockIdx>::default())
                    })
                    .1
                    .insert(*frontier_idx);
            }
        }
    }

    PhiInsertionPoints(insertion_points)
}

pub fn insert_phis(
    cfg: &mut FunctionCfg,
    phi_insertion_points: PhiInsertionPoints,
) {
    let mut phis_to_insert = SecondaryMap::new();
    for (variable, (ty, places_to_insert)) in phi_insertion_points.0 {
        for place_to_insert in places_to_insert {
            phis_to_insert
                .entry(place_to_insert)
                .unwrap()
                .or_insert_with(Vec::default)
                .push(Instruction::Value {
                    args: vec![],
                    dest: variable.clone(),
                    funcs: vec![],
                    labels: vec![],
                    op: ValueOps::Get,
                    pos: None,
                    op_type: ty.clone(),
                });
        }
    }
    for (block_idx, phis) in phis_to_insert {
        cfg.vertices[block_idx].instructions.splice(0..0, phis);
    }
}

#[derive(Default)]
pub struct DominatingDefinitionsStacks {
    inner: HashMap<String, Vec<BasicBlockIdx>>,
}

impl DominatingDefinitionsStacks {
    pub fn with_new_definitions<T>(
        &mut self,
        current_idx: BasicBlockIdx,
        new_definitions: &[String],
        then: impl FnOnce(&mut Self) -> T,
    ) -> T {
        for new_definition in new_definitions {
            self.inner
                .entry(new_definition.clone())
                .or_default()
                .push(current_idx);
        }
        let result = then(self);
        for new_definition in new_definitions {
            if let Some(stack) = self.inner.get_mut(new_definition) {
                let popped_idx = stack.pop();
                assert_eq!(
                    popped_idx.expect("We just pushed to this key"),
                    current_idx
                );
            }
        }
        result
    }
}

#[derive(Default)]
pub struct LocalRenamer {
    inner: HashMap<String, usize>,
}

pub fn rename_and_insert_upsilons(
    cfg: &mut FunctionCfg,
    block_idx: BasicBlockIdx,
    dominance_tree: &SecondaryMap<BasicBlockIdx, HashSet<BasicBlockIdx>>,
    dominating_definitions_stacks: &mut DominatingDefinitionsStacks,
    //other_undefs: &mut HashSet<(String, Type)>,
) {
    let block_idx_number = block_idx.as_index_for_slotmap_version_1_0_7_only();

    let mut local_renamer = LocalRenamer::default();

    //let mut local_rename_map = HashMap::new();
    //let local_rename = |map: &mut HashMap<String, usize>,
    //                    i: usize,
    //                    last: &HashMap<String, usize>,
    //                    variable: String|
    // -> String {
    //    if let Some(last_idx) = last.get(&variable).copied() {
    //        if i == last_idx {
    //            return format!("{variable}.{block_idx_number}.last");
    //        }
    //    }
    //    let current_value = map.entry(variable.clone()).or_insert(0);
    //    *current_value += 1;
    //    format!("{variable}.{block_idx_number}.{current_value}")
    //};
    //let rename_arguments = |map: &HashMap<String, usize>,
    //                        mini_stack: &HashMap<
    //    String,
    //    Vec<BasicBlockIdx>,
    //>,
    //                        i: usize,
    //                        last: &HashMap<String, usize>,
    //                        args: Vec<String>|
    // -> Vec<String> {
    //    args.into_iter()
    //        .map(|arg| {
    //            if let Some(last_idx) = last.get(&arg).copied() {
    //                if i > last_idx {
    //                    return format!("{arg}.{block_idx_number}.last");
    //                }
    //            }
    //
    //            if let Some(local_id) = map.get(&arg).copied() {
    //                format!("{arg}.{block_idx_number}.{local_id}")
    //            } else if let Some(dom_idx) =
    //                mini_stack.get(&arg).and_then(|stack|
    // stack.last()).copied()            {
    //                format!(
    //                    "{arg}.{}.last",
    //                    dom_idx.as_index_for_slotmap_version_1_0_7_only()
    //                )
    //            } else {
    //                // it must be a function arg?
    //                arg
    //            }
    //        })
    //        .collect()
    //};
    //
    //let mut last_assignment = HashMap::new();
    //
    //for (i, instruction) in
    //    cfg.vertices[block_idx].instructions.iter().enumerate()
    //{
    //    if let Instruction::Constant { dest, .. }
    //    | Instruction::Value { dest, .. } = &instruction
    //    {
    //        last_assignment.insert(dest.clone(), i);
    //    }
    //}
    //
    //for (name, _) in last_assignment.clone() {
    //    mini_stack.entry(name).or_default().push(block_idx);
    //}
    //
    //for (i, instruction) in
    //    cfg.vertices[block_idx].instructions.iter_mut().enumerate()
    //{
    //    *instruction = match instruction.clone() {
    //        Instruction::Constant {
    //            dest,
    //            op,
    //            pos,
    //            const_type,
    //            value,
    //        } => Instruction::Constant {
    //            dest: local_rename(
    //                &mut local_rename_map,
    //                i,
    //                &last_assignment,
    //                dest,
    //            ),
    //            op,
    //            pos,
    //            const_type,
    //            value,
    //        },
    //        Instruction::Value {
    //            args,
    //            dest,
    //            funcs,
    //            labels,
    //            op,
    //            pos,
    //            op_type,
    //        } => {
    //            let new_args = rename_arguments(
    //                &local_rename_map,
    //                mini_stack,
    //                i,
    //                &last_assignment,
    //                args,
    //            );
    //            Instruction::Value {
    //                args: new_args,
    //                dest: local_rename(
    //                    &mut local_rename_map,
    //                    i,
    //                    &last_assignment,
    //                    dest,
    //                ),
    //                funcs,
    //                labels,
    //                op,
    //                pos,
    //                op_type,
    //            }
    //        }
    //        Instruction::Effect {
    //            args,
    //            funcs,
    //            labels,
    //            op,
    //            pos,
    //        } if !matches!(op, EffectOps::Set) => Instruction::Effect {
    //            args: rename_arguments(
    //                &local_rename_map,
    //                mini_stack,
    //                i,
    //                &last_assignment,
    //                args,
    //            ),
    //            funcs,
    //            labels,
    //            op,
    //            pos,
    //        },
    //        other => other,
    //    };
    //}
    //
    //let mut undefs = HashSet::new();
    //
    //for next_idx in cfg.successors(block_idx) {
    //    let new_gets = cfg.vertices[next_idx]
    //        .instructions
    //        .iter()
    //        .filter_map(|instruction| match instruction {
    //            Instruction::Value {
    //                dest,
    //                op: ValueOps::Get,
    //                op_type,
    //                ..
    //            } => {
    //                let original_name = dest
    //                    .split_once(".")
    //                    .map(|(name, _)| name.to_string())
    //                    .unwrap_or(dest.clone());
    //                let next_idx_index =
    //                    next_idx.as_index_for_slotmap_version_1_0_7_only();
    //                let set_from = if last_assignment
    //                    .contains_key(&original_name)
    //                {
    //                    format!("{original_name}.{block_idx_number}.last")
    //                } else if let Some(dom_idx) = mini_stack
    //                    .get(&original_name)
    //                    .and_then(|stack| stack.last())
    //                    .copied()
    //                {
    //                    format!(
    //                        "{original_name}.{}.last",
    //                        dom_idx.as_index_for_slotmap_version_1_0_7_only()
    //                    )
    //                } else {
    //                    undefs.insert((original_name.clone(),
    // op_type.clone()));
    // format!("{original_name}.{block_idx_number}.undef")                };
    //                other_undefs.insert((set_from.clone(), op_type.clone()));
    //                Some(Instruction::Effect {
    //                    args: vec![
    //                        format!("{original_name}.{next_idx_index}.1"),
    //                        set_from,
    //                    ],
    //                    funcs: vec![],
    //                    labels: vec![],
    //                    op: bril_rs::EffectOps::Set,
    //                    pos: None,
    //                })
    //            }
    //            _ => None,
    //        })
    //        .collect::<Vec<_>>();
    //    let has_no_trailing_branch =
    //        matches!(cfg.vertices[block_idx].exit, LabeledExit::Fallthrough);
    //    let last = if !has_no_trailing_branch {
    //        cfg.vertices[block_idx].instructions.pop()
    //    } else {
    //        None
    //    };
    //    cfg.vertices[block_idx]
    //        .instructions
    //        .extend(new_gets.into_iter().chain(last.into_iter()));
    //}
    //
    //for undef in undefs {
    //    cfg.vertices[block_idx].instructions.insert(
    //        0,
    //        Instruction::Value {
    //            args: vec![],
    //            dest: format!("{}.{block_idx_number}.undef", undef.0),
    //            funcs: vec![],
    //            labels: vec![],
    //            op: ValueOps::Undef,
    //            pos: None,
    //            op_type: undef.1,
    //        },
    //    );
    //}
    //
    //for imm_idx in &dominance_tree[block_idx] {
    //    rename(cfg, *imm_idx, dominance_tree, mini_stack, other_undefs);
    //}
    //
    //for name in last_assignment.keys() {
    //    if let Some(stack) = mini_stack.get_mut(name) {
    //        stack.pop();
    //    }
    //}
}

pub fn dumb_postprocess(
    cfg: &mut FunctionCfg,
    mut potential_undefs: HashMap<String, Type>,
) {
    for block in cfg.vertices.values_mut() {
        for instruction in &block.instructions {
            if let Instruction::Constant { dest, .. }
            | Instruction::Value { dest, .. } = &instruction
            {
                potential_undefs.remove(dest);
            }
        }
    }
    for (other, ty) in potential_undefs {
        cfg.vertices[cfg.entry].instructions.insert(
            0,
            Instruction::Value {
                args: vec![],
                dest: other,
                funcs: vec![],
                labels: vec![],
                op: ValueOps::Undef,
                pos: None,
                op_type: ty,
            },
        );
    }
}

pub fn from_ssa(cfg: &mut FunctionCfg) -> Result<(), Whatever> {
    let mut set_operation_types = HashMap::new();
    for block in cfg.vertices.values() {
        for instruction in &block.instructions {
            if let Instruction::Value {
                dest,
                op: ValueOps::Get,
                op_type,
                ..
            } = instruction
            {
                set_operation_types.insert(dest.clone(), op_type.clone());
            }
        }
    }

    for block in cfg.vertices.values_mut() {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                Instruction::Value {
                    op: ValueOps::Get,
                    ..
                },
            )
        });
        for instruction in block.instructions.iter_mut() {
            if let Some(replacement) = if let Instruction::Effect {
                args,
                op: EffectOps::Set,
                ..
            } = instruction
            {
                assert!(
                    args.len() == 2,
                    "EffectOps::Set should have two arguments"
                );
                Some(Instruction::Value {
                    args: vec![args[1].clone()],
                    dest: args[0].clone(),
                    funcs: vec![],
                    labels: vec![],
                    op: ValueOps::Id,
                    pos: None,
                    op_type: set_operation_types.get(&args[0]).whatever_context("The corresponding `get` instruction does not exist or did not specify a type")?.clone(),
                })
            } else {
                None
            } {
                *instruction = replacement;
            }
        }
    }

    Ok(())
}
