use itertools::Itertools;

#[derive(Debug)]
struct Chunk {
    start_id: u32,
    start_index: usize,
}

#[derive(Default)]
pub struct SparseVec<T: Chunkable> {
    chunks: Vec<Chunk>,
    items: Vec<T>,
}

impl<T: Chunkable> SparseVec<T> {
    pub fn get_item_index(&self, id: u32) -> Option<usize> {
        match self.chunks.binary_search_by(|x| x.start_id.cmp(&id)) {
            Ok(x) => Some(self.chunks[x].start_index),
            Err(x) => {
                if x > 0 {
                    let chunk = &self.chunks[x - 1];
                    Some(chunk.start_index + (id - chunk.start_id) as usize)
                } else {
                    None
                }
            }
        }
    }

    pub fn get(&self, id: u32) -> Option<&T> {
        let index = self.get_item_index(id)?;
        let candidate = self.items.get(index)?;
        if candidate.get_id() == id {
            Some(candidate)
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        let index = self.get_item_index(id)?;
        let candidate = self.items.get_mut(index)?;
        if candidate.get_id() == id {
            Some(candidate)
        } else {
            None
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.items.iter_mut()
    }
}

impl<T: Chunkable> FromIterator<T> for SparseVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let items: Vec<T> = iter.into_iter().sorted_by_key(|item| item.get_id()).collect();
        let mut chunks: Vec<Chunk> = vec![Chunk {
            start_id: items[0].get_id(),
            start_index: 0,
        }];
        chunks.extend(
            items
                .iter()
                .map(|x| x.get_id())
                .enumerate()
                .tuple_windows()
                .filter(|(a, b)| b.1 - a.1 != 1)
                .map(|(_, b)| Chunk {
                    start_id: b.1,
                    start_index: b.0,
                }),
        );

        Self { chunks, items }
    }
}

impl<T: Chunkable> IntoIterator for SparseVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T: Chunkable> IntoIterator for &'a SparseVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: Chunkable> IntoIterator for &'a mut SparseVec<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

pub trait Chunkable {
    fn get_id(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq<(u32, usize)> for Chunk {
        fn eq(&self, (start_id, start_index): &(u32, usize)) -> bool {
            self.start_id == *start_id && self.start_index == *start_index
        }
    }

    struct TestItem {
        id: u32,
    }

    impl Chunkable for TestItem {
        fn get_id(&self) -> u32 {
            self.id
        }
    }

    #[test]
    fn test_sparse_vec() {
        let ids = [1, 2, 3, 50, 51, 52, 65, 70, 100, 101];
        let sparse_vec: SparseVec<TestItem> = ids.into_iter().map(|id| TestItem { id }).collect();
        assert_eq!(sparse_vec.items.len(), 10);
        assert_eq!(sparse_vec.chunks.len(), 5);
        assert_eq!(sparse_vec.chunks[0], (1, 0));
        assert_eq!(sparse_vec.chunks[1], (50, 3));
        assert_eq!(sparse_vec.chunks[2], (65, 6));
        assert_eq!(sparse_vec.chunks[3], (70, 7));
        assert_eq!(sparse_vec.chunks[4], (100, 8));

        let test_ids = [Ok(3), Ok(1), Ok(65), Ok(101), Err(0), Err(5), Err(69), Err(102)];
        for test in test_ids.into_iter() {
            match test {
                Ok(id) => {
                    let block = sparse_vec.get(id).unwrap();
                    assert_eq!(block.id, id);
                }
                Err(id) => {
                    let block = sparse_vec.get(id);
                    assert!(block.is_none());
                }
            }
        }
    }
}
