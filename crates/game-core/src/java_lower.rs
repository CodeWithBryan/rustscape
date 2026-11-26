/*
 * Copyright (c) 2003, 2013, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */

/*
 * Copyright (c) 1999, 2016, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */

/*
 *
 * (C) Copyright Taligent, Inc. 1996, 1997 - All Rights Reserved
 * (C) Copyright IBM Corp. 1996 - 2002 - All Rights Reserved
 *
 * The original version of this source code and documentation
 * is copyrighted and owned by Taligent, Inc., a wholly-owned
 * subsidiary of IBM. These materials are provided under terms
 * of a License Agreement between Taligent and Sun. This technology
 * is protected by multiple US and International patents.
 *
 * This notice and attribution to Taligent may not be removed.
 * Taligent is a registered trademark of Taligent, Inc.
 */

// SPDX-License-Identifier: GPL-2.0-only WITH Classpath-exception-2.0
//! JDK 11 ROOT lowercase, including its contextual sigma word segmentation.
#[path = "java_lower_data.rs"]
mod data;
fn category(code: u32) -> i32 {
    let index = data::WORDS.partition_point(|&(first, _, _)| first <= code) - 1;
    data::WORDS[index].2
}
fn cased(code: u32) -> bool {
    let index = data::CASED.partition_point(|&(first, _)| first <= code);
    index > 0 && code <= data::CASED[index - 1].1
}
struct Words<'a> {
    units: &'a [u16],
    pos: usize,
    cached: Option<usize>,
}
const DONE: u32 = 0xffff;
fn code_at(units: &[u16], at: usize) -> (u32, usize) {
    let Some(&first) = units.get(at) else {
        return (DONE, 1);
    };
    if (0xd800..=0xdbff).contains(&first)
        && let Some(&second) = units.get(at + 1)
        && (0xdc00..=0xdfff).contains(&second)
    {
        return (
            0x10000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second) - 0xdc00,
            2,
        );
    }
    (u32::from(first), 1)
}
fn code_before(units: &[u16], at: usize) -> (u32, usize) {
    if at == 0 {
        return (DONE, 1);
    }
    if (0xdc00..=0xdfff).contains(&units[at - 1])
        && at > 1
        && (0xd800..=0xdbff).contains(&units[at - 2])
    {
        code_at(units, at - 2)
    } else {
        (u32::from(units[at - 1]), 1)
    }
}
impl<'a> Words<'a> {
    fn new(units: &'a [u16]) -> Self {
        Self {
            units,
            pos: 0,
            cached: None,
        }
    }
    fn next_char(&mut self) -> u32 {
        let next = self.pos + code_at(self.units, self.pos).1;
        if next >= self.units.len() {
            DONE
        } else {
            self.pos = next;
            code_at(self.units, self.pos).0
        }
    }
    fn previous_char(&mut self) -> u32 {
        if self.pos == 0 {
            DONE
        } else {
            let (code, len) = code_before(self.units, self.pos);
            self.pos -= len;
            code
        }
    }
    fn next_index(&self) -> usize {
        (self.pos + code_at(self.units, self.pos).1).min(self.units.len())
    }
    fn forward(&mut self) -> Option<usize> {
        if self.pos == self.units.len() {
            return None;
        }
        let mut result = self.next_index();
        let mut lookahead = 0;
        let mut state = 1;
        let mut code = code_at(self.units, self.pos).0;
        while code != DONE && state != 0 {
            let cat = category(code);
            if cat != -1 {
                state = data::STATES[state * data::CATEGORIES + cat as usize]
            }
            if data::LOOK[state] {
                if data::END[state] {
                    result = lookahead
                } else {
                    lookahead = self.next_index()
                }
            } else if data::END[state] {
                result = self.next_index()
            }
            code = self.next_char();
        }
        if code == DONE && lookahead == self.units.len() {
            result = lookahead
        }
        self.pos = result;
        Some(result)
    }
    fn backward(&mut self) -> usize {
        let mut state = 1;
        let mut cat = 0;
        let mut last = 0;
        let mut code = code_at(self.units, self.pos).0;
        while code != DONE && state != 0 {
            last = cat;
            cat = category(code);
            if cat != -1 {
                state = data::BACK[state * data::CATEGORIES + cat as usize]
            }
            code = self.previous_char();
        }
        if code != DONE {
            self.next_char();
            if last != -1 {
                self.next_char();
            }
        }
        self.pos
    }
    fn is_boundary(&mut self, at: usize) -> bool {
        if at == 0 {
            return true;
        }
        let offset = at - 1;
        self.pos = offset;
        if offset == 0 {
            self.cached = self.forward();
            return self.cached == Some(at);
        }
        let mut result = if self.cached.is_none_or(|cached| cached >= offset) {
            Some(self.backward())
        } else {
            self.pos = self.cached.unwrap();
            self.cached
        };
        while result.is_some_and(|value| value <= offset) {
            result = self.forward();
        }
        self.cached = result;
        result == Some(at)
    }
}
/// Forward ROOT word boundaries as UTF-16 offsets, for differential validation.
/// Other locales and general BreakIterator APIs are outside this narrow helper.
pub fn root_word_boundaries(text: &str) -> Vec<usize> {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let mut words = Words::new(&units);
    let mut result = vec![0];
    while let Some(at) = words.forward() {
        result.push(at)
    }
    result
}
fn final_cased(units: &[u16], index: usize) -> bool {
    let mut words = Words::new(units);
    let mut at = index;
    while !words.is_boundary(at) {
        let (code, len) = code_before(units, at);
        if cased(code) {
            at = index + 1;
            while at < units.len() && !words.is_boundary(at) {
                let (code, len) = code_at(units, at);
                if cased(code) {
                    return false;
                }
                at += len;
            }
            return true;
        }
        at -= len;
    }
    false
}
pub fn lowercase_root(text: &str) -> String {
    if text.is_ascii() {
        return text.to_ascii_lowercase();
    }
    let units = text
        .contains('Σ')
        .then(|| text.encode_utf16().collect::<Vec<_>>());
    let mut result = String::with_capacity(text.len());
    let mut offset = 0;
    for c in text.chars() {
        if c == 'Σ' {
            result.push(if final_cased(units.as_ref().unwrap(), offset) {
                'ς'
            } else {
                'σ'
            })
        } else if let Ok(index) = data::LOWER.binary_search_by_key(&(c as u32), |&(code, _)| code) {
            result.push_str(data::LOWER[index].1)
        } else {
            result.push(c)
        }
        offset += c.len_utf16();
    }
    result
}
