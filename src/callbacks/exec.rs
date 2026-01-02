use miniserde::{Deserialize, Serialize, json};
use std::process::Stdio;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteExec {
    message: String,
    task_id: String,
    command: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteExecCallback {
    task_id: String,
    result: String,
    exit_code: i32,
    finished_at: String,
}

// Receive string directly instead of struct to avoid redundant parsing
pub async fn exec_command(
    utf8_str: &str,
    callback_url: String,
    ignore_unsafe_cert: &bool,
) -> Result<(), String> {
    let remote_exec: RemoteExec =
        json::from_str(utf8_str).map_err(|_| "Failed to parse RemoteExec".to_string())?;

    let exec = tokio::spawn(async move {
        let Ok(child) = Command::new("bash")
            .arg("-c")
            .arg(&remote_exec.command) // Avoid cloning
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        else {
            return Err("failed to execute process".to_string());
        };

        let Ok(output) = child.wait_with_output().await else {
            return Err("failed to get process output".to_string());
        };

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);

        let status = output.status.code().unwrap_or(1);

        Ok((status, format!("{stdout_str}{stderr_str}"))) // Simplify string concatenation
    });

    let Ok(Ok((status, output))) = exec.await else {
        return Err("failed to execute process".to_string());
    };

    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    let finished_at = now.format(&Rfc3339).unwrap_or_default();

    let reply = RemoteExecCallback {
        task_id: remote_exec.task_id,
        result: output,
        exit_code: status,
        finished_at,
    };

    let json_string = json::to_string(&reply);

    #[cfg(all(feature = "ureq-support", not(feature = "nyquest-support")))]
    {
        use crate::utils::create_ureq_agent;
        let agent = create_ureq_agent(*ignore_unsafe_cert);
        if let Ok(req) = agent.post(callback_url).send(&json_string) {
            if req.status().is_success() {
                Ok(())
            } else {
                Err("server returned a error".to_string())
            }
        } else {
            Err("Unable to connect server".to_string())
        }
    }

    #[cfg(feature = "nyquest-support")]
    {
        use nyquest::Body;
        use nyquest::Request;
        let client = crate::utils::create_nyquest_client(*ignore_unsafe_cert);
        let body = Body::text(json_string, "application/json");
        let request = Request::post(callback_url).with_body(body);

        if let Ok(res) = client.request(request) {
            if res.status().is_successful() {
                Ok(())
            } else {
                Err("server returned a error".to_string())
            }
        } else {
            Err("Unable to connect server".to_string())
        }
    }

    #[cfg(not(any(feature = "ureq-support", feature = "nyquest-support")))]
    {
        Err("No HTTP client feature enabled".to_string())
    }
}
