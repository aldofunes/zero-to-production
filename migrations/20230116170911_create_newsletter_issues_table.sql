CREATE TABLE newsletter_issues (
  newsletter_issue_id uuid NOT NULL,
  title TEXT NOT NULL,
  text_content TEXT NOT NULL,
  html_content TEXT NOT NULL,
  published_at TEXT NOT NULL,
  PRIMARY KEY(newsletter_issue_id)
);

INSERT INTO issue_delivery_queue (newsletter_issue_id, subscriber_email)
SELECT $1,
  email
FROM subscriptions
WHERE STATUS = 'confirmed'