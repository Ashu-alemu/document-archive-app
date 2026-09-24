ALTER TABLE documents ADD COLUMN ocr_text TEXT;

CREATE INDEX IF NOT EXISTS idx_documents_ocr_text ON documents(ocr_text);
