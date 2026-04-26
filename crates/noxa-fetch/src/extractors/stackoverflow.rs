use serde_json::{Value, json};

use super::{ExtractorInfo, http::ExtractorHttp};
use crate::error::FetchError;

pub const INFO: ExtractorInfo = ExtractorInfo {
    name: "stackoverflow",
    label: "Stack Overflow Question",
    description: "Extract question metadata from Stack Overflow.",
    url_patterns: &["https://stackoverflow.com/questions/*"],
};

pub fn matches(url: &str) -> bool {
    parse_question_id(url).is_some()
}

pub async fn extract(client: &dyn ExtractorHttp, url: &str) -> Result<Value, FetchError> {
    let id = parse_question_id(url).ok_or_else(|| {
        FetchError::Build(format!(
            "stackoverflow: cannot parse question id from '{url}'"
        ))
    })?;
    let q_url = format!(
        "https://api.stackexchange.com/2.3/questions/{id}?site=stackoverflow&filter=withbody"
    );
    let q_body = client.get_json(&q_url).await?;
    let question = q_body
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| FetchError::Build(format!("stackoverflow: question {id} not found")))?;
    let a_url = format!(
        "https://api.stackexchange.com/2.3/questions/{id}/answers?site=stackoverflow&filter=withbody&order=desc&sort=votes"
    );
    let a_body = client.get_json(&a_url).await?;
    let answers: Vec<_> = a_body
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|answer| {
            json!({
                "answer_id": answer.get("answer_id").cloned(),
                "is_accepted": answer.get("is_accepted").cloned(),
                "score": answer.get("score").cloned(),
                "body": answer.get("body").cloned(),
                "creation_date": answer.get("creation_date").cloned(),
                "last_edit_date": answer.get("last_edit_date").cloned(),
                "author": answer.pointer("/owner/display_name").cloned(),
                "author_rep": answer.pointer("/owner/reputation").cloned(),
            })
        })
        .collect();
    let accepted = answers
        .iter()
        .find(|answer| {
            answer
                .get("is_accepted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .cloned();

    Ok(json!({
        "url": url,
        "question_id": question.get("question_id").cloned(),
        "title": question.get("title").cloned(),
        "body": question.get("body").cloned(),
        "tags": question.get("tags").cloned(),
        "score": question.get("score").cloned(),
        "view_count": question.get("view_count").cloned(),
        "answer_count": question.get("answer_count").cloned(),
        "is_answered": question.get("is_answered").cloned(),
        "accepted_answer_id": question.get("accepted_answer_id").cloned(),
        "creation_date": question.get("creation_date").cloned(),
        "last_activity_date": question.get("last_activity_date").cloned(),
        "author": question.pointer("/owner/display_name").cloned(),
        "author_rep": question.pointer("/owner/reputation").cloned(),
        "link": question.get("link").cloned(),
        "accepted_answer": accepted,
        "top_answers": answers,
    }))
}

fn parse_question_id(url: &str) -> Option<u64> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host != "stackoverflow.com" && host != "www.stackoverflow.com" {
        return None;
    }
    let segs: Vec<_> = parsed.path_segments()?.filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 || segs[0] != "questions" {
        return None;
    }
    segs[1].parse().ok()
}
