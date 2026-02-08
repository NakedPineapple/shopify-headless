#!/bin/sh
set -euo pipefail

# pg-backup.sh — Dump both NakedPineapple databases to Cloudflare R2.
#
# Required environment variables:
#   PG_STOREFRONT_URL   - Postgres connection string for np_storefront
#   PG_ADMIN_URL        - Postgres connection string for np_admin
#   R2_ACCESS_KEY_ID    - Cloudflare R2 access key
#   R2_SECRET_ACCESS_KEY - Cloudflare R2 secret key
#   R2_ENDPOINT         - R2 S3-compatible endpoint URL
#   R2_BUCKET           - R2 bucket name

DATE=$(date -u +%Y-%m-%d)
RETENTION_DAYS=${RETENTION_DAYS:-30}

export AWS_ACCESS_KEY_ID="$R2_ACCESS_KEY_ID"
export AWS_SECRET_ACCESS_KEY="$R2_SECRET_ACCESS_KEY"

dump_and_upload() {
    local db_name="$1"
    local db_url="$2"
    local file="/tmp/${db_name}-${DATE}.dump"

    echo "Dumping ${db_name}..."
    pg_dump --format=custom --compress=6 "$db_url" -f "$file"

    echo "Uploading ${db_name} to R2..."
    aws s3 cp "$file" "s3://${R2_BUCKET}/${db_name}/${db_name}-${DATE}.dump" \
        --endpoint-url "$R2_ENDPOINT"

    rm -f "$file"
    echo "${db_name} backup complete."
}

cleanup_old_backups() {
    local db_name="$1"
    local cutoff
    cutoff=$(date -u -d "-${RETENTION_DAYS} days" +%Y-%m-%d 2>/dev/null || \
             date -u -v-${RETENTION_DAYS}d +%Y-%m-%d)

    echo "Cleaning up ${db_name} backups older than ${RETENTION_DAYS} days (before ${cutoff})..."
    aws s3 ls "s3://${R2_BUCKET}/${db_name}/" --endpoint-url "$R2_ENDPOINT" | \
        awk '{print $4}' | while read -r key; do
            # Extract date from filename: dbname-YYYY-MM-DD.dump
            file_date=$(echo "$key" | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' || true)
            if [ -n "$file_date" ] && [ "$file_date" \< "$cutoff" ]; then
                echo "Deleting old backup: ${key}"
                aws s3 rm "s3://${R2_BUCKET}/${db_name}/${key}" --endpoint-url "$R2_ENDPOINT"
            fi
        done
}

echo "=== NakedPineapple Database Backup — ${DATE} ==="

dump_and_upload "np_storefront" "$PG_STOREFRONT_URL"
dump_and_upload "np_admin" "$PG_ADMIN_URL"

cleanup_old_backups "np_storefront"
cleanup_old_backups "np_admin"

echo "=== Backup complete ==="
