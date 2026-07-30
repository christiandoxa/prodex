use std::collections::BTreeMap;

use anyhow::{Context, Result};
use redis::Commands;
use sha2::{Digest, Sha256};

use super::{runtime_gateway_redis_usage_hash_key, runtime_gateway_redis_usage_index_key};

const RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MAX_ENTRIES: usize = 100_000;
pub(super) const RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE: &str = "1";

pub(super) fn runtime_gateway_redis_migrate_legacy_usage_from_connection(
    conn: &mut redis::Connection,
    redis_key: &str,
) -> Result<()> {
    let marker_key = runtime_gateway_redis_usage_migration_marker_key(redis_key);
    let in_progress_key = runtime_gateway_redis_usage_migration_in_progress_key(redis_key);
    let Some(payload) = conn.get::<_, Option<String>>(redis_key)? else {
        let _: i32 = redis::cmd("EVAL")
            .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_COMPLETE_EMPTY_SCRIPT)
            .arg(3)
            .arg(redis_key)
            .arg(&marker_key)
            .arg(&in_progress_key)
            .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE)
            .query(conn)?;
        return Ok(());
    };
    let legacy_usage = serde_json::from_str::<
        BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
    >(&payload)
    .context("failed to parse legacy gateway redis virtual key usage")?;
    if legacy_usage.len() > RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MAX_ENTRIES {
        anyhow::bail!("legacy gateway redis usage exceeds the migration limit");
    }
    let fingerprint = runtime_gateway_redis_legacy_usage_fingerprint(&payload);
    let started: i32 = redis::cmd("EVAL")
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_BEGIN_SCRIPT)
        .arg(3)
        .arg(redis_key)
        .arg(&marker_key)
        .arg(&in_progress_key)
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE)
        .arg(&payload)
        .arg(&fingerprint)
        .query(conn)?;
    if started == 0 {
        return Ok(());
    }

    if !legacy_usage.is_empty() {
        let index_key = runtime_gateway_redis_usage_index_key(redis_key);
        let migrated_keys_key = runtime_gateway_redis_usage_migrated_keys_key(redis_key);
        let mut migrations = redis::pipe();
        for (key_name, usage) in legacy_usage {
            migrations
                .cmd("EVAL")
                .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT)
                .arg(3)
                .arg(&index_key)
                .arg(runtime_gateway_redis_usage_hash_key(redis_key, &key_name))
                .arg(&migrated_keys_key)
                .arg(key_name)
                .arg(usage.minute_epoch)
                .arg(usage.requests_this_minute)
                .arg(usage.tokens_this_minute)
                .arg(usage.requests_total)
                .arg(usage.spend_microusd);
        }
        migrations.query::<Vec<i32>>(conn)?;
    }

    let _: i32 = redis::cmd("EVAL")
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_FINALIZE_SCRIPT)
        .arg(3)
        .arg(redis_key)
        .arg(&marker_key)
        .arg(&in_progress_key)
        .arg(RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATION_MARKER_VALUE)
        .arg(&payload)
        .arg(&fingerprint)
        .query(conn)?;
    Ok(())
}

pub(super) fn runtime_gateway_redis_usage_migration_marker_key(redis_key: &str) -> String {
    format!("{redis_key}:legacy_usage_migrated_v1")
}

pub(super) fn runtime_gateway_redis_usage_migrated_keys_key(redis_key: &str) -> String {
    format!("{redis_key}:legacy_usage_migrated_keys_v1")
}

pub(super) fn runtime_gateway_redis_usage_migration_in_progress_key(redis_key: &str) -> String {
    format!("{redis_key}:legacy_usage_migration_v1")
}

