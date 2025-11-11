use std::cmp::min;

pub fn chunk<T: Clone>(list: Vec<T>, size: usize) -> Vec<Vec<T>> {
    // If the list is shorter than the size, then we have to make fewer lists
    let real_size = min(list.len(), size);

    let mut chunks: Vec<Vec<T>> = Vec::with_capacity(real_size);
    for _ in 0..real_size {
        chunks.push(Vec::new())
    }
    for (i, entry) in list.iter().enumerate() {
        chunks
            .get_mut(i.rem_euclid(size))
            .expect("unknown chunk")
            .push(entry.clone());
    }
    chunks
}
