//! Original legacy widget expressions and comparison conditions.
use super::{
    Component, skills,
    state::{Key, State},
    vm::Vm,
};

pub fn run(state: &mut State, vm: &Vm, index: i32, c: &Component) -> i32 {
    let Some(scripts) = &c.cs1_scripts else {
        return -2;
    };
    if index >= scripts.len() as i32 {
        return -2;
    }
    fn inner(state: &mut State, vm: &Vm, script: &[i32]) -> Result<i32, ()> {
        let mut pc = 0;
        let mut accumulator = 0i32;
        let mut mode = 0;
        fn next(script: &[i32], pc: &mut usize) -> Result<i32, ()> {
            let value = *script.get(*pc).ok_or(())?;
            *pc += 1;
            Ok(value)
        }
        fn varp(vm: &Vm, id: i32) -> Result<i32, ()> {
            if !(0..3500).contains(&id) {
                return Err(());
            }
            Ok(vm.varps.get(&id).copied().unwrap_or(0))
        }
        loop {
            let op = next(script, &mut pc)?;
            let mut value = 0i32;
            let mut next_mode = 0;
            match op {
                0 => return Ok(accumulator),
                1..=3 => {
                    let id = next(script, &mut pc)? as usize;
                    value = *match op {
                        1 => &vm.boosted_levels,
                        2 => &vm.base_levels,
                        _ => &vm.experience,
                    }
                    .get(id)
                    .ok_or(())?;
                }
                4 | 10 => {
                    let pointer = next(script, &mut pc)?
                        .wrapping_shl(16)
                        .wrapping_add(next(script, &mut pc)?);
                    let other = state.get_mut(Key::component(pointer)).ok().cloned();
                    let id = next(script, &mut pc)?;
                    if id != -1
                        && (!state.items.get(&state.pack, id).map_err(|_| ())?.members
                            || vm.members)
                    {
                        let other = other.ok_or(())?;
                        let ids = other.obj_types.ok_or(())?;
                        for (i, n) in ids.into_iter().enumerate() {
                            if id.wrapping_add(1) == n {
                                if op == 10 {
                                    value = 999999999;
                                    break;
                                }
                                value = value.wrapping_add(
                                    *other.obj_counts.as_ref().and_then(|v| v.get(i)).ok_or(())?,
                                );
                            }
                        }
                    }
                }
                5 => value = varp(vm, next(script, &mut pc)?)?,
                6 => {
                    let skill = next(script, &mut pc)? as usize;
                    let level = vm.base_levels.get(skill).ok_or(())?.wrapping_sub(1);
                    value = *skills::thresholds().get(level as usize).ok_or(())?;
                }
                7 => value = varp(vm, next(script, &mut pc)?)?.wrapping_mul(100) / 46875,
                8 => value = vm.combat_level,
                9 => {
                    for n in vm.base_levels.iter().take(24) {
                        value = value.wrapping_add(*n);
                    }
                }
                11 => value = vm.run_energy,
                12 => value = vm.weight,
                13 => {
                    let v = varp(vm, next(script, &mut pc)?)?;
                    let bit = next(script, &mut pc)?;
                    value = i32::from(v & 1i32.wrapping_shl(bit as u32) != 0);
                }
                14 => value = vm.varbit(state, next(script, &mut pc)?).map_err(|_| ())?,
                15 => next_mode = 1,
                16 => next_mode = 2,
                17 => next_mode = 3,
                18 => value = vm.location[0],
                19 => value = vm.location[1],
                20 => value = next(script, &mut pc)?,
                _ => {}
            }
            if next_mode == 0 {
                match mode {
                    0 => accumulator = accumulator.wrapping_add(value),
                    1 => accumulator = accumulator.wrapping_sub(value),
                    2 if value != 0 => accumulator = accumulator.wrapping_div(value),
                    3 => accumulator = accumulator.wrapping_mul(value),
                    _ => {}
                }
                mode = 0;
            } else {
                mode = next_mode;
            }
        }
    }
    scripts
        .get(index as usize)
        .ok_or(())
        .and_then(|s| inner(state, vm, s))
        .unwrap_or(-1)
}

pub fn is_true(state: &mut State, vm: &Vm, c: &Component) -> Result<bool, String> {
    let Some(opcodes) = &c.cs1_comparison_opcodes else {
        return Ok(false);
    };
    for (i, &opcode) in opcodes.iter().enumerate() {
        let value = run(state, vm, i as i32, c);
        let operand = *c
            .cs1_comparison_operands
            .as_ref()
            .and_then(|v| v.get(i))
            .ok_or("CS1 operand index")?;
        let pass = match opcode {
            2 => value < operand,
            3 => value > operand,
            4 => value != operand,
            _ => value == operand,
        };
        if !pass {
            return Ok(false);
        }
    }
    Ok(true)
}
