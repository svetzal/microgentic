//! Process-local scripted gateway; named pipes carry test control, never HTTP.
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};

use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    Omlx,
    Ollama,
}

impl Wire {
    pub fn name(self) -> &'static str {
        match self {
            Wire::Omlx => "omlx",
            Wire::Ollama => "ollama",
        }
    }
}

/// A one-shot latch a test opens to release a held stream.
#[derive(Clone, Default)]
pub struct Gate(Arc<(Mutex<bool>, Condvar)>);

impl Gate {
    pub fn open(&self) {
        let (open, signal) = &*self.0;
        *open.lock().expect("gate") = true;
        signal.notify_all();
    }

    pub fn wait(&self) {
        let (open, signal) = &*self.0;
        let guard = open.lock().unwrap();
        drop(signal.wait_while(guard, |open| !*open).unwrap());
    }
}

/// One piece of a streamed reply.
#[derive(Clone)]
pub enum Step {
    Thinking(String),
    Content(String),
    /// Tool calls in one frame.
    Tools(Vec<ToolCall>),
    /// Hold the stream until the gate opens.
    Hold(Gate),
    /// Fail the normalized stream before a completed response.
    Abort,
}

pub fn thinking(text: &str) -> Step {
    Step::Thinking(text.into())
}

pub fn content(text: &str) -> Step {
    Step::Content(text.into())
}

/// One tool call. oMLX calls without an `id` get `call-{request}-{index}`;
/// Ollama has no call ids.
#[derive(Clone)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

pub fn tool(name: &str, arguments: Value) -> Step {
    tools(&[(name, arguments)])
}

pub fn tools(calls: &[(&str, Value)]) -> Step {
    Step::Tools(
        calls
            .iter()
            .map(|(name, arguments)| ToolCall {
                name: (*name).into(),
                arguments: arguments.clone(),
            })
            .collect(),
    )
}

/// The normalized gateway answer to one request.
pub enum Reply {
    /// A streamed model response, finished unless it holds an `Abort`.
    Stream(Vec<Step>),
    Status(u16),
}

/// What a script sees about a model request.
pub struct ModelRequest<'a> {
    /// 1-based count of model requests so far, including this one.
    pub number: usize,
    pub body: &'a Value,
}

impl ModelRequest<'_> {
    /// The text of the last message, usually the user's prompt.
    pub fn last_message(&self) -> &str {
        self.body["messages"]
            .as_array()
            .and_then(|messages| messages.last())
            .and_then(|message| message["content"].as_str())
            .unwrap_or_default()
    }
}

use std::fs::{File, OpenOptions};

pub struct ScriptedGateway {
    wire: Wire,
    root: tempfile::TempDir,
    requests: Arc<Mutex<Vec<Value>>>,
}
pub fn control_pipe(path: &std::path::Path) -> File {
    assert!(
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap()
}
impl ScriptedGateway {
    pub fn start(
        wire: Wire,
        script: impl Fn(&ModelRequest) -> Reply + Send + Sync + 'static,
    ) -> Self {
        Self::start_with_models(wire, &["test-model", "other-model"], script)
    }
    pub fn start_with_models(
        wire: Wire,
        models: &[&str],
        script: impl Fn(&ModelRequest) -> Reply + Send + Sync + 'static,
    ) -> Self {
        let root = tempfile::tempdir().unwrap();
        let input = control_pipe(&root.path().join("requests"));

        std::fs::write(
            root.path().join("metadata.json"),
            json!({"models":models,"window":null}).to_string(),
        )
        .unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(input).lines() {
                let Ok(line) = line else { break };
                let mut body: Value = serde_json::from_str(&line).unwrap();
                if body == "stop" {
                    break;
                }
                let reply = body.as_object_mut().unwrap().remove("reply").unwrap();
                let mut output = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(reply.as_str().unwrap())
                    .unwrap();
                let number = {
                    let mut requests = recorded.lock().unwrap();
                    requests.push(body.clone());
                    requests.len()
                };
                let steps = match script(&ModelRequest {
                    number,
                    body: &body,
                }) {
                    Reply::Stream(steps) => steps,
                    Reply::Status(status) => {
                        writeln!(
                            output,
                            "{}",
                            json!({"error":format!("scripted error {status}")})
                        )
                        .unwrap();
                        continue;
                    }
                };
                for step in steps {
                    let frame = match step {
                        Step::Hold(gate) => {
                            gate.wait();
                            continue;
                        }
                        Step::Content(text) => json!({"content":text}),
                        Step::Thinking(text) => json!({"thinking":text}),
                        Step::Tools(calls) => {
                            json!({"tools": calls.iter().enumerate().map(|(index,call)| {
                            json!({"id":if wire == Wire::Omlx {Some(format!("call-{number}-{index}"))} else {None},"name":call.name,"arguments":call.arguments})
                        }).collect::<Vec<_>>()})
                        }
                        Step::Abort => json!({"error":"scripted interrupted stream"}),
                    };
                    if writeln!(output, "{frame}").is_err() {
                        return;
                    }
                }
                if writeln!(output, "{}", json!({"done":true})).is_err() {
                    return;
                }
            }
        });
        Self {
            wire,
            root,
            requests,
        }
    }
    pub fn replying(wire: Wire, steps: Vec<Step>) -> Self {
        Self::start(wire, move |_| Reply::Stream(steps.clone()))
    }
    pub fn turns(wire: Wire, turns: Vec<Vec<Step>>) -> Self {
        Self::start(wire, move |request| {
            Reply::Stream(turns[(request.number - 1).min(turns.len() - 1)].clone())
        })
    }
    pub fn wire(&self) -> Wire {
        self.wire
    }
    pub fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }
    pub fn request_count(&self) -> usize {
        self.requests().len()
    }
    pub fn fail_listing(&self, message: &str) {
        let path = self.root.path().join("metadata.json");
        let mut metadata: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        metadata["models_error"] = json!(message);
        std::fs::write(path, metadata.to_string()).unwrap();
    }
    pub fn advertise_window(&self, tokens: Option<usize>) {
        let path = self.root.path().join("metadata.json");
        let mut metadata: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        metadata["window"] = tokens.map(|tokens|json!({"max_context_tokens":tokens,"field":match self.wire {Wire::Omlx=>"max_model_len",Wire::Ollama=>"model_info.test.context_length"},"model_num_ctx":null})).unwrap_or(Value::Null);
        std::fs::write(path, metadata.to_string()).unwrap();
    }
    pub fn metadata(&self) -> Vec<Value> {
        std::fs::read_to_string(self.root.path().join("metadata.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    pub fn env(&self) -> Vec<(&'static str, String)> {
        vec![(
            "MICROGENTIC_TUI_SCRIPT",
            self.root.path().display().to_string(),
        )]
    }
}
impl Drop for ScriptedGateway {
    fn drop(&mut self) {
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .open(self.root.path().join("requests"))
        {
            let _ = writeln!(file, "\"stop\"");
        }
    }
}
/// Every user message's text, in order.
pub fn user_messages(request: &Value) -> Vec<String> {
    request["messages"]
        .as_array()
        .map(|messages| {
            messages
                .iter()
                .filter(|m| m["role"] == "user")
                .filter_map(|m| m["content"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

pub fn system_message(request: &Value) -> String {
    request["messages"][0]["content"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}
