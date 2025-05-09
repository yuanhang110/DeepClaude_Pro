//! Anthropic API client implementation for interacting with Claude models.
//!
//! This module provides a client implementation for communicating with Anthropic's API,
//! specifically designed to work with Claude language models. It supports both streaming
//! and non-streaming interactions, handling all aspects of API communication including:
//!
//! - Authentication and request signing
//! - Message formatting and serialization
//! - Response parsing and deserialization
//! - Error handling and type conversion
//! - Streaming response processing
//!
//! # Main Components
//!
//! - [`AnthropicClient`]: The main client struct for making API requests
//! - [`AnthropicResponse`]: Represents the structured response from the API
//! - [`StreamEvent`]: Represents different types of events in streaming responses
//!
//! # Example Usage
//!
//! ```no_run
//! use deepclaude::clients::AnthropicClient;
//! use deepclaude::models::{Message, ApiConfig};
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = AnthropicClient::new("your-api-key".to_string());
//!     let messages = vec![/* your messages */];
//!     let config = ApiConfig::default();
//!
//!     // Non-streaming request
//!     let response = client.chat(messages.clone(), None, &config).await?;
//!
//!     // Streaming request
//!     let stream = client.chat_stream(messages, None, &config);
//!     Ok(())
//! }
//! ```

use crate::{
    error::{ApiError, Result},
    models::request::{ApiConfig, Message, Role},
};
use futures::Stream;
use reqwest::{header::HeaderMap, Client};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, pin::Pin};
use futures::StreamExt;
use serde_json;
use tracing;
use std::env;
use std::fs;
use std::path::PathBuf;

// 从.env文件中读取API URL，如果未设置则使用默认值
pub(crate) fn get_anthropic_api_url() -> String {
    read_env_from_dotenv("ANTHROPIC_API_URL").unwrap_or_else(|| String::from("https://api.gptsapi.net/v1/messages"))
}

// 从.env文件中读取Claude的OpenAI格式API URL，如果未设置则使用默认值
pub(crate) fn get_claude_openai_type_api_url() -> String {
    read_env_from_dotenv("CLAUDE_OPENAI_TYPE_API_URL").unwrap_or_else(|| String::from("https://api.claude-Plus.top/v1/chat/completions"))
}

// 从.env文件中读取DeepSeek的OpenAI格式API URL，如果未设置则使用默认值
pub(crate) fn get_deepseek_openai_type_api_url() -> String {
    read_env_from_dotenv("DEEPSEEK_OPENAI_TYPE_API_URL").unwrap_or_else(|| String::from("https://ark.cn-beijing.volces.com/api/v3/chat/completions"))
}

// 从.env文件中读取Claude模型名称，如果未设置则使用默认值
pub(crate) fn get_claude_default_model() -> String {
    read_env_from_dotenv("CLAUDE_DEFAULT_MODEL").unwrap_or_else(|| String::from("wild-3-7-sonnet-20250219"))
}

// 辅助函数，从.env文件读取配置
fn read_env_from_dotenv(key: &str) -> Option<String> {
    // 只从.env文件读取配置
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let env_path = current_dir.join(".env");
    
    // 确保.env文件存在
    if !ensure_env_file_exists(&env_path) {
        return None;
    }
    
    match fs::read_to_string(&env_path) {
        Ok(content) => {
            content.lines()
                .find(|line| line.starts_with(&format!("{}=", key)))
                .and_then(|line| line.split('=').nth(1))
                .map(|value| value.trim().trim_matches('"').to_string())
        }
        Err(e) => {
            tracing::debug!("读取.env文件失败: {}", e);
            None
        }
    }
}

// 确保.env文件存在
fn ensure_env_file_exists(env_path: &PathBuf) -> bool {
    if !env_path.exists() {
        tracing::error!(".env文件不存在: {:?}", env_path);
        tracing::error!("请在项目根目录创建.env文件并设置必要的配置项");
        return false;
    }
    true
}

