CREATE TABLE IF NOT EXISTS categories (
  name TEXT PRIMARY KEY COLLATE NOCASE,
  created_at TEXT NOT NULL
);

INSERT OR IGNORE INTO categories (name, created_at)
SELECT DISTINCT trim(document_type), strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM documents
WHERE trim(document_type) <> '';

INSERT OR IGNORE INTO categories (name, created_at) VALUES
  ('Letter', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Receipt', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Bill / Invoice', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Meeting', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Report', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Certificate', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Contract / Agreement', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Payment Document', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Photo / Scan', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  ('Other', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));

CREATE TABLE IF NOT EXISTS tags (
  name TEXT PRIMARY KEY COLLATE NOCASE,
  created_at TEXT NOT NULL
);

WITH RECURSIVE tag_parts(document_id, value, remaining) AS (
  SELECT id, '', coalesce(tags, '') || ','
  FROM documents
  WHERE trim(coalesce(tags, '')) <> ''
  UNION ALL
  SELECT document_id,
    trim(substr(remaining, 1, instr(remaining, ',') - 1)),
    substr(remaining, instr(remaining, ',') + 1)
  FROM tag_parts
  WHERE remaining <> ''
)
INSERT OR IGNORE INTO tags (name, created_at)
SELECT DISTINCT value, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM tag_parts
WHERE value <> '';

WITH RECURSIVE tag_parts(document_id, value, remaining) AS (
  SELECT id, '', coalesce(tags, '') || ','
  FROM documents
  WHERE trim(coalesce(tags, '')) <> ''
  UNION ALL
  SELECT document_id,
    trim(substr(remaining, 1, instr(remaining, ',') - 1)),
    substr(remaining, instr(remaining, ',') + 1)
  FROM tag_parts
  WHERE remaining <> ''
),
normalized_tags AS (
  SELECT document_id, group_concat(value, ', ') AS value
  FROM (
    SELECT document_id, value
    FROM tag_parts
    WHERE value <> ''
    GROUP BY document_id, value COLLATE NOCASE
    ORDER BY value COLLATE NOCASE
  )
  GROUP BY document_id
)
UPDATE documents
SET tags = (SELECT value FROM normalized_tags WHERE document_id = documents.id)
WHERE id IN (SELECT document_id FROM normalized_tags);

CREATE TABLE IF NOT EXISTS document_favorites (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  PRIMARY KEY (user_id, document_id)
);

CREATE TABLE IF NOT EXISTS recent_documents (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  last_opened_at TEXT NOT NULL,
  PRIMARY KEY (user_id, document_id)
);

CREATE INDEX IF NOT EXISTS idx_documents_browse
  ON documents(status, document_date DESC, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_documents_category_date
  ON documents(status, document_type, document_date DESC);
CREATE INDEX IF NOT EXISTS idx_documents_updated
  ON documents(status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_documents_title_browse
  ON documents(status, title COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_documents_category_browse
  ON documents(status, document_type COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_documents_size_browse
  ON documents(status, file_size);
CREATE INDEX IF NOT EXISTS idx_favorites_user_created
  ON document_favorites(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_favorites_document
  ON document_favorites(document_id);
CREATE INDEX IF NOT EXISTS idx_recent_user_opened
  ON recent_documents(user_id, last_opened_at DESC);
CREATE INDEX IF NOT EXISTS idx_recent_document
  ON recent_documents(document_id);
