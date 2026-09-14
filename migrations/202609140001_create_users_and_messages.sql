CREATE TABLE IF NOT EXISTS users (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    username VARCHAR(24) CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    UNIQUE KEY uq_users_username (username)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS messages (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    sender_id BIGINT UNSIGNED NOT NULL,
    recipient_id BIGINT UNSIGNED NULL,
    room VARCHAR(32) NULL,
    content VARCHAR(1000) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    PRIMARY KEY (id),
    INDEX idx_messages_public_history (room, recipient_id, created_at),
    INDEX idx_messages_recipient (recipient_id, created_at),
    CONSTRAINT fk_messages_sender
        FOREIGN KEY (sender_id) REFERENCES users (id) ON DELETE RESTRICT,
    CONSTRAINT fk_messages_recipient
        FOREIGN KEY (recipient_id) REFERENCES users (id) ON DELETE RESTRICT,
    CONSTRAINT chk_messages_destination CHECK (
        (recipient_id IS NULL AND room IS NOT NULL)
        OR (recipient_id IS NOT NULL AND room IS NULL)
    )
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_0900_ai_ci;
