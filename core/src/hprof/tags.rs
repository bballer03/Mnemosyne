pub const TAG_STRING_IN_UTF8: u8 = 0x01;
pub const TAG_LOAD_CLASS: u8 = 0x02;
pub const TAG_UNLOAD_CLASS: u8 = 0x03;
pub const TAG_STACK_FRAME: u8 = 0x04;
pub const TAG_STACK_TRACE: u8 = 0x05;
pub const TAG_ALLOC_SITES: u8 = 0x06;
pub const TAG_HEAP_SUMMARY: u8 = 0x07;
pub const TAG_START_THREAD: u8 = 0x0A;
pub const TAG_END_THREAD: u8 = 0x0B;
pub const TAG_HEAP_DUMP: u8 = 0x0C;
pub const TAG_CPU_SAMPLES: u8 = 0x0D;
pub const TAG_CONTROL_SETTINGS: u8 = 0x0E;
pub const TAG_HEAP_DUMP_SEGMENT: u8 = 0x1C;
pub const TAG_HEAP_DUMP_END: u8 = 0x2C;

pub const SUB_ROOT_JNI_GLOBAL: u8 = 0x01;
pub const SUB_ROOT_JNI_LOCAL: u8 = 0x02;
pub const SUB_ROOT_JAVA_FRAME: u8 = 0x03;
pub const SUB_ROOT_NATIVE_STACK: u8 = 0x04;
pub const SUB_ROOT_STICKY_CLASS: u8 = 0x05;
pub const SUB_ROOT_THREAD_BLOCK: u8 = 0x06;
pub const SUB_ROOT_MONITOR_USED: u8 = 0x07;
pub const SUB_ROOT_THREAD_OBJECT: u8 = 0x08;
pub const SUB_ROOT_UNKNOWN: u8 = 0x09;
pub const SUB_ROOT_INTERNED_STRING: u8 = 0x0A;
pub const SUB_ROOT_FINALIZING: u8 = 0x0B;
pub const SUB_ROOT_DEBUGGER: u8 = 0x0C;
pub const SUB_ROOT_REFERENCE_CLEANUP: u8 = 0x0D;
pub const SUB_ROOT_VM_INTERNAL: u8 = 0x0E;
pub const SUB_ROOT_JNI_MONITOR: u8 = 0x0F;
pub const SUB_ROOT_UNREACHABLE: u8 = 0x10;
pub const SUB_CLASS_DUMP: u8 = 0x20;
pub const SUB_INSTANCE_DUMP: u8 = 0x21;
pub const SUB_OBJ_ARRAY_DUMP: u8 = 0x22;
pub const SUB_PRIM_ARRAY_DUMP: u8 = 0x23;
pub const SUB_HEAP_DUMP_INFO: u8 = 0xFE;
pub const SUB_PRIMITIVE_ARRAY_NODATA: u8 = 0xFF;

pub fn tag_name(tag: u8) -> &'static str {
    match tag {
        TAG_STRING_IN_UTF8 => "STRING_IN_UTF8",
        TAG_LOAD_CLASS => "LOAD_CLASS",
        TAG_UNLOAD_CLASS => "UNLOAD_CLASS",
        TAG_STACK_FRAME => "STACK_FRAME",
        TAG_STACK_TRACE => "STACK_TRACE",
        TAG_ALLOC_SITES => "ALLOC_SITES",
        TAG_HEAP_SUMMARY => "HEAP_SUMMARY",
        TAG_START_THREAD => "START_THREAD",
        TAG_END_THREAD => "END_THREAD",
        TAG_HEAP_DUMP => "HEAP_DUMP",
        TAG_CPU_SAMPLES => "CPU_SAMPLES",
        TAG_CONTROL_SETTINGS => "CONTROL_SETTINGS",
        TAG_HEAP_DUMP_SEGMENT => "HEAP_DUMP_SEGMENT",
        TAG_HEAP_DUMP_END => "HEAP_DUMP_END",
        _ => "UNKNOWN",
    }
}
