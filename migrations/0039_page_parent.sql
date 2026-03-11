ALTER TABLE posts ADD COLUMN parent_id UUID REFERENCES posts(id) ON DELETE SET NULL;
CREATE INDEX posts_parent_id_idx ON posts(parent_id);
