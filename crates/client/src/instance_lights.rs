//! Point lights of an instanced scene: the tail of RT4
//! `SceneGraph.readDynamicTerrain`, which reads every light of a chunk's
//! source map file and keeps those standing in the copied chunk on its source
//! level, turned with the chunk.
//!
//! As in the original, a light keeps its source level even when its chunk is
//! placed on another level, and its span profile is not turned.
use crate::instance::Chunk;
use std::collections::BTreeMap;

/// `rotateXFine`/`rotateYFine`: a fine position inside an 8-tile chunk (0 to
/// 1023 on each axis) after the chunk is turned.
pub fn rotate_fine(rotation: i32, x: i32, y: i32) -> [i32; 2] {
    match rotation & 3 {
        0 => [x, y],
        1 => [y, 1023 - x],
        2 => [1023 - x, 1023 - y],
        _ => [1023 - y, x],
    }
}

/// The scene's lights in the order the original adds them (levels, then chunk
/// columns, then chunk rows, then the source file's order), as light records
/// whose x and y have been replaced by scene-local fine coordinates.
/// `chunks` is the 4 x 13 x 13 table; `sources` maps a packed region ID to the
/// raw light records of that region's map file. Records too short to hold
/// their coordinates are skipped.
pub fn lights(chunks: &[i32], sources: &BTreeMap<i32, Vec<Vec<u8>>>) -> Vec<Vec<u8>> {
    let mut out = vec![];
    for level in 0..4 {
        for chunk_x in 0..13 {
            for chunk_y in 0..13 {
                let Some((chunk, records)) = chunks
                    .get((level * 13 + chunk_x) * 13 + chunk_y)
                    .copied()
                    .and_then(Chunk::unpack)
                    .and_then(|chunk| Some((chunk, sources.get(&chunk.region())?)))
                else {
                    continue;
                };
                let from = [(chunk.zone[0] & 7) * 8, (chunk.zone[1] & 7) * 8];
                for record in records {
                    let Some(&[flags, x0, x1, y0, y1]) = record.first_chunk::<5>() else {
                        continue;
                    };
                    let x = i32::from(u16::from_be_bytes([x0, x1]));
                    let y = i32::from(u16::from_be_bytes([y0, y1]));
                    if usize::from(flags & 7) != chunk.level
                        || !(from[0]..from[0] + 8).contains(&(x >> 7))
                        || !(from[1]..from[1] + 8).contains(&(y >> 7))
                    {
                        continue;
                    }
                    let [dx, dy] = rotate_fine(chunk.rotation, x & 0x3ff, y & 0x3ff);
                    let placed = [
                        dx + ((chunk_x as i32 * 8) << 7),
                        dy + ((chunk_y as i32 * 8) << 7),
                    ];
                    let mut record = record.clone();
                    record[1..3].copy_from_slice(&(placed[0] as u16).to_be_bytes());
                    record[3..5].copy_from_slice(&(placed[1] as u16).to_be_bytes());
                    out.push(record);
                }
            }
        }
    }
    out
}