// 为了向后兼容，保留这些常量，但它们现在使用函数获取值
#[allow(dead_code)]
pub(crate) const ANTHROPIC_API_URL: &str = "https://api.gptsapi.net/v1/messages";
#[allow(dead_code)]
pub(crate) const CLAUDE_OPENAI_TYPE_API_URL: &str = "https://api.claude-Plus.top/v1/chat/completions";
#[allow(dead_code)]
pub(crate) const DEEPSEEK_OPENAI_TYPE_API_URL: &str = "https://ark.cn-beijing.volces.com/api/v3/chat/completions";
//pub(crate) const ANTHROPIC_API_URL: &str = "https://anthropic.claude-plus.top/v1/messages";
/// pub(crate) const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages
// const DEFAULT_MODEL: &str = "claude-3-5-sonnet-20241022";
//const DEFAULT_MODEL: &str = "wild-3-5-sonnet-20241022";
#[allow(dead_code)]
const CLAUDE_DEFAULT_MODEL: &str = "wild-3-7-sonnet-20250219";
//const DEFAULT_MODEL: &str = "deepseek-v3-241226";
//const DEFAULT_MODEL: &str = "claude-3-7-sonnet-20250219";
/// Client for interacting with Anthropic's Claude models.
///
/// This client handles authentication, request construction, and response parsing
/// for both streaming and non-streaming interactions with Anthropic's API.
///
/// # Examples
///
/// ```no_run
/// use deepclaude::clients::AnthropicClient;
///
/// let client = AnthropicClient::new("api_token".to_string());
/// ```
#[derive(Debug)]
pub struct AnthropicClient {
    pub(crate) client: Client,
    _api_token: String,  // 添加下划线前缀，表示有意不使用
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub model: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AnthropicRequest {
    messages: Vec<AnthropicMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(flatten)]
    additional_params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AnthropicMessage {
    role: String,
    content: String,
}

// Event types for streaming responses
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "message_start")]
    MessageStart {
        #[allow(dead_code)]
        message: AnthropicResponse,
    },
    #[serde(rename = "content_block_start")]
    #[allow(dead_code)]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    #[allow(dead_code)]
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    #[serde(rename = "content_block_stop")]
    #[allow(dead_code)]
    ContentBlockStop {
        index: usize,
    },
    #[serde(rename = "message_delta")]
    #[allow(dead_code)]
    MessageDelta {
        delta: MessageDelta,
        #[serde(default)]
        usage: Option<Usage>,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContentDelta {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub delta_type: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct MessageDelta {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

impl AnthropicClient {
    /// Creates a new Anthropic client instance.
    ///
    /// # Arguments
    ///
    /// * `api_token` - API token for authentication with Anthropic's API
    ///
    /// # Returns
    ///
    /// A new `AnthropicClient` instance configured with the provided API token
    pub fn new(api_token: String) -> Self {
        Self {
            client: Client::new(),
            _api_token: api_token,
        }
    }

    /// Builds the HTTP headers required for Anthropic API requests.
    ///
    /// # Arguments
    ///
    /// * `custom_headers` - Optional additional headers to include in requests
    /// * `is_deepseek` - Whether the request is for Deepseek API
    ///
    /// # Returns
    ///
    /// * `Result<HeaderMap>` - The constructed headers on success, or an error if header construction fails
    ///
    /// # Errors
    ///
    /// Returns `ApiError::Internal` if:
    /// - The API token is invalid
    /// - Content-Type or Anthropic-Version headers cannot be constructed
    pub(crate) fn build_headers(&self, custom_headers: Option<&HashMap<String, String>>, is_deepseek: bool) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        
        // 根据API类型添加不同的认证头
        if is_deepseek {
            // DeepSeek API认证
            let deepseek_token = read_env_from_dotenv("DEEPSEEK_API_KEY")
                .ok_or_else(|| ApiError::Internal { 
                    message: "未在.env文件中找到DEEPSEEK_API_KEY".to_string() 
                })?;
            
            headers.insert(
                "Authorization",
                format!("Bearer {}", deepseek_token)
                    .parse()
                    .map_err(|e| ApiError::Internal { 
                        message: format!("无效的Authorization头: {}", e) 
                    })?,
            );
        } else if should_use_openai_format() {
            // OpenAI格式API认证
            let api_token = self._api_token.clone();
            
            headers.insert(
                "Authorization",
                format!("Bearer {}", api_token)
                    .parse()
                    .map_err(|e| ApiError::Internal { 
                        message: format!("无效的Authorization头: {}", e) 
                    })?,
            );
            
            // OpenAI格式API不需要额外的头部
        } else {
            // Anthropic原生格式API认证
            // 从.env文件获取API密钥
            let anthropic_token = read_env_from_dotenv("ANTHROPIC_API_KEY")
                .ok_or_else(|| ApiError::Internal { 
                    message: "未在.env文件中找到ANTHROPIC_API_KEY".to_string() 
                })?;
            
            headers.insert(
                "x-api-key",
                anthropic_token
                    .parse()
                    .map_err(|e| ApiError::Internal { 
                        message: format!("无效的API令牌: {}", e) 
                    })?,
            );
            
            // 添加Authorization头
            headers.insert(
                "Authorization",
                format!("Bearer {}", anthropic_token)
                    .parse()
                    .map_err(|e| ApiError::Internal { 
                        message: format!("无效的Authorization头: {}", e) 
                    })?,
            );
            
            // Anthropic特有的版本头
            headers.insert(
                "anthropic-version",
                "2023-06-01"
                    .parse()
                    .map_err(|e| ApiError::Internal { 
                        message: format!("无效的anthropic版本: {}", e) 
                    })?,
            );

            // 添加流式处理所需的头部
            headers.insert(
                "accept",
                "text/event-stream"
                    .parse()
                    .map_err(|e| ApiError::Internal {
                        message: format!("无效的accept头: {}", e)
                    })?,
            );
        }
        
        // 通用头部
        headers.insert(
            "content-type",
            "application/json"
                .parse()
                .map_err(|e| ApiError::Internal { 
                    message: format!("无效的内容类型: {}", e) 
                })?,
        );

        // 添加自定义头部
        if let Some(custom) = custom_headers {
            headers.extend(super::build_headers(custom)?);
        }

        tracing::debug!("最终请求头: {:?}", headers);

        Ok(headers)
    }

    /// Constructs a request object for the Anthropic API.
    ///
    /// # Arguments
    ///
    /// * `messages` - Vector of messages to send to the model
    /// * `system` - Optional system prompt to set context
    /// * `stream` - Whether to enable streaming mode
    /// * `config` - Configuration options for the request
    ///
    /// # Returns
    ///
    /// An `AnthropicRequest` object configured with the provided parameters and defaults
    pub(crate) fn build_request(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        stream: bool,
        config: &ApiConfig,
    ) -> AnthropicRequest {
        let filtered_messages = messages
            .into_iter()
            .filter(|msg| msg.role != Role::System)
            .filter(|msg| !msg.content.trim().is_empty())
            .map(|msg| AnthropicMessage {
                role: match msg.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => unreachable!(),
                },
                content: msg.content,
            })
            .collect();

        // Create base request with required fields
        let default_model = get_claude_default_model();
        let default_model_json = serde_json::json!(default_model);
        let model_value = config.body.get("model").unwrap_or(&default_model_json);
        let model_str = model_value.as_str().unwrap_or(&default_model);
        let _is_deepseek = model_str.starts_with("deepseek") || model_str == "deepclaude";
        
        let default_max_tokens = if let Some(model_str) = model_value.as_str() {
            if model_str.contains("claude-3-opus") {
                4096
            } else {
                8192
            }
        } else {
            8192
        };
        let default_max_tokens_json = serde_json::json!(default_max_tokens);

        let mut request_value = serde_json::json!({
            "messages": filtered_messages,
            "stream": stream,
            "model": model_value,
            "max_tokens": config.body.get("max_tokens").unwrap_or(&default_max_tokens_json),
            "temperature": config.body.get("temperature").unwrap_or(&serde_json::json!(0.7)),
            "top_p": config.body.get("top_p").unwrap_or(&serde_json::json!(0.95))
        });

        // Add system if present
        if let Some(ref sys) = system {
            if let serde_json::Value::Object(mut map) = request_value {
                map.insert("system".to_string(), serde_json::json!(sys));
                request_value = serde_json::Value::Object(map);
            }
        }

        // Merge additional configuration from config.body while protecting critical fields
        if let serde_json::Value::Object(mut map) = request_value {
            if let serde_json::Value::Object(mut body) = serde_json::to_value(&config.body).unwrap_or_default() {
                // Remove protected fields from config body
                body.remove("stream");
                body.remove("messages");
                body.remove("system");
                
                // Merge remaining fields from config.body
                for (key, value) in body {
                    map.insert(key, value);
                }
            }
            request_value = serde_json::Value::Object(map);
        }

        //tracing::debug!("请求体: {}", serde_json::to_string_pretty(&request_value).unwrap_or_default());

        // Convert the merged JSON value into our request structure
        serde_json::from_value(request_value).unwrap_or_else(|_| AnthropicRequest {
            messages: filtered_messages,
            stream,
            system,
            additional_params: config.body.clone(),
        })
    }

    /// Sends a non-streaming chat request to the Anthropic API.
    ///
    /// # Arguments
    ///
    /// * `messages` - Vector of messages for the conversation
    /// * `system` - Optional system prompt to set context
    /// * `config` - Configuration options for the request
    ///
    /// # Returns
    ///
    /// * `Result<AnthropicResponse>` - The model's response on success
    ///
    /// # Errors
    ///
    /// Returns `ApiError::AnthropicError` if:
    /// - The API request fails
    /// - The response status is not successful
    /// - The response cannot be parsed
    pub async fn chat(
        &self,
        messages: Vec<Message>,
        system: Option<String>,
        config: &ApiConfig,
    ) -> Result<AnthropicResponse> {
        // 验证消息不为空
        if messages.is_empty() {
            return Err(ApiError::AnthropicError {
                message: "消息不能为空".to_string(),
                type_: "validation_error".to_string(),
                param: None,
                code: None
            });
        }

        // 验证最后一条消息
        if let Some(last_msg) = messages.last() {
            if last_msg.role == Role::Assistant && last_msg.content.trim().is_empty() {
                return Err(ApiError::AnthropicError {
                    message: "最后一条assistant消息不能为空".to_string(),
                    type_: "validation_error".to_string(),
                    param: None,
                    code: None
                });
            }
        }

        // 获取模型名称，决定使用哪个API端点
        let default_model = get_claude_default_model();
        let default_model_json = serde_json::json!(default_model);
        let model_value = config.body.get("model").unwrap_or(&default_model_json);
        let model_str = model_value.as_str().unwrap_or(&default_model);
        let _is_deepseek = model_str.starts_with("deepseek") || model_str == "deepclaude";
        
        // 选择API端点
        let api_url = if _is_deepseek {
            get_deepseek_openai_type_api_url()
        } else if should_use_openai_format() {
            // 使用OpenAI格式的API
            get_claude_openai_type_api_url()
        } else {
            // 使用Anthropic原生API
            get_anthropic_api_url()
        };
        
        // 构建请求头和请求体
        let headers = self.build_headers(Some(&config.headers), _is_deepseek)?;
        let request = self.build_request(messages, system, false, config);
        
        // 记录请求信息
        tracing::debug!("API请求URL: {}", api_url);
        tracing::debug!("API请求头: {:?}", headers);
        //tracing::debug!("Anthropic请求体: {}", serde_json::to_string(&request).unwrap_or_default());
        
        // 发送请求
        let response = self.client
            .post(api_url)
            .headers(headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| ApiError::AnthropicError {
                message: format!("请求失败: {}", e),
                type_: "request_failed".to_string(),
                param: None,
                code: None
            })?;
        
        let _status = response.status();
        let raw_response = response.text().await.map_err(|e| ApiError::AnthropicError {
            message: format!("获取响应文本失败: {}", e),
            type_: "io_error".to_string(),
            param: None,
            code: None
        })?;

        tracing::debug!("原始Anthropic块的响应: {}", raw_response);

        // 处理不同API的响应格式
        if _is_deepseek {
            // 处理Deepseek API响应
            return parse_deepseek_response(&raw_response);
        } else {
            // 处理原有Anthropic API响应
            // 即使响应包含错误信息，也尝试提取有效内容
            if raw_response.contains("id") && raw_response.contains("content") && (raw_response.contains("message") || raw_response.contains("text")) {
                // 优先尝试标准格式解析
                if let Ok(data) = serde_json::from_str::<AnthropicResponse>(&raw_response) {
                    return Ok(data);
                }
                
                // 尝试提取内容
                if let Ok(content_blocks) = extract_content_from_response(&raw_response) {
                    if !content_blocks.is_empty() && !content_blocks[0].text.is_empty() {
                        // 构造响应
                        return Ok(AnthropicResponse {
                            id: extract_id_from_response(&raw_response).unwrap_or_else(|| "generated_id".to_string()),
                            response_type: "message".to_string(),
                            role: "assistant".to_string(),
                            model: {
                                let default_model = get_claude_default_model();
                                extract_model_from_response(&raw_response).unwrap_or_else(|| default_model)
                            },
                            content: content_blocks,
                            stop_reason: Some("stop".to_string()),
                            stop_sequence: None,
                            usage: extract_usage_from_response(&raw_response).unwrap_or_default(),
                        });
                    }
                }
            }
        }
        
        // 如果无法提取任何有效内容，则返回错误
        Err(ApiError::AnthropicError {
            message: format!("无法解析响应: {}", raw_response),
            type_: "parse_error".to_string(),
            param: None,
            code: None
        })
    }

    /// Sends a streaming chat request to the Anthropic API.
    ///
    /// Returns a stream that yields events from the model's response as they arrive.
    ///
    /// # Arguments
    ///
    /// * `messages` - Vector of messages for the conversation
    /// * `system` - Optional system prompt to set context
    /// * `config` - Configuration options for the request
    ///
    /// # Returns
    ///
    /// * `Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>` - A stream of response events
    ///
    /// # Errors
    ///
    /// The stream may yield `ApiError::AnthropicError` if:
    /// - The API request fails
    /// - Stream processing encounters an error
    /// - Response events cannot be parsed
    pub fn chat_stream<'a>(
        &'a self,
        messages: Vec<Message>,
        system: Option<String>,
        config: &'a ApiConfig,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + 'a>> {
        // 获取模型名称，决定使用哪个API端点
        let default_model = get_claude_default_model();
        let default_model_json = serde_json::json!(default_model);
        let model_value = config.body.get("model").unwrap_or(&default_model_json);
        let model_str = model_value.as_str().unwrap_or(&default_model);
        let _is_deepseek = model_str.starts_with("deepseek") || model_str == "deepclaude";
        
        // 选择API端点
        let api_url = if _is_deepseek {
            get_deepseek_openai_type_api_url()
        } else if should_use_openai_format() {
            // 使用OpenAI格式的API
            get_claude_openai_type_api_url()
        } else {
            // 使用Anthropic原生API
            get_anthropic_api_url()
        };
        
        tracing::info!("使用API端点: {}, 模型: {}", api_url, model_str);
        
        let headers = match self.build_headers(Some(&config.headers), _is_deepseek) {
            Ok(h) => h,
            Err(e) => return Box::pin(futures::stream::once(async move { Err(e) })),
        };

        // 克隆需要在异步流中使用的值
        let messages = messages.clone();
        let system = system.clone();
        let request = self.build_request(messages, system, true, config);
        let client = self.client.clone();

        Box::pin(async_stream::stream! {
            let response = match client
                .post(api_url)
                .headers(headers)
                .json(&request)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    yield Err(ApiError::AnthropicError { 
                        message: format!("请求失败: {}", e),
                        type_: "request_failed".to_string(),
                        param: None,
                        code: None
                    });
                    return;
                }
            };
            
            let status = response.status();
            tracing::debug!("流式响应状态码: {}", status);
            
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_else(|_| "无法获取错误详情".to_string());
                tracing::error!("API返回错误: {} - {}", status, error_text);
                yield Err(ApiError::AnthropicError { 
                    message: format!("API返回错误: {} - {}", status, error_text),
                    type_: "api_error".to_string(),
                    param: None,
                    code: Some(status.as_u16().to_string())
                });
                return;
            }
            
            let mut stream = response.bytes_stream();
            let mut data = String::new();
            let mut content_buffer = String::new();
            let mut _has_content = false;
            let mut stream_ended = false;
            
            tracing::debug!("开始处理流式响应");
            
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                            // 检查是否为OpenAI格式的最终标记
                            if text.trim() == "data: [DONE]" {
                                tracing::debug!("接收到OpenAI格式的流结束标记");
                                stream_ended = true;
                                yield Ok(StreamEvent::MessageStop);
                                break;
                            }
                            
                            // 处理流式响应
                            let lines: Vec<&str> = text.lines().collect();
                            
                            for line in lines {
                                // 跳过空行
                                if line.trim().is_empty() {
                                    continue;
                                }
                                
                                // 处理OpenAI格式的数据行
                                if line.starts_with("data: ") {
                                    let json_str = &line[6..]; // 移除 "data: " 前缀
                                    
                                    // 跳过[DONE]标记，已在前面处理
                                    if json_str.trim() == "[DONE]" {
                                        continue;
                                    }
                                    
                                    // 先尝试解析为OpenAI格式
                                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        // 调试输出原始JSON
                                        tracing::debug!("OpenAI格式原始响应: {}", json_str);
                                        
                                        // 检查是否有choices字段，判断是否为OpenAI格式
                                        if let Some(choices) = json_value.get("choices").and_then(|v| v.as_array()) {
                                            if !choices.is_empty() {
                                                // 提取delta内容
                                                if let Some(choice) = choices.first() {
                                                    // 提取delta中的content字段
                                                    if let Some(delta) = choice.get("delta") {
                                                        let content = delta.get("content").and_then(|c| c.as_str());
                                                        if let Some(content_str) = content {
                                                            if !content_str.is_empty() {
                                                                tracing::debug!("解析到OpenAI格式的内容: {}", content_str);
                                                                content_buffer.push_str(content_str);
                                                                yield Ok(StreamEvent::ContentBlockDelta {
                                                                    index: 0,
                                                                    delta: ContentDelta {
                                                                        delta_type: "text".to_string(),
                                                                        text: content_str.to_string(),
                                                                    },
                                                                });
                                                            }
                                                        }
                                                        continue;
                                                    }
                                                    
                                                    // 检查是否为完成原因
                                                    if let Some(finish_reason) = choice.get("finish_reason") {
                                                        if !finish_reason.is_null() {
                                                            tracing::debug!("检测到完成原因: {:?}", finish_reason);
                                                            stream_ended = true;
                                                            yield Ok(StreamEvent::MessageStop);
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            // 处理没有type字段的JSON响应
                                            if json_value.get("type").is_none() && json_value.get("choices").is_some() {
                                                tracing::debug!("处理没有type字段的OpenAI格式响应");
                                                // 这里是处理最后一个块的代码，通常包含完整的usage信息
                                                // 如果需要提取usage信息并更新，可以在这里添加代码
                                                
                                                // 由于这不是流式内容块，我们只需记录并继续处理
                                                tracing::info!("收到非流式块: {}", json_str);
                                                continue;
                                            }
                                        }
                                    }
                                    
                                    // 如果不是OpenAI格式，尝试解析为Anthropic格式
                                    match serde_json::from_str::<StreamEvent>(json_str) {
                                        Ok(event) => {
                                            _has_content = true;
                                            match &event {
                                                StreamEvent::ContentBlockDelta { delta, .. } => {
                                                    //tracing::debug!("解析到Anthropic格式的内容: {}", delta.text);
                                                    content_buffer.push_str(&delta.text);
                                                }
                                                StreamEvent::MessageStop => {
                                                    tracing::debug!("收到消息结束事件");
                                                    stream_ended = true;
                                                }
                                                _ => {
                                                    tracing::debug!("收到其他类型事件: {:?}", event);
                                                }
                                            }
                                            yield Ok(event);
                                        }
                                        Err(e) => {
                                            // 检查错误是否是因为不完整的JSON
                                            let err_msg = e.to_string();
                                            if err_msg.contains("EOF while parsing") || err_msg.contains("unexpected end of input") {
                                                // 这是不完整的JSON，只记录调试信息，不返回错误
                                                tracing::debug!("收到不完整的JSON数据，跳过处理: {}", json_str);
                                                continue;
                                            }
                                            
                                            // 只记录关键错误，不记录所有解析失败
                                            if !json_str.contains("ping") && !json_str.contains("HEARTBEAT") {
                                                tracing::error!("解析事件JSON失败: {} - {}", e, json_str);
                                            }
                                            // 不要为所有解析错误生成错误事件
                                            if json_str != "[DONE]" && !json_str.contains("HEARTBEAT") {
                                                yield Err(ApiError::Internal {
                                                    message: format!("Failed to parse event JSON: {}", e),
                                                });
                                            }
                                        }
                                    }
                                } else {
                                    //tracing::debug!("跳过非data事件: {}", raw_event);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("读取数据块时出错: {}", e);
                        yield Err(ApiError::AnthropicError { 
                            message: format!("Stream error: {}", e),
                            type_: "stream_error".to_string(),
                            param: None,
                            code: None
                        });
                        return;
                    }
                }

                // 如果流已经结束，不再继续处理
                if stream_ended {
                    break;
                }
            }
        })
    }
}

