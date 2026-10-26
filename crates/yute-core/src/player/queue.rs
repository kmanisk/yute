use crate::backend::models::VideoCard;
use std::collections::VecDeque;

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct PlayQueue {
    items: VecDeque<VideoCard>,
}

#[allow(dead_code)]
impl PlayQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_back(&mut self, video: VideoCard) {
        self.items.push_back(video);
    }

    pub fn push_front(&mut self, video: VideoCard) {
        self.items.push_front(video);
    }

    pub fn pop_next(&mut self) -> Option<VideoCard> {
        self.items.pop_front()
    }

    pub fn items(&self) -> &VecDeque<VideoCard> {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn remove(&mut self, index: usize) -> Option<VideoCard> {
        self.items.remove(index)
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}
