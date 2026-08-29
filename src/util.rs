/// Assumes values is not empty and sorted
pub fn median(values: &[u16]) -> u16 {
    let mid = values.len() / 2;

    if values.len() % 2 == 1 {
        values[mid]
    } else {
        u16::midpoint(values[mid - 1], values[mid])
    }
}
