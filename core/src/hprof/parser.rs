use crate::errors::{CoreError, CoreResult};
use byteorder::{BigEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapSummary {
    pub heap_path: String,
    pub total_objects: u64,
    pub total_size_bytes: u64,
    pub classes: Vec<ClassStat>,
    pub generated_at: SystemTime,
    pub header: Option<HprofHeader>,
    pub total_records: u64,
    pub record_stats: Vec<RecordStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClassStat {
    pub name: String,
    pub instances: u64,
    pub total_size_bytes: u64,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapDiff {
    pub before: String,
    pub after: String,
    pub delta_bytes: i64,
    pub delta_objects: i64,
    pub changed_classes: Vec<ClassDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDelta {
    pub name: String,
    pub before_bytes: u64,
    pub after_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapParseJob {
    pub path: String,
    pub include_strings: bool,
    pub max_objects: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HprofHeader {
    pub format: String,
    pub identifier_size: u32,
    pub timestamp_millis: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecordStat {
    pub tag: u8,
    pub name: String,
    pub count: u64,
    pub bytes: u64,
}

pub fn parse_heap(job: &HeapParseJob) -> CoreResult<HeapSummary> {
    let metadata = std::fs::metadata(&job.path)?;
    let file = File::open(&job.path)?;
    let mut reader = BufReader::new(file);
    let header = parse_hprof_header(&mut reader)?;
    let (total_records, record_stats, object_guess) = scan_hprof_records(&mut reader)?;
    let estimated_objects = if object_guess == 0 {
        total_records
    } else {
        object_guess
    };
    let classes = summarize_class_stats(&record_stats, metadata.len());

    Ok(HeapSummary {
        heap_path: job.path.clone(),
        total_objects: estimated_objects,
        total_size_bytes: metadata.len(),
        classes,
        generated_at: SystemTime::now(),
        header: Some(header),
        total_records,
        record_stats,
    })
}

fn parse_hprof_header<R: Read>(reader: &mut R) -> CoreResult<HprofHeader> {
    let mut header_bytes = Vec::with_capacity(64);
    loop {
        let mut byte = [0u8; 1];
        if reader.read(&mut byte)? == 0 {
            return Err(CoreError::HprofParseError {
                phase: "header".into(),
                detail: "unexpected EOF while reading header".into(),
            });
        }
        if byte[0] == 0 {
            break;
        }
        if header_bytes.len() > 1024 {
            return Err(CoreError::HprofParseError {
                phase: "header".into(),
                detail: "header string exceeded 1024 bytes — this may not be an HPROF file".into(),
            });
        }
        header_bytes.push(byte[0]);
    }

    let format = String::from_utf8(header_bytes).map_err(|err| CoreError::HprofParseError {
        phase: "header".into(),
        detail: format!("invalid header string: {err}"),
    })?;
    let identifier_size = reader.read_u32::<BigEndian>()?;
    let timestamp_millis = reader.read_u64::<BigEndian>()?;

    Ok(HprofHeader {
        format,
        identifier_size,
        timestamp_millis,
    })
}

fn scan_hprof_records<R: Read>(reader: &mut R) -> CoreResult<(u64, Vec<RecordStat>, u64)> {
    use std::io::ErrorKind;

    let mut total_records = 0u64;
    let mut stats: HashMap<u8, RecordStat> = HashMap::new();

    loop {
        let tag = match reader.read_u8() {
            Ok(tag) => tag,
            Err(err) if err.kind() == ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };

        let _time_delta = reader.read_u32::<BigEndian>()?;
        let length = reader.read_u32::<BigEndian>()?;

        skip_bytes(reader, length as u64)?;

        let entry = stats.entry(tag).or_insert_with(|| RecordStat {
            tag,
            name: tag_name(tag).into(),
            count: 0,
            bytes: 0,
        });
        entry.count += 1;
        entry.bytes += length as u64;
        total_records += 1;
    }

    let mut record_stats: Vec<RecordStat> = stats.into_values().collect();
    record_stats.sort_by(|a, b| b.bytes.cmp(&a.bytes));

    let object_guess = record_stats
        .iter()
        .filter(|stat| matches!(stat.tag, 0x21..=0x23))
        .map(|stat| stat.count)
        .sum();

    Ok((total_records, record_stats, object_guess))
}

fn skip_bytes<R: Read>(reader: &mut R, len: u64) -> CoreResult<()> {
    let mut remaining = len;
    let mut buffer = [0u8; 8 * 1024];
    while remaining > 0 {
        let to_read = buffer.len().min(remaining as usize);
        reader.read_exact(&mut buffer[..to_read])?;
        remaining -= to_read as u64;
    }
    Ok(())
}

fn tag_name(tag: u8) -> &'static str {
    match tag {
        0x01 => "STRING_IN_UTF8",
        0x02 => "LOAD_CLASS",
        0x03 => "UNLOAD_CLASS",
        0x04 => "STACK_FRAME",
        0x05 => "STACK_TRACE",
        0x06 => "ALLOC_SITES",
        0x07 => "HEAP_SUMMARY",
        0x0A => "START_THREAD",
        0x0B => "END_THREAD",
        0x0C => "HEAP_DUMP",
        0x0D => "HEAP_DUMP_SEGMENT",
        0x0E => "HEAP_DUMP_END",
        0x1C => "CPU_SAMPLES",
        0x1D => "CONTROL_SETTINGS",
        0x1E => "ROOT_UNKNOWN",
        0x1F => "ROOT_JNI_GLOBAL",
        0x20 => "ROOT_JNI_LOCAL",
        0x21 => "INSTANCE_DUMP",
        0x22 => "OBJECT_ARRAY_DUMP",
        0x23 => "PRIMITIVE_ARRAY_DUMP",
        0x24 => "HEAP_DUMP_INFO",
        0x2C => "HEAP_DUMP_SEGMENT_EXT",
        _ => "UNKNOWN",
    }
}

fn summarize_class_stats(record_stats: &[RecordStat], total_size_bytes: u64) -> Vec<ClassStat> {
    if total_size_bytes == 0 {
        return Vec::new();
    }

    let total_bytes = total_size_bytes as f64;
    record_stats
        .iter()
        .filter(|stat| matches!(stat.tag, 0x21..=0x23))
        .map(|stat| {
            let percentage = if stat.bytes == 0 {
                0.0
            } else {
                ((stat.bytes as f64 / total_bytes) * 100.0) as f32
            };
            ClassStat {
                name: stat.name.clone(),
                instances: stat.count,
                total_size_bytes: stat.bytes,
                percentage,
            }
        })
        .collect()
}

impl HeapSummary {
    pub fn placeholder(path: &str) -> Self {
        Self {
            heap_path: path.into(),
            total_objects: 1_234_567,
            total_size_bytes: 2_453_291_008,
            classes: vec![ClassStat {
                name: "java.lang.String".into(),
                instances: 421_032,
                total_size_bytes: 441_651_200,
                percentage: 18.0,
            }],
            generated_at: SystemTime::now(),
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        }
    }
}

impl HeapDiff {
    pub fn placeholder(before: &str, after: &str) -> Self {
        Self {
            before: before.into(),
            after: after.into(),
            delta_bytes: -347_000_000,
            delta_objects: -250_000,
            changed_classes: vec![ClassDelta {
                name: "com.example.UserSession".into(),
                before_bytes: 385_000_000,
                after_bytes: 89_000_000,
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_class_stats_from_records() {
        let record_stats = vec![
            RecordStat {
                tag: 0x21,
                name: "INSTANCE_DUMP".into(),
                count: 10,
                bytes: 512,
            },
            RecordStat {
                tag: 0x07,
                name: "HEAP_SUMMARY".into(),
                count: 1,
                bytes: 128,
            },
            RecordStat {
                tag: 0x22,
                name: "OBJECT_ARRAY_DUMP".into(),
                count: 4,
                bytes: 256,
            },
        ];

        let classes = summarize_class_stats(&record_stats, 1024);
        assert_eq!(2, classes.len());
        assert_eq!("INSTANCE_DUMP", classes[0].name);
        assert_eq!(10, classes[0].instances);
        assert!(classes[0].percentage > 45.0 && classes[0].percentage < 55.0);
        assert_eq!("OBJECT_ARRAY_DUMP", classes[1].name);
    }

    #[test]
    fn handles_zero_length_heaps() {
        let classes = summarize_class_stats(&[], 0);
        assert!(classes.is_empty());
    }
}
