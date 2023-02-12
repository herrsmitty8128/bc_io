

use chrono::{DateTime, TimeZone, FixedOffset, NaiveDateTime};

struct MyStruct {
    time_zone: FixedOffset,
    timestamp: NaiveDateTime,
}

fn main() {
    let x  = TryInto::try_into::<u64>(UNIX_EPOCH);
    
    println!("size of SystemTime = {}", std::mem::size_of::<Instant>());
}