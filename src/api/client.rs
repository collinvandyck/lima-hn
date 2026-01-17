use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use super::types::{Comment, Feed, HnItem, Story};

const API_BASE: &str = "https://hacker-news.firebaseio.com/v0";
const CACHE_TTL: Duration = Duration::from_secs(60);
const PAGE_SIZE: usize = 30;

struct CacheEntry<T> {
    data: T,
    fetched_at: Instant,
}

impl<T> CacheEntry<T> {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < CACHE_TTL
    }
}

pub struct HnClient {
    http: reqwest::Client,
    item_cache: Arc<RwLock<HashMap<u64, CacheEntry<HnItem>>>>,
}

impl HnClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
            item_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn clear_cache(&self) {
        self.item_cache.write().await.clear();
    }

    pub async fn fetch_feed_ids(&self, feed: Feed) -> Result<Vec<u64>> {
        let url = format!("{}/{}.json", API_BASE, feed.endpoint());
        let ids: Vec<u64> = self
            .http
            .get(&url)
            .send()
            .await
            .context("Failed to fetch feed")?
            .json()
            .await
            .context("Failed to parse feed IDs")?;
        Ok(ids)
    }

    async fn fetch_item(&self, id: u64) -> Result<HnItem> {
        {
            let cache = self.item_cache.read().await;
            if let Some(entry) = cache.get(&id)
                && entry.is_fresh()
            {
                return Ok(entry.data.clone());
            }
        }

        let url = format!("{}/item/{}.json", API_BASE, id);
        let item: HnItem = self
            .http
            .get(&url)
            .send()
            .await
            .context("Failed to fetch item")?
            .json()
            .await
            .context("Failed to parse item")?;

        {
            let mut cache = self.item_cache.write().await;
            cache.insert(
                id,
                CacheEntry {
                    data: item.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }

        Ok(item)
    }

    pub async fn fetch_stories(&self, feed: Feed, page: usize) -> Result<Vec<Story>> {
        let ids = self.fetch_feed_ids(feed).await?;
        let start = page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(ids.len());

        if start >= ids.len() {
            return Ok(vec![]);
        }

        let page_ids = &ids[start..end];
        self.fetch_stories_by_ids(page_ids).await
    }

    pub async fn fetch_stories_by_ids(&self, ids: &[u64]) -> Result<Vec<Story>> {
        let futures: Vec<_> = ids.iter().map(|&id| self.fetch_item(id)).collect();
        let results = futures::future::join_all(futures).await;

        let stories: Vec<Story> = results
            .into_iter()
            .filter_map(|r| r.ok())
            .filter_map(Story::from_item)
            .collect();

        Ok(stories)
    }

    /// Fetches comments using BFS for parallelism, then reorders to DFS for display
    pub async fn fetch_comments_flat(
        &self,
        story: &Story,
        max_depth: usize,
    ) -> Result<Vec<Comment>> {
        use std::collections::HashSet;

        let mut items: HashMap<u64, HnItem> = HashMap::new();
        let mut attempted: HashSet<u64> = HashSet::new();
        let mut to_fetch: Vec<u64> = story.kids.clone();
        let mut depth = 0;

        while !to_fetch.is_empty() && depth <= max_depth {
            let futures: Vec<_> = to_fetch.iter().map(|&id| self.fetch_item(id)).collect();
            let results = futures::future::join_all(futures).await;

            let mut next_fetch = Vec::new();
            for (id, result) in to_fetch.into_iter().zip(results) {
                attempted.insert(id);
                if let Ok(item) = result {
                    if item.deleted.unwrap_or(false) || item.dead.unwrap_or(false) {
                        continue;
                    }
                    if depth < max_depth {
                        next_fetch.extend(&item.kids);
                    }
                    items.insert(id, item);
                }
            }
            to_fetch = next_fetch;
            depth += 1;
        }

        let mut comments = Vec::new();
        let mut stack: Vec<(u64, usize)> = story.kids.iter().rev().map(|&id| (id, 0)).collect();

        while let Some((id, depth)) = stack.pop() {
            if let Some(mut item) = items.remove(&id) {
                // Filter kids that were attempted but not fetched (deleted/dead).
                // Keep kids that were never attempted (beyond max_depth) so UI shows
                // they have replies even if we can't display them.
                item.kids
                    .retain(|kid_id| !attempted.contains(kid_id) || items.contains_key(kid_id));

                // Reverse order so first child is processed first
                for &kid_id in item.kids.iter().rev() {
                    stack.push((kid_id, depth + 1));
                }
                if let Some(comment) = Comment::from_item(item, depth) {
                    comments.push(comment);
                }
            }
        }

        Ok(comments)
    }
}

impl Default for HnClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for HnClient {
    fn clone(&self) -> Self {
        Self {
            http: self.http.clone(),
            item_cache: Arc::clone(&self.item_cache),
        }
    }
}

impl Clone for HnItem {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            item_type: self.item_type.clone(),
            by: self.by.clone(),
            time: self.time,
            text: self.text.clone(),
            url: self.url.clone(),
            score: self.score,
            title: self.title.clone(),
            descendants: self.descendants,
            kids: self.kids.clone(),
            parent: self.parent,
            deleted: self.deleted,
            dead: self.dead,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[tokio::test]
    async fn test_client_creation() {
        let client = HnClient::new();
        // Just verify it doesn't panic
        drop(client);
    }

    /// Verifies that deleted children (attempted but not fetched) are filtered
    /// out of the kids array.
    #[test]
    fn test_deleted_children_filtered_from_kids() {
        let mut items: HashMap<u64, HnItem> = HashMap::new();
        let mut attempted: HashSet<u64> = HashSet::new();

        // Parent comment with kids [2, 3] - child 3 was attempted but deleted
        items.insert(
            1,
            HnItem {
                id: 1,
                item_type: Some("comment".to_string()),
                by: Some("parent".to_string()),
                time: Some(1700000000),
                text: Some("Parent comment".to_string()),
                url: None,
                score: None,
                title: None,
                descendants: None,
                kids: vec![2, 3],
                parent: None,
                deleted: None,
                dead: None,
            },
        );

        // Child 2 exists and was fetched
        items.insert(
            2,
            HnItem {
                id: 2,
                item_type: Some("comment".to_string()),
                by: Some("child".to_string()),
                time: Some(1700000000),
                text: Some("Child comment".to_string()),
                url: None,
                score: None,
                title: None,
                descendants: None,
                kids: vec![],
                parent: Some(1),
                deleted: None,
                dead: None,
            },
        );

        // Both children were attempted
        attempted.insert(2);
        attempted.insert(3);
        // But child 3 was deleted, so it's not in items

        // Simulate the filtering logic from fetch_comments_flat
        let mut comments = Vec::new();
        let mut stack: Vec<(u64, usize)> = vec![(1, 0)];

        while let Some((id, depth)) = stack.pop() {
            if let Some(mut item) = items.remove(&id) {
                item.kids
                    .retain(|kid_id| !attempted.contains(kid_id) || items.contains_key(kid_id));

                for &kid_id in item.kids.iter().rev() {
                    stack.push((kid_id, depth + 1));
                }
                if let Some(comment) = Comment::from_item(item, depth) {
                    comments.push(comment);
                }
            }
        }

        assert_eq!(comments.len(), 2);

        // Parent should only have 1 kid now (child 3 was filtered out)
        let parent = &comments[0];
        assert_eq!(parent.id, 1);
        assert_eq!(parent.kids.len(), 1);
        assert_eq!(parent.kids[0], 2);
    }

    /// Verifies that a comment whose only child was deleted ends up with
    /// an empty kids array (showing [ ] instead of [+] in the UI).
    #[test]
    fn test_all_children_deleted_results_in_empty_kids() {
        let mut items: HashMap<u64, HnItem> = HashMap::new();
        let mut attempted: HashSet<u64> = HashSet::new();

        items.insert(
            1,
            HnItem {
                id: 1,
                item_type: Some("comment".to_string()),
                by: Some("author".to_string()),
                time: Some(1700000000),
                text: Some("Comment with deleted reply".to_string()),
                url: None,
                score: None,
                title: None,
                descendants: None,
                kids: vec![999], // This child was deleted
                parent: None,
                deleted: None,
                dead: None,
            },
        );

        // Child 999 was attempted but deleted (not in items)
        attempted.insert(999);

        let mut item = items.remove(&1).unwrap();
        item.kids
            .retain(|kid_id| !attempted.contains(kid_id) || items.contains_key(kid_id));

        let comment = Comment::from_item(item, 0).unwrap();

        // Kids should be empty since child 999 was attempted but deleted
        assert!(comment.kids.is_empty());
    }

    /// Verifies that children beyond max_depth (never attempted) are kept in
    /// the kids array so the UI shows they have replies.
    #[test]
    fn test_children_beyond_max_depth_kept_in_kids() {
        let mut items: HashMap<u64, HnItem> = HashMap::new();
        let attempted: HashSet<u64> = HashSet::new(); // Nothing beyond this comment was attempted

        // Comment at max_depth with a child that was never fetched
        items.insert(
            1,
            HnItem {
                id: 1,
                item_type: Some("comment".to_string()),
                by: Some("deep_commenter".to_string()),
                time: Some(1700000000),
                text: Some("Comment at max depth".to_string()),
                url: None,
                score: None,
                title: None,
                descendants: None,
                kids: vec![999], // This child exists but wasn't fetched (beyond max_depth)
                parent: None,
                deleted: None,
                dead: None,
            },
        );

        // Child 999 was NOT attempted (beyond max_depth)

        let mut item = items.remove(&1).unwrap();
        item.kids
            .retain(|kid_id| !attempted.contains(kid_id) || items.contains_key(kid_id));

        let comment = Comment::from_item(item, 0).unwrap();

        // Kids should still contain 999 since it was never attempted
        // (exists beyond max_depth, not deleted)
        assert_eq!(comment.kids.len(), 1);
        assert_eq!(comment.kids[0], 999);
    }
}