pub(super) fn runtime_gateway_redis_legacy_usage_fingerprint(payload: &str) -> String {
    let digest = Sha256::digest(payload.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

pub(super) const RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_COMPLETE_EMPTY_SCRIPT: &str = r#"
        local marker = redis.call('GET', KEYS[2])
        local legacy_exists = redis.call('EXISTS', KEYS[1])
        local in_progress = redis.call('GET', KEYS[3])
        if marker ~= false then
            if marker ~= ARGV[1] then
                return redis.error_reply('gateway redis usage migration marker is invalid')
            end
            if legacy_exists ~= 0 then
                return redis.error_reply('gateway redis usage migration retained its legacy blob')
            end
            if in_progress ~= false then
                return redis.error_reply('gateway redis usage migration retained in-progress state')
            end
            return 0
        end
        if legacy_exists ~= 0 then
            return redis.error_reply('legacy gateway redis usage changed during migration')
        end
        if in_progress ~= false then
            return redis.error_reply('legacy gateway redis usage migration lost its source blob')
        end
        redis.call('SET', KEYS[2], ARGV[1])
        return 1
        "#;

pub(super) const RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_BEGIN_SCRIPT: &str = r#"
        local marker = redis.call('GET', KEYS[2])
        local legacy = redis.call('GET', KEYS[1])
        local in_progress = redis.call('GET', KEYS[3])
        if marker ~= false then
            if marker ~= ARGV[1] then
                return redis.error_reply('gateway redis usage migration marker is invalid')
            end
            if legacy ~= false then
                return redis.error_reply('gateway redis usage migration retained its legacy blob')
            end
            if in_progress ~= false then
                return redis.error_reply('gateway redis usage migration retained in-progress state')
            end
            return 0
        end
        if legacy == false or legacy ~= ARGV[2] then
            return redis.error_reply('legacy gateway redis usage changed during migration')
        end
        if in_progress == false then
            redis.call('SET', KEYS[3], ARGV[3])
        elseif in_progress ~= ARGV[3] then
            return redis.error_reply('legacy gateway redis usage changed while migration was in progress')
        end
        return 1
        "#;

pub(super) const RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_FINALIZE_SCRIPT: &str = r#"
        local marker = redis.call('GET', KEYS[2])
        local legacy = redis.call('GET', KEYS[1])
        local in_progress = redis.call('GET', KEYS[3])
        if marker ~= false then
            if marker ~= ARGV[1] then
                return redis.error_reply('gateway redis usage migration marker is invalid')
            end
            if legacy ~= false then
                return redis.error_reply('gateway redis usage migration retained its legacy blob')
            end
            if in_progress ~= false then
                return redis.error_reply('gateway redis usage migration retained in-progress state')
            end
            return 0
        end
        if legacy == false or legacy ~= ARGV[2] then
            return redis.error_reply('legacy gateway redis usage changed during migration')
        end
        if in_progress ~= ARGV[3] then
            return redis.error_reply('legacy gateway redis usage migration state is inconsistent')
        end
        redis.call('DEL', KEYS[1])
        redis.call('DEL', KEYS[3])
        redis.call('SET', KEYS[2], ARGV[1])
        return 1
        "#;

pub(super) const RUNTIME_GATEWAY_REDIS_LEGACY_USAGE_MIGRATE_SCRIPT: &str = r#"
        local function valid_type(key, expected)
            local actual = redis.call('TYPE', key).ok
            return actual == 'none' or actual == expected
        end
        if not valid_type(KEYS[1], 'set')
            or not valid_type(KEYS[2], 'hash')
            or not valid_type(KEYS[3], 'set') then
            return redis.error_reply('WRONGTYPE gateway usage migration key')
        end
        local usage_member = redis.call('SISMEMBER', KEYS[1], ARGV[1])
        local usage_exists = redis.call('EXISTS', KEYS[2])
        if usage_member ~= usage_exists then
            return redis.error_reply('gateway usage migration index is inconsistent')
        end
        local already_migrated = redis.call('SISMEMBER', KEYS[3], ARGV[1])
        if already_migrated == 1 then
            if usage_exists ~= 1 then
                return redis.error_reply('gateway usage migration marker is inconsistent')
            end
            return 0
        end

        local function normalize(value)
            if value == false or value == nil or value == '' then
                return '0'
            end
            if string.match(value, '^%d+$') == nil then
                return nil
            end
            value = string.gsub(value, '^0+', '')
            if value == '' then
                return '0'
            end
            return value
        end
        local function add_decimal(left, right)
            left = normalize(left)
            right = normalize(right)
            if left == nil or right == nil then
                return nil
            end
            local result = ''
            local carry = 0
            local left_index = string.len(left)
            local right_index = string.len(right)
            while left_index > 0 or right_index > 0 or carry > 0 do
                local left_digit = 0
                local right_digit = 0
                if left_index > 0 then
                    left_digit = string.byte(left, left_index) - 48
                    left_index = left_index - 1
                end
                if right_index > 0 then
                    right_digit = string.byte(right, right_index) - 48
                    right_index = right_index - 1
                end
                local total = left_digit + right_digit + carry
                result = string.char(48 + (total % 10)) .. result
                carry = math.floor(total / 10)
            end
            if string.len(result) > 19
                or (string.len(result) == 19 and result > '9223372036854775807') then
                return nil
            end
            return result
        end
        local function compare_decimal(left, right)
            if string.len(left) < string.len(right) then
                return -1
            end
            if string.len(left) > string.len(right) then
                return 1
            end
            if left < right then
                return -1
            end
            if left > right then
                return 1
            end
            return 0
        end

        local legacy_minute = normalize(ARGV[2])
        local legacy_requests = add_decimal('0', ARGV[3])
        local legacy_tokens = add_decimal('0', ARGV[4])
        local legacy_total = add_decimal('0', ARGV[5])
        local legacy_spend = add_decimal('0', ARGV[6])
        if legacy_minute == nil or legacy_requests == nil or legacy_tokens == nil
            or legacy_total == nil or legacy_spend == nil then
            return redis.error_reply('gateway usage migration counter is malformed or overflowed')
        end

        local next_minute = legacy_minute
        local next_requests = legacy_requests
        local next_tokens = legacy_tokens
        local next_total = legacy_total
        local next_spend = legacy_spend
        if usage_exists == 1 then
            local current_minute = normalize(redis.call('HGET', KEYS[2], 'minute_epoch'))
            local current_requests =
                add_decimal('0', redis.call('HGET', KEYS[2], 'requests_this_minute'))
            local current_tokens =
                add_decimal('0', redis.call('HGET', KEYS[2], 'tokens_this_minute'))
            local current_total = add_decimal('0', redis.call('HGET', KEYS[2], 'requests_total'))
            local current_spend = add_decimal('0', redis.call('HGET', KEYS[2], 'spend_microusd'))
            if current_minute == nil or current_requests == nil or current_tokens == nil
                or current_total == nil or current_spend == nil then
                return redis.error_reply(
                    'gateway usage migration counter is malformed or overflowed'
                )
            end
            next_total = add_decimal(current_total, legacy_total)
            next_spend = add_decimal(current_spend, legacy_spend)
            local minute_order = compare_decimal(current_minute, legacy_minute)
            if minute_order == 0 then
                next_requests = add_decimal(current_requests, legacy_requests)
                next_tokens = add_decimal(current_tokens, legacy_tokens)
            elseif minute_order > 0 then
                next_minute = current_minute
                next_requests = current_requests
                next_tokens = current_tokens
            end
            if next_requests == nil or next_tokens == nil
                or next_total == nil or next_spend == nil then
                return redis.error_reply('gateway usage migration counter overflow')
            end
        end

        redis.call('SADD', KEYS[1], ARGV[1])
        redis.call(
            'HSET', KEYS[2],
            'minute_epoch', next_minute,
            'requests_this_minute', next_requests,
            'tokens_this_minute', next_tokens,
            'requests_total', next_total,
            'spend_microusd', next_spend
        )
        redis.call('SADD', KEYS[3], ARGV[1])
        return 1
        "#;
