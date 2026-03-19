/// 建表SQL — 所有本地表
pub const CREATE_ALL: &str = r#"

-- 会话表
CREATE TABLE IF NOT EXISTS conversations (
    id              TEXT PRIMARY KEY,
    conv_type       TEXT NOT NULL DEFAULT 'direct',
    peer_hasn_id    TEXT,
    peer_star_id    TEXT,
    peer_name       TEXT,
    peer_type       TEXT,
    peer_avatar_url TEXT,
    last_message_at TEXT,
    last_message_preview TEXT,
    message_count   INTEGER DEFAULT 0,
    unread_count    INTEGER DEFAULT 0,
    status          TEXT DEFAULT 'active',
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_conv_last_msg ON conversations(last_message_at DESC);

-- 消息表
CREATE TABLE IF NOT EXISTS messages (
    id              INTEGER,
    local_id        TEXT NOT NULL UNIQUE,
    conversation_id TEXT NOT NULL,
    from_id         TEXT NOT NULL,
    from_star_id    TEXT,
    from_type       INTEGER NOT NULL DEFAULT 1,
    content         TEXT NOT NULL,
    content_type    INTEGER DEFAULT 1,
    metadata        TEXT,
    reply_to        INTEGER,
    status          INTEGER DEFAULT 1,
    send_status     TEXT DEFAULT 'sending',
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_msg_conv ON messages(conversation_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_msg_server_id ON messages(id) WHERE id IS NOT NULL AND id > 0;
CREATE INDEX IF NOT EXISTS idx_msg_send_status ON messages(send_status) WHERE send_status IN ('sending', 'failed');

-- 联系人表
CREATE TABLE IF NOT EXISTS contacts (
    id              INTEGER PRIMARY KEY,
    peer_hasn_id    TEXT NOT NULL,
    peer_star_id    TEXT NOT NULL,
    peer_name       TEXT NOT NULL,
    peer_type       TEXT NOT NULL DEFAULT 'human',
    peer_avatar_url TEXT,
    relation_type   TEXT DEFAULT 'social',
    trust_level     INTEGER DEFAULT 1,
    nickname        TEXT,
    tags            TEXT,
    status          TEXT DEFAULT 'pending',
    connected_at    TEXT,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(peer_hasn_id, relation_type)
);

-- 同步游标表
CREATE TABLE IF NOT EXISTS sync_cursors (
    conversation_id TEXT PRIMARY KEY,
    last_synced_id  INTEGER NOT NULL,
    synced_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 认证信息表
CREATE TABLE IF NOT EXISTS auth_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

"#;
