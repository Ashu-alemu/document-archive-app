ALTER TABLE documents ADD COLUMN ocr_status TEXT NOT NULL DEFAULT 'skipped';

CREATE INDEX IF NOT EXISTS idx_documents_ocr_status ON documents(ocr_status);
