//! User repository for database operations.
//!
//! This module provides database access for users and their `WebAuthn` credentials.
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use webauthn_rs::prelude::Passkey;

use naked_pineapple_core::{CredentialId, Email, UserId};

use super::RepositoryError;
use crate::models::user::{User, UserCredential};

/// Repository for user database operations.
pub struct UserRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> UserRepository<'a> {
    /// Create a new user repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Get a user by their email address.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the email in the database is invalid.
    pub async fn get_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, email, email_verified,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM storefront.user
            WHERE email = $1
            "#,
            email.as_str()
        )
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => {
                let email = Email::parse(&r.email).map_err(|e| {
                    RepositoryError::DataCorruption(format!("invalid email in database: {e}"))
                })?;

                Ok(Some(User {
                    id: UserId::new(r.id),
                    email,
                    email_verified: r.email_verified,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Get a user by their ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the email in the database is invalid.
    pub async fn get_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, email, email_verified,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM storefront.user
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => {
                let email = Email::parse(&r.email).map_err(|e| {
                    RepositoryError::DataCorruption(format!("invalid email in database: {e}"))
                })?;

                Ok(Some(User {
                    id: UserId::new(r.id),
                    email,
                    email_verified: r.email_verified,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Create a new user with just an email (no password).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if the email already exists.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn create(&self, email: &Email) -> Result<User, RepositoryError> {
        let row = sqlx::query!(
            r#"
            INSERT INTO storefront.user (email)
            VALUES ($1)
            RETURNING id, email, email_verified,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            email.as_str()
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.is_unique_violation()
            {
                return RepositoryError::Conflict("email already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?;

        let email = Email::parse(&row.email).map_err(|e| {
            RepositoryError::DataCorruption(format!("invalid email in database: {e}"))
        })?;

        Ok(User {
            id: UserId::new(row.id),
            email,
            email_verified: row.email_verified,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// Create a new user with email and password.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if the email already exists.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn create_with_password(
        &self,
        email: &Email,
        password_hash: &str,
    ) -> Result<User, RepositoryError> {
        let mut tx = self.pool.begin().await?;

        // Create user
        let row = sqlx::query!(
            r#"
            INSERT INTO storefront.user (email)
            VALUES ($1)
            RETURNING id, email, email_verified,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            email.as_str()
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.is_unique_violation()
            {
                return RepositoryError::Conflict("email already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?;

        let parsed_email = Email::parse(&row.email).map_err(|e| {
            RepositoryError::DataCorruption(format!("invalid email in database: {e}"))
        })?;

        let user = User {
            id: UserId::new(row.id),
            email: parsed_email,
            email_verified: row.email_verified,
            created_at: row.created_at,
            updated_at: row.updated_at,
        };

        // Create password entry
        sqlx::query!(
            r#"
            INSERT INTO storefront.user_password (user_id, password_hash)
            VALUES ($1, $2)
            "#,
            user.id.as_i32(),
            password_hash
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(user)
    }

    /// Get a user's password hash by email.
    ///
    /// Returns `None` if the user doesn't exist or has no password set.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_password_hash(
        &self,
        email: &Email,
    ) -> Result<Option<(User, String)>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT u.id, u.email, u.email_verified,
                   u.created_at as "created_at: DateTime<Utc>",
                   u.updated_at as "updated_at: DateTime<Utc>",
                   p.password_hash as "password_hash?"
            FROM storefront.user u
            LEFT JOIN storefront.user_password p ON u.id = p.user_id
            WHERE u.email = $1
            "#,
            email.as_str()
        )
        .fetch_optional(self.pool)
        .await?;

        let Some(r) = row else {
            return Ok(None);
        };

        let Some(password_hash) = r.password_hash else {
            return Ok(None);
        };

        let email = Email::parse(&r.email)
            .map_err(|e| RepositoryError::DataCorruption(format!("invalid email: {e}")))?;

        let user = User {
            id: UserId::new(r.id),
            email,
            email_verified: r.email_verified,
            created_at: r.created_at,
            updated_at: r.updated_at,
        };

        Ok(Some((user, password_hash)))
    }

    /// Get all credentials for a user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if any credential data is invalid.
    pub async fn get_credentials(
        &self,
        user_id: UserId,
    ) -> Result<Vec<UserCredential>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, user_id, credential_id, public_key, name,
                   created_at as "created_at: DateTime<Utc>"
            FROM storefront.user_credential
            WHERE user_id = $1
            ORDER BY created_at ASC
            "#,
            user_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        let mut credentials = Vec::with_capacity(rows.len());
        for r in rows {
            let passkey: Passkey = serde_json::from_slice(&r.public_key).map_err(|e| {
                RepositoryError::DataCorruption(format!("invalid passkey data: {e}"))
            })?;

            credentials.push(UserCredential {
                id: CredentialId::new(r.id),
                user_id: UserId::new(r.user_id),
                webauthn_id: r.credential_id,
                passkey,
                name: r.name,
                created_at: r.created_at,
            });
        }

        Ok(credentials)
    }

    /// Create a new credential for a user.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Conflict` if the credential ID already exists.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn create_credential(
        &self,
        user_id: UserId,
        passkey: &Passkey,
        name: &str,
    ) -> Result<UserCredential, RepositoryError> {
        let public_key = serde_json::to_vec(passkey).map_err(|e| {
            RepositoryError::DataCorruption(format!("failed to serialize passkey: {e}"))
        })?;

        let row = sqlx::query!(
            r#"
            INSERT INTO storefront.user_credential (user_id, credential_id, public_key, counter, name)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, user_id, credential_id, public_key, name,
                      created_at as "created_at: DateTime<Utc>"
            "#,
            user_id.as_i32(),
            passkey.cred_id().as_ref(),
            &public_key,
            0_i32,
            name
        )
        .fetch_one(self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.is_unique_violation()
            {
                return RepositoryError::Conflict("credential already exists".to_owned());
            }
            RepositoryError::Database(e)
        })?;

        let passkey: Passkey = serde_json::from_slice(&row.public_key)
            .map_err(|e| RepositoryError::DataCorruption(format!("invalid passkey data: {e}")))?;

        Ok(UserCredential {
            id: CredentialId::new(row.id),
            user_id: UserId::new(row.user_id),
            webauthn_id: row.credential_id,
            passkey,
            name: row.name,
            created_at: row.created_at,
        })
    }

    /// Get a credential by its `WebAuthn` credential ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    /// Returns `RepositoryError::DataCorruption` if the credential data is invalid.
    pub async fn get_credential_by_webauthn_id(
        &self,
        credential_id: &[u8],
    ) -> Result<Option<UserCredential>, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT id, user_id, credential_id, public_key, name,
                   created_at as "created_at: DateTime<Utc>"
            FROM storefront.user_credential
            WHERE credential_id = $1
            "#,
            credential_id
        )
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => {
                let passkey: Passkey = serde_json::from_slice(&r.public_key).map_err(|e| {
                    RepositoryError::DataCorruption(format!("invalid passkey data: {e}"))
                })?;

                Ok(Some(UserCredential {
                    id: CredentialId::new(r.id),
                    user_id: UserId::new(r.user_id),
                    webauthn_id: r.credential_id,
                    passkey,
                    name: r.name,
                    created_at: r.created_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Update the counter for a credential (after successful authentication).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the credential doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn update_credential_counter(
        &self,
        credential_id: &[u8],
        counter: u32,
    ) -> Result<(), RepositoryError> {
        let counter_i32 = i32::try_from(counter).unwrap_or(i32::MAX);

        let result = sqlx::query!(
            r#"
            UPDATE storefront.user_credential
            SET counter = $1
            WHERE credential_id = $2
            "#,
            counter_i32,
            credential_id
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Update a credential's passkey data (after successful authentication).
    ///
    /// This updates the serialized passkey which includes the counter.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the credential doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn update_credential(
        &self,
        credential_id: &[u8],
        passkey: &Passkey,
    ) -> Result<(), RepositoryError> {
        let public_key = serde_json::to_vec(passkey).map_err(|e| {
            RepositoryError::DataCorruption(format!("failed to serialize passkey: {e}"))
        })?;

        let result = sqlx::query!(
            r#"
            UPDATE storefront.user_credential
            SET public_key = $1
            WHERE credential_id = $2
            "#,
            &public_key,
            credential_id
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Delete a credential by its database ID.
    ///
    /// # Returns
    ///
    /// Returns `true` if the credential was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn delete_credential(
        &self,
        user_id: UserId,
        credential_id: CredentialId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM storefront.user_credential
            WHERE id = $1 AND user_id = $2
            "#,
            credential_id.as_i32(),
            user_id.as_i32()
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Mark a user's email as verified.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the user doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn verify_email(&self, user_id: UserId) -> Result<(), RepositoryError> {
        let result = sqlx::query!(
            r#"
            UPDATE storefront.user
            SET email_verified = TRUE
            WHERE id = $1
            "#,
            user_id.as_i32()
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }
}
