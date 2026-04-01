/// 建表SQL — 所有本地表 (HASN Protocol v4.0)
pub const CREATE_ALL: &str = r#"

-- 会话表 (对齐 02 协议 4.2 节)
CREATE TABLE IF NOT EXISTS conversations (
    id              TEXT PRIMARY KEY,
    conv_type       TEXT NOT NULL DEFAULT 'direct',
    relation_type   TEXT DEFAULT 'social',
    peer_hasn_id    TEXT,
    peer_star_id    TEXT,
    peer_name       TEXT,
    peer_type       TEXT,
    peer_avatar_url TEXT,
    peer_owner_id   TEXT,
    last_message_at TEXT,
    last_message_preview TEXT,
    message_count   INTEGER DEFAULT 0,
    unread_count    INTEGER DEFAULT 0,
    status          TEXT DEFAULT 'active',
    trade_session_id TEXT,
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_conv_last_msg ON conversations(last_message_at DESC);
CREATE INDEX IF NOT EXISTS idx_conv_peer ON conversations(peer_hasn_id);

-- 消息表 (对齐 02 协议 2.1 节 — 全新 v4.0 Schema)
CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    from_hasn_id    TEXT NOT NULL,
    from_owner_id   TEXT NOT NULL,
    from_entity_type TEXT NOT NULL DEFAULT 'human',
    to_hasn_id      TEXT NOT NULL,
    to_owner_id     TEXT NOT NULL,
    content_type    TEXT NOT NULL DEFAULT 'text',
    body            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'sending',
    send_status     TEXT DEFAULT 'sending',
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_msg_conv ON messages(conversation_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_msg_from_owner ON messages(from_owner_id);
CREATE INDEX IF NOT EXISTS idx_msg_send_status ON messages(send_status) WHERE send_status IN ('sending', 'failed');

-- 联系人表
CREATE TABLE IF NOT EXISTS contacts (
    id              INTEGER PRIMARY KEY,
    peer_hasn_id    TEXT NOT NULL,
    peer_star_id    TEXT NOT NULL,
    peer_name       TEXT NOT NULL,
    peer_type       TEXT NOT NULL DEFAULT 'human',
    peer_avatar_url TEXT,
    peer_owner_id   TEXT,
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
    last_synced_id  TEXT NOT NULL,
    synced_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 认证信息表
CREATE TABLE IF NOT EXISTS auth_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

"#;
