use crate::backend::models::CommentItem;
use crate::backend::parse::{extract_text, find_json_key, parse_comments};
use anyhow::{bail, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;

const SUCCEEDED: &str = "STATUS_SUCCEEDED";

#[derive(Default)]
struct Proto(Vec<u8>);

impl Proto {
    fn varint(mut self, field: u32, value: u64) -> Self {
        self.write_varint((field << 3) as u64);
        self.write_varint(value);
        self
    }

    fn string(mut self, field: u32, value: &str) -> Self {
        self.write_varint(((field << 3) | 2) as u64);
        self.write_varint(value.len() as u64);
        self.0.extend_from_slice(value.as_bytes());
        self
    }

    fn message(mut self, field: u32, value: Proto) -> Self {
        self.write_varint(((field << 3) | 2) as u64);
        self.write_varint(value.0.len() as u64);
        self.0.extend_from_slice(&value.0);
        self
    }

    fn write_varint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.0.push(((value as u8) & 0x7f) | 0x80);
            value >>= 7;
        }
        self.0.push(value as u8);
    }
}

fn encode(proto: Proto) -> String {
    urlencoding::encode(&STANDARD.encode(proto.0)).into_owned()
}

/// TVHTML5 create_comment params: {2: videoId, 5: {}, 10: 7}.
pub(crate) fn create_comment_params(video_id: &str) -> String {
    encode(
        Proto::default()
            .string(2, video_id)
            .message(5, Proto::default())
            .varint(10, 7),
    )
}

/// TVHTML5 create_comment_reply params: {2: videoId, 4: parentId, 5: {1: 0}, 10: 7}.
pub(crate) fn create_reply_params(video_id: &str, parent_id: &str) -> String {
    encode(
        Proto::default()
            .string(2, video_id)
            .string(4, parent_id)
            .message(5, Proto::default().varint(1, 0))
            .varint(10, 7),
    )
}

/// perform_comment_action delete params: {1: 6, 2: 2, 3: commentId, 5: videoId}.
pub(crate) fn delete_comment_params(video_id: &str, comment_id: &str) -> String {
    encode(
        Proto::default()
            .varint(1, 6)
            .varint(2, 2)
            .string(3, comment_id)
            .string(5, video_id),
    )
}

fn action_result(value: &Value) -> Option<&Value> {
    find_json_key(value, "actionResult")
}

fn status(result: Option<&Value>) -> Option<&str> {
    result?.get("status")?.as_str()
}

fn feedback(result: &Value) -> Option<String> {
    let text = result
        .get("feedbackText")
        .map(extract_text)
        .unwrap_or_default();
    (!text.trim().is_empty()).then_some(text)
}

/// Returns the created comment when YouTube describes it. A successful response may omit the
/// renderer; that is still success and must not be retried because retrying could duplicate a post.
pub(crate) fn created_comment(answer: &Value) -> Result<Option<CommentItem>> {
    let result = action_result(answer);
    if let Some(s) = status(result) {
        if s != SUCCEEDED {
            let why = result.and_then(feedback).unwrap_or_else(|| s.to_string());
            bail!("YouTube refused comment write: {why}");
        }
    }
    if let Some(comment) = parse_comments(answer)
        .comments
        .into_iter()
        .find(|comment| !comment.id.is_empty())
    {
        return Ok(Some(comment));
    }
    if status(result) == Some(SUCCEEDED) {
        return Ok(None);
    }
    bail!("YouTube did not confirm the comment write")
}

pub(crate) fn confirm_deleted(answer: &Value, comment_id: &str) -> Result<()> {
    let remove = find_json_key(answer, "removeCommentAction");
    let result = remove
        .and_then(|value| value.get("actionResult"))
        .or_else(|| action_result(answer));
    let Some(s) = status(result) else {
        bail!("YouTube did not confirm comment deletion");
    };
    if s != SUCCEEDED {
        let why = result.and_then(feedback).unwrap_or_else(|| s.to_string());
        bail!("YouTube refused comment deletion: {why}");
    }
    if let Some(removed) = remove
        .and_then(|value| value.get("commentId"))
        .and_then(Value::as_str)
    {
        if removed != comment_id {
            bail!("YouTube confirmed deletion for a different comment");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn captured_comment_write_params_match_known_good_requests() {
        assert_eq!(
            create_comment_params("Gp3BRbNSLEA"),
            "EgtHcDNCUmJOU0xFQSoAUAc%3D"
        );
        assert_eq!(
            create_reply_params("jNQXAC9IVRw", "Ugws8WkM4-stbgyJzI54AaABAg"),
            "EgtqTlFYQUM5SVZSdyIaVWd3czhXa000LXN0Ymd5SnpJNTRBYUFCQWcqAggAUAc%3D"
        );
        assert_eq!(
            delete_comment_params("jNQXAC9IVRw", "UgwbBtiUuiLoDx-QlId4AaABAg"),
            "CAYQAhoaVWd3YkJ0aVV1aUxvRHgtUWxJZDRBYUFCQWcqC2pOUVhBQzlJVlJ3"
        );
    }

    #[test]
    fn delete_keeps_full_reply_id_and_verifies_named_comment() {
        let reply = "Ugws8WkM4-stbgyJzI54AaABAg.AbOBNn0tNCeAbOBPW6vShG";
        let encoded = delete_comment_params("jNQXAC9IVRw", reply);
        let raw = base64::engine::general_purpose::STANDARD
            .decode(urlencoding::decode(&encoded).unwrap().as_bytes())
            .unwrap();
        assert!(raw
            .windows(reply.len())
            .any(|window| window == reply.as_bytes()));
        assert!(confirm_deleted(
            &json!({"removeCommentAction":{"commentId":reply,"actionResult":{"status":"STATUS_SUCCEEDED"}}}),
            reply
        )
        .is_ok());
        assert!(confirm_deleted(
            &json!({"removeCommentAction":{"commentId":"other","actionResult":{"status":"STATUS_SUCCEEDED"}}}),
            reply
        )
        .is_err());
    }

    #[test]
    fn successful_write_without_renderer_is_not_reported_as_failure() {
        assert!(
            created_comment(&json!({"actionResult":{"status":"STATUS_SUCCEEDED"}}))
                .unwrap()
                .is_none()
        );
        assert!(created_comment(&json!({"actionResult":{"status":"STATUS_FAILED","feedbackText":{"runs":[{"text":"Comment failed"}]}}})).is_err());
    }
}
