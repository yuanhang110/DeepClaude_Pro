//! Response models for the API endpoints.
//!
//! This module defines the structures used to represent API responses,
//! including chat completions, usage statistics, and streaming events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
//use crate::handlers::format_cost;

/// Primary response structure for chat API endpoints.
///
/// Contains the complete response from both AI models, including
/// content blocks, usage statistics, and optional raw API responses.
#[derive(Debug, Serialize, Clone)]
pub struct ApiResponse {
    pub created: DateTime<Utc>,
    pub content: Vec<ContentBlock>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deepseek_response: Option<ExternalApiResponse>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_response: Option<ExternalApiResponse>,
    
    pub combined_usage: CombinedUsage,
}

/// A block of content in a response.
///
/// Represents a single piece of content in the response,
/// with its type and actual text content.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Raw response from an external API.
///
/// Contains the complete response details from an external API
/// call, including status code, headers, and response body.
#[derive(Debug, Serialize, Clone)]
pub struct ExternalApiResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: serde_json::Value,
}

/// Combined usage statistics from both AI models.
///
/// Aggregates token usage and cost information from both
/// DeepSeek and Anthropic API calls.
#[derive(Debug, Serialize, Clone)]
pub struct CombinedUsage {
    pub total_cost: String,
    pub deepseek_usage: DeepSeekUsage,
    pub anthropic_usage: AnthropicUsage,
}

/// Usage statistics for DeepSeek API calls.
///
/// Tracks token consumption and costs specific to
/// DeepSeek model usage.
#[derive(Debug, Serialize, Clone, Default)]
pub struct DeepSeekUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub reasoning_tokens: u32,
    pub cached_input_tokens: u32,
    pub total_tokens: u32,
    pub total_cost: String,
}

/// Usage statistics for Anthropic API calls.
///
/// Tracks token consumption and costs specific to
/// Anthropic model usage.
#[derive(Debug, Serialize, Clone, Default)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cached_write_tokens: u32,
    pub cached_read_tokens: u32,
    pub total_tokens: u32,
    pub total_cost: String,
}

// Streaming event types
/// Events emitted during streaming responses.
///
/// Represents different types of events that can occur
/// during a streaming response, including content updates
/// and usage statistics.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "start")]
    #[allow(dead_code)]
    Start {
        created: DateTime<Utc>,
    },
    
    #[serde(rename = "content")]
    #[allow(dead_code)]
    Content {
        content: Vec<ContentBlock>,
    },
    
    #[serde(rename = "usage")]
    #[allow(dead_code)]
    Usage {
        usage: CombinedUsage,
    },
    
    #[serde(rename = "done")]
    #[allow(dead_code)]
    Done,
    
    #[serde(rename = "error")]
    #[allow(dead_code)]
    Error {
        message: String,
        code: u16,
    },
}

impl ContentBlock {
    /// Creates a new text content block.
    ///
    /// # Arguments
    ///
    /// * `text` - The text content to include in the block
    ///
    /// # Returns
    ///
    /// A new `ContentBlock` with the type set to "text"
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content_type: "text".to_string(),
            text: text.into(),
        }
    }

    /// Converts an Anthropic content block to a generic content block.
    ///
    /// # Arguments
    ///
    /// * `block` - The Anthropic-specific content block to convert
    ///
    /// # Returns
    ///
    /// A new `ContentBlock` with the same content type and text
    pub fn from_anthropic(block: crate::clients::anthropic::ContentBlock) -> Self {
        Self {
            content_type: block.content_type,
            text: block.text,
        }
    }
}

impl ApiResponse {
    /// Creates a new API response with simple text content.
    ///
    /// # Arguments
    ///
    /// * `content` - The text content for the response
    ///
    /// # Returns
    ///
    /// A new `ApiResponse` with default values and the provided content
    #[allow(dead_code)]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            created: Utc::now(),
            content: vec![ContentBlock::text(content)],
            deepseek_response: None,
            anthropic_response: None,
            combined_usage: CombinedUsage {
                total_cost: "$0.00".to_string(),
                deepseek_usage: DeepSeekUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                    reasoning_tokens: 0,
                    cached_input_tokens: 0,
                    total_tokens: 0,
                    total_cost: "$0.00".to_string(),
                },
                anthropic_usage: AnthropicUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                    cached_write_tokens: 0,
                    cached_read_tokens: 0,
                    total_tokens: 0,
                    total_cost: "$0.00".to_string(),
                },
            },
        }
    }
}

impl AnthropicUsage {
    /// Converts Anthropic-specific usage statistics to the application's format.
    ///
    /// # Arguments
    ///
    /// * `usage` - The Anthropic-specific usage statistics to convert
    ///
    /// # Returns
    ///
    /// A new `AnthropicUsage` with values copied from the Anthropic usage
    #[allow(dead_code)]
    pub fn from_anthropic(usage: crate::clients::anthropic::Usage) -> Self {
        Self {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cached_write_tokens: usage.cache_creation_input_tokens,
            cached_read_tokens: usage.cache_read_input_tokens,
            total_tokens: usage.input_tokens + usage.output_tokens,
            total_cost: "$0.00".to_string(), // Cost will be calculated later
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Choice {
    pub index: i32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAICompatibleResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

// 在文件底部添加
impl From<OpenAICompatibleResponse> for ApiResponse {
    fn from(response: OpenAICompatibleResponse) -> Self {
        let content = response.choices[0].message.content.clone();
        let reasoning = response.choices[0].message.reasoning_content.clone();
        
        let mut content_blocks = Vec::new();
        
        // 添加thinking块
        content_blocks.push(ContentBlock {
            content_type: "text".to_string(),
            text: format!("<thinking>\n{}\n</thinking>", reasoning.unwrap_or_default()),
        });
        
        // 添加主要内容块
        content_blocks.push(ContentBlock {
            content_type: "text".to_string(),
            text: content,
        });

        ApiResponse {
            created: Utc::now(),
            content: content_blocks,
            deepseek_response: None,
            anthropic_response: None,
            combined_usage: CombinedUsage {
                total_cost: "$0.00".to_string(),
                deepseek_usage: DeepSeekUsage::default(),
                anthropic_usage: AnthropicUsage::default(),
            },
        }
    }
}