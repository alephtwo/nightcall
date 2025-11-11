pub fn chunk<T: Clone>(list: Vec<T>, size: usize) -> Vec<Vec<T>> {
    let mut chunks: Vec<Vec<T>> = Vec::with_capacity(size);
    for _ in 0..size {
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