/// The same records with x and y measured from another origin, `tiles` tiles
/// south-west of the scene: how they are kept on the first map region of the
/// regions an instanced scene is laid out over. None if a coordinate would not
/// fit the record's 16 bits.
pub fn rebased(records: Vec<Vec<u8>>, tiles: [i32; 2]) -> Option<Vec<Vec<u8>>> {
    records
        .into_iter()
        .map(|mut record| {
            if record.len() < 5 {
                return None;
            }
            for (axis, tiles) in tiles.into_iter().enumerate() {
                let at = 1 + axis * 2;
                let value =
                    i32::from(u16::from_be_bytes([record[at], record[at + 1]])) + tiles * 128;
                record[at..at + 2].copy_from_slice(&u16::try_from(value).ok()?.to_be_bytes());
            }
            Some(record)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(level: i32, zone: [i32; 2], rotation: i32) -> i32 {
        (level << 24) | (zone[0] << 14) | (zone[1] << 3) | (rotation << 1)
    }
    fn chunks(entries: &[(usize, usize, usize, i32)]) -> Vec<i32> {
        let mut chunks = vec![-1; 4 * 13 * 13];
        for &(level, x, y, packed) in entries {
            chunks[(level * 13 + x) * 13 + y] = packed;
        }
        chunks
    }
    /// A light record: flags, x, y, z, radius 0 with its one span, a colour
    /// and a preset.
    fn record(flags: u8, x: u16, y: u16) -> Vec<u8> {
        let mut out = vec![flags];
        out.extend(x.to_be_bytes());
        out.extend(y.to_be_bytes());
        out.extend([0, 50, 0, 0, 1, 0, 9, 2]);
        out
    }
    fn position(record: &[u8]) -> [u16; 2] {
        [
            u16::from_be_bytes([record[1], record[2]]),
            u16::from_be_bytes([record[3], record[4]]),
        ]
    }

    // Transcription tests of `readDynamicTerrain` lines 5442-5463.
    #[test]
    fn a_light_turns_with_its_chunk_about_the_chunk_s_fine_extent() {
        // Region (50, 50) is zone (400..408, 400..408); its chunk (1, 2)
        // covers fine x 1024..2047 and y 2048..3071.
        let source = BTreeMap::from([((50 << 8) | 50, vec![record(0, 1024 + 100, 2048 + 300)])]);
        for (rotation, expected) in [
            (0, [100, 300]),
            (1, [300, 1023 - 100]),
            (2, [1023 - 100, 1023 - 300]),
            (3, [1023 - 300, 100]),
        ] {
            let table = chunks(&[(0, 4, 6, pack(0, [401, 402], rotation))]);
            let placed = lights(&table, &source);
            assert_eq!(placed.len(), 1);
            assert_eq!(
                position(&placed[0]),
                [4 * 1024 + expected[0], 6 * 1024 + expected[1]],
                "rotation {rotation}"
            );
            // Everything after the coordinates is carried unchanged.
            assert_eq!(placed[0][5..], record(0, 0, 0)[5..]);
        }
    }

    #[test]
    fn only_lights_in_the_copied_chunk_on_its_source_level_are_kept() {
        let source = BTreeMap::from([(
            (50 << 8) | 50,
            vec![
                record(1, 1024, 2048),     // source level 1, inside
                record(0, 1024, 2048),     // source level 0
                record(1 | 8, 2047, 3071), // level 1 with a flag, last fine unit
                record(1, 2048, 2048),     // next chunk east
                record(1, 1024, 2047),     // next chunk south
                vec![1, 4],                // truncated
            ],
        )]);
        // The chunk is copied from source level 1 onto scene level 2: the
        // lights keep their own level byte.
        let table = chunks(&[(2, 0, 0, pack(1, [401, 402], 0))]);
        let placed = lights(&table, &source);
        assert_eq!(placed.len(), 2);
        assert_eq!((placed[0][0], position(&placed[0])), (1, [0, 0]));
        assert_eq!((placed[1][0], position(&placed[1])), (9, [1023, 1023]));
        // A chunk whose source region has no file brings nothing.
        assert!(lights(&chunks(&[(0, 0, 0, pack(0, [8, 8], 0))]), &source).is_empty());
    }

    #[test]
    fn lights_come_in_level_column_row_order_and_once_per_copy() {
        let source = BTreeMap::from([((50 << 8) | 50, vec![record(0, 10, 20), record(0, 30, 40)])]);
        let from = pack(0, [400, 400], 0);
        let table = chunks(&[(1, 0, 0, from), (0, 2, 1, from), (0, 1, 5, from)]);
        let placed: Vec<_> = lights(&table, &source)
            .iter()
            .map(|r| position(r))
            .collect();
        assert_eq!(
            placed,
            [
                [1024 + 10, 5 * 1024 + 20],
                [1024 + 30, 5 * 1024 + 40],
                [2048 + 10, 1024 + 20],
                [2048 + 30, 1024 + 40],
                [10, 20],
                [30, 40],
            ]
        );
    }

    #[test]
    fn records_are_rebased_onto_the_first_region() {
        let moved = rebased(vec![record(0, 10, 20)], [63, 2]).unwrap();
        assert_eq!(position(&moved[0]), [63 * 128 + 10, 2 * 128 + 20]);
        assert!(rebased(vec![record(0, 65535, 0)], [1, 0]).is_none());
    }
}
