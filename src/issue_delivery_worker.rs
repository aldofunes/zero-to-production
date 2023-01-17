use std::time::Duration;

use crate::{
    configuration::Settings, domain::SubscriberEmail, email_client::EmailClient,
    startup::get_db_pool,
};
use chrono::Utc;
use rand::{
    rngs::{OsRng, StdRng},
    Rng, SeedableRng,
};
use sqlx::{PgPool, Postgres, Transaction};
use std::cmp::min;
use tracing::{field::display, Span};
use uuid::Uuid;

pub async fn run_worker_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    let connection_pool = get_db_pool(&configuration.database);
    let email_client = configuration.email_client.client();
    worker_loop(connection_pool, email_client).await
}

async fn worker_loop(pool: PgPool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    let mut rng = StdRng::from_seed(OsRng.gen());
    loop {
        if try_execute_task(&pool, &email_client, &mut rng)
            .await
            .is_err()
        {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument(skip_all,fields(newsletter_issue_id=tracing::field::Empty,subscriber_email=tracing::field::Empty),err)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
    rng: &mut StdRng,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let task = dequeue_task(pool).await?;

    if task.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }
    let (mut transaction, queue_item) = task.unwrap();

    Span::current()
        .record("newsletter_issue_id", &display(queue_item.issue_id))
        .record("subscriber_email", &display(&queue_item.email))
        .record("n_retries", &display(queue_item.n_retries))
        .record("execute_after", &display(&queue_item.execute_after));

    match SubscriberEmail::parse(queue_item.email.clone()) {
        Ok(email) => {
            let issue = get_issue(pool, queue_item.issue_id).await?;
            match email_client
                .send_email(
                    &email,
                    &issue.title,
                    &issue.html_content,
                    &issue.text_content,
                )
                .await
            {
                Ok(_) => delete_task(transaction, queue_item.issue_id, email.as_ref()).await?,
                Err(e) => {
                    const CAP: u16 = 900;
                    const BASE: u16 = 2;

                    let max_wait_seconds =
                        min(CAP, BASE.pow((queue_item.n_retries + 1).try_into()?)); // max wait is 5 minutes
                    let wait_seconds = rng.gen_range(1..max_wait_seconds);
                    let execute_after = Utc::now() + chrono::Duration::seconds(wait_seconds.into());

                    tracing::error!(
                        error.cause_chain = ?e,
                        error.message = %e,
                        n_retries = %queue_item.n_retries,
                        execute_after = %execute_after,
                        wait_seconds = %wait_seconds,
                        "Failed to deliver issue to a confirmed subscriber. Retrying.",
                    );

                    sqlx::query!(
                        r#"
                        UPDATE issue_delivery_queue
                        SET
                            n_retries = $3,
                            execute_after = $4
                        WHERE
                            newsletter_issue_id = $1
                            AND subscriber_email = $2
                        "#,
                        queue_item.issue_id,
                        email.to_string(),
                        queue_item.n_retries + 1,
                        execute_after,
                    )
                    .execute(&mut transaction)
                    .await?;
                    transaction.commit().await?;
                }
            }
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "Skipping a confirmed subscriber. Their stored contact details are invalid",
            );
            delete_task(transaction, queue_item.issue_id, queue_item.email.as_ref()).await?;
        }
    }
    Ok(ExecutionOutcome::TaskCompleted)
}

#[derive(Debug)]
struct IssueDeliveryQueueItem {
    issue_id: Uuid,
    email: String,
    n_retries: i16,
    execute_after: chrono::DateTime<Utc>,
}

type PgTransaction = Transaction<'static, Postgres>;
#[tracing::instrument(skip_all)]
async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(PgTransaction, IssueDeliveryQueueItem)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let r = sqlx::query!(
        r#"
        SELECT newsletter_issue_id, subscriber_email, n_retries, execute_after
        FROM issue_delivery_queue
        WHERE execute_after < NOW()
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut transaction)
    .await?;
    if let Some(r) = r {
        Ok(Some((
            transaction,
            IssueDeliveryQueueItem {
                issue_id: r.newsletter_issue_id,
                email: r.subscriber_email,
                n_retries: r.n_retries,
                execute_after: r.execute_after,
            },
        )))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE
            newsletter_issue_id = $1 AND
            subscriber_email = $2
        "#,
        issue_id,
        email
    )
    .execute(&mut transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}
#[tracing::instrument(skip_all)]
async fn get_issue(pool: &PgPool, issue_id: Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title, text_content, html_content
        FROM newsletter_issues
        WHERE
            newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}
