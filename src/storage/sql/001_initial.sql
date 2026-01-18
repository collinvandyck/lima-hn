-- Stories table
CREATE TABLE IF NOT EXISTS stories (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    url TEXT,
    score INTEGER NOT NULL,
    by TEXT NOT NULL,
    time INTEGER NOT NULL,
    descendants INTEGER NOT NULL,
    kids TEXT,
    fetched_at INTEGER NOT NULL,
    bookmarked_at INTEGER,
    read_at INTEGER,
    last_viewed_at INTEGER
);

-- Comments table
CREATE TABLE IF NOT EXISTS comments (
    id INTEGER PRIMARY KEY,
    story_id INTEGER NOT NULL,
    parent_id INTEGER,
    text TEXT NOT NULL,
    by TEXT NOT NULL,
    time INTEGER NOT NULL,
    depth INTEGER NOT NULL,
    kids TEXT,
    fetched_at INTEGER NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id)
);

-- Feeds table (cached feed orderings)
CREATE TABLE IF NOT EXISTS feeds (
    feed_type TEXT NOT NULL,
    story_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    fetched_at INTEGER NOT NULL,
    PRIMARY KEY (feed_type, position)
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_stories_fetched_at ON stories(fetched_at);
CREATE INDEX IF NOT EXISTS idx_stories_bookmarked ON stories(bookmarked_at) WHERE bookmarked_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_stories_read ON stories(read_at) WHERE read_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_stories_last_viewed ON stories(last_viewed_at) WHERE last_viewed_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_comments_story_id ON comments(story_id);
CREATE INDEX IF NOT EXISTS idx_comments_fetched_at ON comments(fetched_at);
CREATE INDEX IF NOT EXISTS idx_comments_parent_id ON comments(parent_id);

CREATE INDEX IF NOT EXISTS idx_feeds_story ON feeds(story_id);
CREATE INDEX IF NOT EXISTS idx_feeds_fetched ON feeds(feed_type, fetched_at);