/// Converts an Anthropic content block into the application's generic content block type.
impl From<ContentBlock> for crate::models::response::ContentBlock {
    fn from(block: ContentBlock) -> Self {
        Self {
            content_type: block.content_type,
            text: block.text,
        }
    }
}

// 辅助函数，从非标准响应中提取内容
fn extract_content_from_response(raw_response: &str) -> Result<Vec<ContentBlock>> {
    // 尝试将响应解析为JSON对象
    let json_value: serde_json::Value = serde_json::from_str(raw_response)
        .map_err(|e| ApiError::AnthropicError {
            message: format!("解析JSON失败: {}", e),
            type_: "parse_error".to_string(),
            param: None,
            code: None
        })?;
    
    // 尝试不同的路径提取内容
    let content_text = if let Some(content) = json_value.get("content") {
        // 支持数组格式内容
        if content.is_array() {
            let mut text = String::new();
            for item in content.as_array().unwrap() {
                if let Some(item_text) = item.get("text").and_then(|t| t.as_str()) {
                    text.push_str(item_text);
                }
            }
            text
        } else if let Some(text) = content.as_str() {
            // 直接字符串内容
            text.to_string()
        } else {
            // 内容是其他格式
            json_value.to_string()
        }
    } else if let Some(choices) = json_value.get("choices") {
        // OpenAI格式响应
        if let Some(choice) = choices.as_array().and_then(|arr| arr.first()) {
            if let Some(message) = choice.get("message") {
                if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                    content.to_string()
                } else {
                    json_value.to_string()
                }
            } else {
                json_value.to_string()
            }
        } else {
            json_value.to_string()
        }
    } else {
        // 找不到任何识别的内容格式
        json_value.to_string()
    };
    
    // 返回提取的内容
    Ok(vec![ContentBlock {
        content_type: "text".to_string(),
        text: content_text,
    }])
}

