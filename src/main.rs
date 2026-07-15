use async_openai::{Client, config::OpenAIConfig};
use clap::Parser;
use serde_json::{Value, json};
use std::{env, process};

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short = 'p', long)]
    prompt: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let base_url = env::var("OPENROUTER_BASE_URL")
        .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

    let api_key = env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
        eprintln!("OPENROUTER_API_KEY is not set");
        process::exit(1);
    });

    let config = OpenAIConfig::new()
        .with_api_base(base_url)
        .with_api_key(api_key);

    let client = Client::with_config(config);

    let mut messages: Vec<Value> = vec![
        json!({
            "role": "user",
            "content": args.prompt
        })
    ];

    'agent : loop {
   
        let response: Value = client
            .chat()
            .create_byot(json!({
                "messages": [
                    {
                        "role": "user",
                        "content": args.prompt
                    }
                ],
                "model": "anthropic/claude-haiku-4.5",
                "tools": [{
                            "type": "function",
                            "function": {
                                "name": "Read",
                                "description": "Read and return the contents of a file",
                                "parameters": {
                                "type": "object",
                                "properties": {
                                    "file_path": {
                                    "type": "string",
                                    "description": "The path to the file to read"
                                    }
                                },
                                "required": ["file_path"]
                                }
                            }
                        },
                        {
                            "type": "function",
                            "function": {
                                "name": "Write",
                                "description": "Write content to a file",
                                "parameters": {
                                "type": "object",
                                "required": ["file_path", "content"],
                                "properties": {
                                    "file_path": {
                                    "type": "string",
                                    "description": "The path of the file to write to"
                                    },
                                    "content": {
                                    "type": "string",
                                    "description": "The content to write to the file"
                                    }
                                }
                                }
                            }
                        }],
            }))
            .await?;
        let message = response["choices"][0]["message"].clone();
        eprintln!("{}", serde_json::to_string_pretty(&message)?);
        messages.push(message.clone());

    // Safely extract the message block from the first choice
        if let Some(message_obj) = message.as_object() {
            // 1. Check if the LLM generated any tool calls
            if let Some(tool_calls) = message_obj.get("tool_calls").and_then(|t| t.as_array()) {
                for tool_call in tool_calls {
                    let name = tool_call
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .ok_or("Missing tool name")?;

                    let arguments_str = tool_call
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .ok_or("Missing tool arguments")?;

                    let arguments: Value =
                        serde_json::from_str(arguments_str)?;

                    if name == "Read" {

                        let file_path = arguments
                            .get("file_path")
                            .and_then(|p| p.as_str())
                            .ok_or("Missing file path")?;

                        let contents =
                            std::fs::read_to_string(file_path)?;

                        let tool_call_id = tool_call["id"].as_str().ok_or("MIssing tool id")?;
                    }

                    if name == "Write" {
                        let file_path = arguments
                            .get("file_path")
                            .and_then(|p| p.as_str())
                            .ok_or("Missing file path")?;
                        let contents = arguments
                            .get("contents")
                            .and_then(|c| c.as_str())
                            .ok_or("Missing contents")?; 
                        std::fs::write(file_path, contents)?;
                    }
                    messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_call_id,
                            "content": contents
                        }))
                    }
                }
            continue 'agent;
            }
        
            // 2. Fallback: If no tool calls exist, print the regular assistant response
            if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                print!("{}", content);
                break 'agent;
            }

        
    }

    Ok(())
}
