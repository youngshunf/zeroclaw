-- ============================================================
-- 唤星AI 后端数据库初始化脚本
-- 用于 ai-cloud-backend (MySQL) 的唤星扩展表
-- ============================================================

-- 服务器信息表
CREATE TABLE IF NOT EXISTS huanxing_server (
    id              BIGINT PRIMARY KEY AUTO_INCREMENT,
    server_id       VARCHAR(64) UNIQUE NOT NULL COMMENT '服务器唯一标识（如 server-001）',
    server_name     VARCHAR(128) COMMENT '服务器名称（如 京东云-华北1）',
    ip_address      VARCHAR(45) NOT NULL COMMENT '服务器IP地址',
    port            INT DEFAULT 22 COMMENT 'SSH端口',
    region          VARCHAR(64) COMMENT '地域（如 cn-north-1）',
    provider        VARCHAR(64) COMMENT '云服务商（如 jdcloud/aliyun/tencent）',
    max_users       INT DEFAULT 100 COMMENT '最大用户容量',
    status          VARCHAR(16) DEFAULT 'active' COMMENT '状态: active/maintenance/offline',
    gateway_status  VARCHAR(16) DEFAULT 'unknown' COMMENT 'Gateway状态: running/stopped/unknown',
    last_heartbeat  DATETIME COMMENT '最后心跳时间（guardian 定期上报）',
    config          JSON COMMENT '服务器配置信息（插件版本、OpenClaw版本等）',
    remark          VARCHAR(512) COMMENT '备注',
    created_time    DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_time    DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='唤星服务器信息表';

CREATE INDEX idx_server_status ON huanxing_server(status);

-- 唤星用户表（扩展 sys_user，关联服务器）
CREATE TABLE IF NOT EXISTS huanxing_user (
    id              BIGINT PRIMARY KEY AUTO_INCREMENT,
    user_id         BIGINT NOT NULL COMMENT '关联 sys_user.id',
    server_id       VARCHAR(64) NOT NULL COMMENT '所在服务器ID',
    agent_id        VARCHAR(128) UNIQUE COMMENT 'Agent ID（如 user-abc123）',
    star_name       VARCHAR(64) COMMENT '分身名字',
    template        VARCHAR(64) NOT NULL COMMENT '模板类型',
    workspace_path  VARCHAR(256) COMMENT '工作区路径',
    agent_status    VARCHAR(16) DEFAULT 'active' COMMENT 'Agent状态: active/disabled/suspended',
    channel_type    VARCHAR(16) COMMENT '注册渠道（feishu/qq）',
    channel_peer_id VARCHAR(128) COMMENT '渠道用户ID',
    created_time    DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_time    DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (server_id) REFERENCES huanxing_server(server_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='唤星用户表';

-- 唯一约束：同一用户在同一服务器上只能有一条记录
ALTER TABLE huanxing_user ADD UNIQUE INDEX uk_user_server (user_id, server_id);

CREATE INDEX idx_hx_user_server ON huanxing_user(server_id);
CREATE INDEX idx_hx_user_user ON huanxing_user(user_id);
CREATE INDEX idx_hx_user_channel ON huanxing_user(channel_type, channel_peer_id);
CREATE INDEX idx_hx_user_status ON huanxing_user(agent_status);