// 从响应中提取ID
fn extract_id_from_response(raw_response: &str) -> Option<String> {
    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(raw_response) {
        if let Some(id) = json_value.get("id").and_then(|id| id.as_str()) {
            return Some(id.to_string());
        }
    }
    None
}

// 从响应中提取模型名称
fn extract_model_from_response(raw_response: &str) -> Option<String> {
    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(raw_response) {
        if let Some(model) = json_value.get("model").and_then(|m| m.as_str()) {
            return Some(model.to_string());
        }
    }
    None
}

// 从响应中提取用量信息
fn extract_usage_from_response(raw_response: &str) -> Option<Usage> {
    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(raw_response) {
        if let Some(usage) = json_value.get("usage") {
            if let Ok(usage_data) = serde_json::from_value::<Usage>(usage.clone()) {
                return Some(usage_data);
            }
        }
    }
    None
}

// 添加解析Deepseek响应的函数
fn parse_deepseek_response(raw_response: &str) -> Result<AnthropicResponse> {
    // 尝试将响应解析为JSON对象
    let json_value: serde_json::Value = serde_json::from_str(raw_response)
        .map_err(|e| ApiError::AnthropicError {
            message: format!("解析Deepseek响应JSON失败: {}", e),
            type_: "parse_error".to_string(),
            param: None,
            code: None
        })?;
    
    // 提取必要信息
    let id = json_value.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("deepseek_generated_id")
        .to_string();
    
    let default_model = get_claude_default_model();
    let model = json_value.get("model")
        .and_then(|v| v.as_str())
        .unwrap_or(&default_model)
        .to_string();
    
    // 提取内容
    let content_text = if let Some(choices) = json_value.get("choices").and_then(|v| v.as_array()) {
        if let Some(first_choice) = choices.first() {
            if let Some(message) = first_choice.get("message") {
                if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                    content.to_string()
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        }
    } else {
        "".to_string()
    };
    
    // 提取用量信息
    let usage = if let Some(usage_obj) = json_value.get("usage") {
        let prompt_tokens = usage_obj.get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        
        let completion_tokens = usage_obj.get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        
        Usage {
            input_tokens: prompt_tokens,
            output_tokens: completion_tokens,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        }
    } else {
        Usage::default()
    };
    
    // 提取停止原因
    let stop_reason = if let Some(choices) = json_value.get("choices").and_then(|v| v.as_array()) {
        if let Some(first_choice) = choices.first() {
            first_choice.get("finish_reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        }
    } else {
        None
    };
    
    // 构造内容块
    let content = vec![ContentBlock {
        content_type: "text".to_string(),
        text: content_text,
    }];
    
    // 返回标准化的响应
    Ok(AnthropicResponse {
        id,
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        model,
        content,
        stop_reason,
        stop_sequence: None,
        usage,
    })
}

// 判断是否应该使用OpenAI格式的API（基于.env文件中的配置）
pub(crate) fn should_use_openai_format() -> bool {
    let claude_openai_url = read_env_from_dotenv("CLAUDE_OPENAI_TYPE_API_URL");
    let anthropic_url = read_env_from_dotenv("ANTHROPIC_API_URL");
    
    match (claude_openai_url, anthropic_url) {
        (Some(openai_url), _) if !openai_url.trim().is_empty() => true,
        (_, Some(anthro_url)) if !anthro_url.trim().is_empty() => false,
        _ => true  // 默认使用OpenAI格式，如果.env文件中两者都为空
    }
}
