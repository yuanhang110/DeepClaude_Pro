//! Request handlers for the API endpoints.
//!
//! This module contains the main request handlers and supporting functions
//! for processing chat requests, including both streaming and non-streaming
//! responses. It coordinates between different AI models and handles
//! usage tracking and cost calculations.
use crate::{
    clients::{AnthropicClient, DeepSeekClient},
    config::Config,
    error::{ApiError, Result, SseResponse},
};
use crate::models::{
    request::{ApiRequest, Role},
    response::{
        ApiResponse, AnthropicUsage, Choice, ContentBlock, CombinedUsage,
        DeepSeekUsage, ExternalApiResponse, Message as ResponseMessage,
        OpenAICompatibleResponse, Usage,
    },
};
use crate::clients::anthropic::StreamEvent;
use crate::models::request::Message;
use axum::{
    extract::State,
    response::{sse::Event, IntoResponse, Json},
    Json as AxumJson,
};
use chrono::{Utc, Duration};
use futures::StreamExt;
use std::{sync::Arc, collections::HashMap};
use tokio_stream::wrappers::ReceiverStream;
use crate::clients::deepseek::get_deepseek_default_model;
use dotenv::dotenv;
use std::fs;
use std::io::Write;
use serde::Deserialize;
use serde_json::json;
use crate::utils;

/// Application state shared across request handlers.
///
/// Contains configuration that needs to be accessible
/// to all request handlers.
pub struct AppState {
    pub config: Config,
}
impl AppState {
    pub fn new(config: Config) -> Self {
        AppState { config }
    }
}
/// Extracts API tokens from request headers.
///
/// # Arguments
///
/// * `headers` - The HTTP headers containing the API tokens
///
/// # Returns
///
/// * `Result<(String, String)>` - A tuple of (DeepSeek token, Anthropic token)
///
/// # Errors
///
/// Returns `ApiError::MissingHeader` if either token is missing
/// Returns `ApiError::BadRequest` if tokens are malformed
/// 从.env文件中获取API tokens
#[allow(dead_code)]
fn get_env_api_tokens() -> Option<(String, String)> {
    // 获取当前目录
    let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let env_path = current_dir.join(".env");
    
    // 确保.env文件存在
    if !env_path.exists() {
        tracing::error!(".env文件不存在: {:?}", env_path);
        return None;
    }
    
    // 读取.env文件内容
    let content = match std::fs::read_to_string(&env_path) {
        Ok(content) => content,
        Err(e) => {
            tracing::debug!("读取.env文件失败: {}", e);
            return None;
        }
    };
    
    // 解析.env文件中的环境变量
    let mut deepseek_key = None;
    let mut anthropic_key = None;
    
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim();
            let value = line[pos + 1..].trim();
            
            if key == "DEEPSEEK_API_KEY" {
                deepseek_key = Some(value.to_string());
            } else if key == "ANTHROPIC_API_KEY" {
                anthropic_key = Some(value.to_string());
            }
        }
    }
    
    match (deepseek_key, anthropic_key) {
        (Some(d), Some(a)) => Some((d, a)),
        _ => {
            tracing::debug!("无法从.env文件获取API密钥，将尝试从请求头获取");
            None
        }
    }
}

/// 从Authorization header中提取token
#[allow(dead_code)]
fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers.get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(String::from)
}

/// 验证bearer token是否有效
#[allow(dead_code)]
fn validate_bearer_token(token: &str) -> bool {
    // 这里添加您的token验证逻辑
    // 示例中简单判断是否等于环境变量中的值
    std::env::var("API_TOKEN").map(|env_token| token == env_token).unwrap_or(false)
}

/// 从请求头中提取API tokens
fn extract_api_tokens(headers: &axum::http::HeaderMap) -> Result<(String, String)> {
    // 首先尝试从请求头中获取
    let deepseek_token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(String::from);

    let anthropic_token = headers
        .get("X-Anthropic-API-Token")
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    // 如果请求头中有完整的token，直接返回
    if let (Some(deepseek), Some(anthropic)) = (deepseek_token.clone(), anthropic_token.clone()) {
        tracing::debug!("成功从请求头获取API密钥");
        return Ok((deepseek, anthropic));
    }

    // 如果请求头中没有完整的token，尝试从环境变量获取
    if let Some((deepseek, anthropic)) = get_env_api_tokens() {
        tracing::debug!("成功从环境变量获取API密钥");
        return Ok((deepseek, anthropic));
    }

    // 如果都没有找到，返回详细的错误信息
    let mut missing_headers = Vec::new();
    if deepseek_token.is_none() {
        missing_headers.push("Authorization");
    }
    if anthropic_token.is_none() {
        missing_headers.push("X-Anthropic-API-Token");
    }

    Err(ApiError::MissingHeader {
        header: format!("缺少必要的认证信息：{}。请确保在请求头中提供这些信息，或在环境变量中设置DEEPSEEK_API_KEY和ANTHROPIC_API_KEY", 
            missing_headers.join(", "))
    })
}

/// Calculates the cost of DeepSeek API usage.
///
/// # Arguments
///
/// * `input_tokens` - Number of input tokens processed
/// * `output_tokens` - Number of output tokens generated
/// * `_reasoning_tokens` - Number of tokens used for reasoning
/// * `cached_tokens` - Number of tokens retrieved from cache
/// * `config` - Configuration containing pricing information
///
/// # Returns
///
/// The total cost in dollars for the API usage
fn calculate_deepseek_cost(
    input_tokens: u32,
    output_tokens: u32,
    _reasoning_tokens: u32,
    cached_tokens: u32,
    config: &Config,
) -> f64 {
    let cache_hit_cost = (cached_tokens as f64 / 1_000_000.0) * config.pricing.deepseek.input_cache_hit_price;
    let cache_miss_cost = ((input_tokens - cached_tokens) as f64 / 1_000_000.0) * config.pricing.deepseek.input_cache_miss_price;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * config.pricing.deepseek.output_price;
    
    cache_hit_cost + cache_miss_cost + output_cost
}

/// Calculates the cost of Anthropic API usage.
///
/// # Arguments
///
/// * `model` - The specific Claude model used
/// * `input_tokens` - Number of input tokens processed
/// * `output_tokens` - Number of output tokens generated
/// * `cache_write_tokens` - Number of tokens written to cache
/// * `cache_read_tokens` - Number of tokens read from cache
/// * `config` - Configuration containing pricing information
///
/// # Returns
///
/// The total cost in dollars for the API usage
fn calculate_anthropic_cost(
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
    cache_write_tokens: u32,
    cache_read_tokens: u32,
    config: &Config,
) -> f64 {
    let pricing = if model.contains("claude-3-5-sonnet") {
        &config.pricing.anthropic.claude_3_sonnet
    } else if model.contains("claude-3-5-haiku") {
        &config.pricing.anthropic.claude_3_haiku
    } else if model.contains("claude-3-opus") {
        &config.pricing.anthropic.claude_3_opus
    } else {
        &config.pricing.anthropic.claude_3_sonnet // default to sonnet pricing
    };

    let input_cost = (input_tokens as f64 / 1_000_000.0) * pricing.input_price;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * pricing.output_price;
    let cache_write_cost = (cache_write_tokens as f64 / 1_000_000.0) * pricing.cache_write_price;
    let cache_read_cost = (cache_read_tokens as f64 / 1_000_000.0) * pricing.cache_read_price;

    input_cost + output_cost + cache_write_cost + cache_read_cost
}

/// Formats a cost value as a dollar amount string.
///
/// # Arguments
///
/// * `cost` - The cost value to format
///
/// # Returns
///
/// A string representing the cost with 3 decimal places and $ prefix
pub(crate) fn format_cost(cost: f64) -> String {
    format!("${:.2}", cost)
}

/// 获取MODE环境变量，决定DeepSeek和Claude之间的交互模式
/// 
/// 返回值:
/// - "normal": 只将DeepSeek的推理内容传递给Claude（默认）
/// - "full": 将DeepSeek的最终结果都传递给Claude
fn get_mode() -> String {
    utils::get_mode()
}

/// Main handler for chat requests.
///
/// Routes requests to either streaming or non-streaming handlers
/// based on the request configuration.
///
/// # Arguments
///
/// * `state` - Application state containing configuration
/// * `headers` - HTTP request headers
/// * `request` - The parsed chat request
///
/// # Returns
///
/// * `Result<Response>` - The API response or an error
pub async fn handle_chat(
    state: State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(request): Json<ApiRequest>,
) -> Result<axum::response::Response> {
    if request.stream {
        let stream_response = chat_stream(state, headers, Json(request)).await?;
        Ok(stream_response.into_response())
    } else {
        let json_response = chat(state, headers, Json(request)).await?;
        Ok(json_response.into_response())
    }
}

/// Handler for non-streaming chat requests.
///
/// Processes the request through both AI models sequentially,
/// combining their responses and tracking usage.
///
/// # Arguments
///
/// * `state` - Application state containing configuration
/// * `headers` - HTTP request headers
/// * `request` - The parsed chat request
///
/// # Returns
///
/// * `Result<Json<ApiResponse>>` - The combined API response or an error
pub(crate) async fn chat(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(request): Json<ApiRequest>,
) -> Result<Json<OpenAICompatibleResponse>> {
    // Validate system prompt
    if !request.validate_system_prompt() {
        return Err(ApiError::InvalidSystemPrompt);
    }

    // Extract API tokens
    let (deepseek_token, anthropic_token) = extract_api_tokens(&headers)?;

    // Initialize clients
    let deepseek_client = DeepSeekClient::new(deepseek_token);
    let anthropic_client = AnthropicClient::new(anthropic_token);

    // 获取当前模式
    let mode = get_mode();
    
    // 获取系统提示和消息
    let messages = if mode == "full" {
        // full模式下使用带有特定系统提示的消息
        request.get_messages_with_system()
    } else {
        // normal模式下只使用原始消息
        let mut messages = Vec::new();
        
        // 添加系统消息（如果有）
        if let Some(system) = &request.system {
            messages.push(Message {
                role: Role::System,
                content: system.clone(),
            });
        }
        
        // 添加剩余的消息
        messages.extend(request.messages.iter().filter(|msg| !matches!(msg.role, Role::System)).cloned());
        
        messages
    };

    // Call DeepSeek API
    let deepseek_response = deepseek_client.chat(messages.clone(), &request.deepseek_config).await?;
    
    // Store response metadata
    let _deepseek_status: u16 = 200;
    let _deepseek_headers: HashMap<String, String> = HashMap::new(); // Headers not available when using high-level chat method

    // Extract reasoning content and wrap in thinking tags
    let reasoning_content = deepseek_response
        .choices
        .first()
        .and_then(|c| c.message.reasoning_content.as_ref())
        .ok_or_else(|| ApiError::DeepSeekError { 
            message: "No reasoning content in response".to_string(),
            type_: "missing_content".to_string(),
            param: None,
            code: None
        })?;

    // 获取DeepSeek的普通内容
    let empty_string = String::new();
    let normal_content = deepseek_response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .unwrap_or(&empty_string);

    // 检查内容是否存在
    let has_normal_content = !normal_content.trim().is_empty();
    
    // 将推理内容和普通内容组合在一起
    let thinking_content = if has_normal_content && mode == "full" {
        // 在full模式下，只包含原始回答
        format!("<thinking>\ndeepseek原始回答:{}</thinking>", normal_content)
    } else {
        format!("<thinking>\n{}\n</thinking>", reasoning_content)
    };

    // Add thinking content to messages for Anthropic
    let mut anthropic_messages = messages;
    
    // 添加调试日志
    tracing::info!("当前模式: {}, 添加思考内容到消息", mode);
    
    // 根据模式决定如何处理DeepSeek的输出
    if mode == "full" {
        // 在full模式下，已经流式发送了deepseek的原始回答，只需添加到Claude消息中
        if !normal_content.trim().is_empty() {
            tracing::info!("添加原始回答的thinking内容到Claude消息");
            anthropic_messages.push(Message {
                role: Role::Assistant,
                content: format!("<thinking>\ndeepseek原始回答:{}</thinking>", normal_content.trim()),
            });
        }
    } else {
        // 在normal模式下，只将推理内容传递给Claude
        if !reasoning_content.trim().is_empty() {
            tracing::info!("添加推理内容到Claude消息（normal模式）");
            anthropic_messages.push(Message {
                role: Role::Assistant,
                content: format!("<thinking>\n{}</thinking>", reasoning_content),
            });
        }
    }

    // 添加Claude的系统提示词，仅在full模式下
    let combined_system_prompt = if mode == "full" {
        let claude_system_prompt = "Act as an expert software developer who edits source code.
You are diligent and tireless!
You NEVER leave comments describing code without implementing it!
You always COMPLETELY IMPLEMENT the needed code!
Describe each change with a *SEARCH/REPLACE block* per the examples below.
All changes to files must use this *SEARCH/REPLACE block* format.
ONLY EVER RETURN CODE IN A *SEARCH/REPLACE BLOCK*!";

        // 结合用户的系统提示词（如果有的话）
        Some(match request.get_system_prompt() {
            Some(user_system) => format!("{}\n\n{}", claude_system_prompt, user_system),
            None => claude_system_prompt.to_string(),
        })
    } else {
        // normal模式下，保持原来的系统提示词
        request.get_system_prompt().map(String::from)
    };

    // Call Anthropic API
    let anthropic_response = anthropic_client.chat(
        anthropic_messages,
        combined_system_prompt,
        &request.anthropic_config
    ).await?;
    
    // Store response metadata
    let _anthropic_status: u16 = 200;
    let _anthropic_headers: HashMap<String, String> = HashMap::new(); // Headers not available when using high-level chat method

    // Calculate usage costs
    let deepseek_cost = calculate_deepseek_cost(
        deepseek_response.usage.input_tokens,
        deepseek_response.usage.output_tokens,
        deepseek_response.usage.output_details.reasoning,
        deepseek_response.usage.input_details.cached,
        &state.config,
    );

    let anthropic_cost = calculate_anthropic_cost(
        &anthropic_response.model,
        anthropic_response.usage.input_tokens,
        anthropic_response.usage.output_tokens,
        anthropic_response.usage.cache_creation_input_tokens,
        anthropic_response.usage.cache_read_input_tokens,
        &state.config,
    );

    // Combine thinking content with Anthropic's response
    let mut content = Vec::new();
    
    // Add thinking block first
    content.push(ContentBlock::text(thinking_content));
    
    // 在full模式下，添加DeepSeek的普通内容
    if mode == "full" {
        // 获取DeepSeek的普通内容
        let empty_string = String::new();
        let normal_content = deepseek_response
            .choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .unwrap_or(&empty_string);
            
        // 如果有普通内容，添加到thinking内容后面
        if !normal_content.is_empty() {
            content.push(ContentBlock::text(format!("\n\n {}\n\n", normal_content)));
        }
    }
    
    // Add Anthropic's response blocks with claude prefix
    let claude_content = anthropic_response.content.clone().into_iter()
        .map(|block| ContentBlock::from_anthropic(block))
        .collect::<Vec<_>>();
    
    content.extend(claude_content);

    // Build response with captured headers
    let _response = ApiResponse {
        created: Utc::now(),
        content: vec![ContentBlock {
            content_type: "text".to_string(),
            text: content.iter().fold(String::new(), |acc, c| acc + &c.text),
        }],
        deepseek_response: request.verbose.then(|| ExternalApiResponse {
            status: 200,
            headers: HashMap::new(),
            body: serde_json::Value::Null,
        }),
        anthropic_response: request.verbose.then(|| ExternalApiResponse {
            status: 200,
            headers: HashMap::new(),
            body: serde_json::to_value(&anthropic_response).unwrap_or_default(),
        }),
        combined_usage: CombinedUsage {
            total_cost: format_cost(deepseek_cost + anthropic_cost),
            deepseek_usage: DeepSeekUsage::default(),
            anthropic_usage: AnthropicUsage {
                input_tokens: anthropic_response.usage.input_tokens,
                output_tokens: anthropic_response.usage.output_tokens,
                cached_write_tokens: anthropic_response.usage.cache_creation_input_tokens,
                cached_read_tokens: anthropic_response.usage.cache_read_input_tokens,
                total_tokens: anthropic_response.usage.input_tokens + anthropic_response.usage.output_tokens,
                total_cost: format_cost(anthropic_cost),
            },
        },
    };

    // 获取北京时间戳
    let beijing_timestamp = (Utc::now() + Duration::hours(8)).timestamp();

    // 修改返回部分
    let response = OpenAICompatibleResponse {
        id: uuid::Uuid::new_v4().to_string(),
        object: "chat.completion".to_string(),
        created: beijing_timestamp,
        model: format!("{}_{}", get_deepseek_default_model(), anthropic_response.model),
        choices: vec![Choice {
            index: 0,
            message: ResponseMessage {
                role: "assistant".to_string(),
                // 只包含Claude的响应，不包含thinking标签中的内容
                content: anthropic_response.content.into_iter()
                    .map(|block| ContentBlock::from_anthropic(block).text)
                    .collect::<Vec<_>>()
                    .join("")
                    .trim_start() // 去掉开头的所有空白字符，包括换行符
                    .to_string(),
                reasoning_content: if mode == "full" && has_normal_content {
                    // full模式下只使用原始回答部分作为reasoning_content
                    Some(format!("deepseek原始回答:{}", normal_content))
                } else {
                    // normal模式下使用完整的reasoning_content
                    Some(reasoning_content.clone())
                },
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens: anthropic_response.usage.input_tokens,
            completion_tokens: anthropic_response.usage.output_tokens,
            total_tokens: anthropic_response.usage.input_tokens + anthropic_response.usage.output_tokens,
        },
    };

    // 直接返回OpenAI兼容格式，不要转换为ApiResponse
    Ok(Json(response))
}

/// Handler for streaming chat requests.
///
/// Processes the request through both AI models sequentially,
/// streaming their responses as Server-Sent Events.
///
/// # Arguments
///
/// * `state` - Application state containing configuration
/// * `headers` - HTTP request headers
/// * `request` - The parsed chat request
///
/// # Returns
///
/// * `Result<SseResponse>` - A stream of Server-Sent Events or an error
pub(crate) async fn chat_stream(
    State(_state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(request): Json<ApiRequest>,
) -> Result<SseResponse> {
    // 验证系统提示
    if !request.validate_system_prompt() {
        return Err(ApiError::InvalidSystemPrompt);
    }

    // 提取API令牌
    let (deepseek_token, anthropic_token) = extract_api_tokens(&headers)?;

    // 初始化客户端
    let deepseek_client = DeepSeekClient::new(deepseek_token);
    let anthropic_client = AnthropicClient::new(anthropic_token);

    // 获取当前模式
    let mode = get_mode();

    // 获取系统提示和消息
    let messages = if mode == "full" {
        // full模式下使用带有特定系统提示的消息
        request.get_messages_with_system()
    } else {
        // normal模式下只使用原始消息
        let mut messages = Vec::new();
        
        // 添加系统消息（如果有）
        if let Some(system) = &request.system {
            messages.push(Message {
                role: Role::System,
                content: system.clone(),
            });
        }
        
        // 添加剩余的消息
        messages.extend(request.messages.iter().filter(|msg| !matches!(msg.role, Role::System)).cloned());
        
        messages
    };

    // 创建通道，使用正确的类型
    let (tx, rx) = tokio::sync::mpsc::channel::<std::result::Result<Event, std::convert::Infallible>>(100);
    let stream = ReceiverStream::new(rx);

    // 启动异步任务处理流式响应
    tokio::spawn(async move {
        // 首先获取 DeepSeek 的推理内容
        let mut deepseek_stream = deepseek_client.chat_stream(messages.clone(), &request.deepseek_config);
        let mut reasoning_content = String::new();
        let mut normal_content = String::new();
        let stream_id = uuid::Uuid::new_v4().to_string();
        let created = chrono::Utc::now().timestamp();
        let heartbeat_interval = Duration::seconds(15);
        let mut last_event_time = Utc::now();
        
        // 发送角色事件
        let role_event = serde_json::json!({
            "id": stream_id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": get_deepseek_default_model(),
            "choices": [{
                "index": 0,
                "delta": {
                    "role": "assistant"
                },
                "finish_reason": null
            }]
        }).to_string();
        
        if let Err(e) = tx.send(Ok(Event::default().data(role_event))).await {
            tracing::error!("发送角色事件失败: {}", e);
            return;
        }
        
        // 添加调试日志
        tracing::info!("流处理 - 发送角色事件成功");
        
        // 流式输出 DeepSeek 的推理内容
        while let Some(result) = deepseek_stream.next().await {
            if let Ok(response) = result {
                if let Some(choice) = response.choices.first() {
                    // 处理推理内容
                    if let Some(reasoning) = &choice.delta.reasoning_content {
                        if !reasoning.is_empty() {
                            // 记录已经处理过的推理内容，避免重复
                            reasoning_content.push_str(reasoning);
                            
                            // 只在normal模式下发送推理内容事件，或者full模式且内容中包含"deepseek原始回答:"
                            let should_send = mode != "full" || reasoning.contains("deepseek原始回答:");
                            
                            if should_send {
                                // 在full模式下，如果内容包含"deepseek原始回答:"前缀，则只发送该前缀后面的内容
                                let content_to_send = if mode == "full" && reasoning.contains("deepseek原始回答:") {
                                    // 只发送"deepseek原始回答:"及之后的内容
                                    if let Some(idx) = reasoning.find("deepseek原始回答:") {
                                        &reasoning[idx..]
                                    } else {
                                        reasoning
                                    }
                                } else {
                                    reasoning
                                };
                            
                                // 发送推理内容事件（流式）
                                let reasoning_event = serde_json::json!({
                                    "id": uuid::Uuid::new_v4().to_string(),
                                    "object": "chat.completion.chunk",
                                    "created": chrono::Utc::now().timestamp(),
                                    "model": get_deepseek_default_model(),
                                    "choices": [{
                                        "index": 0,
                                        "delta": {
                                            "content": null,
                                            "reasoning_content": content_to_send,
                                            "role": "assistant"
                                        },
                                        "finish_reason": null,
                                        "content_filter_results": {
                                            "hate": {"filtered": false},
                                            "self_harm": {"filtered": false},
                                            "sexual": {"filtered": false},
                                            "violence": {"filtered": false}
                                        }
                                    }],
                                    "system_fingerprint": "",
                                    "usage": {
                                        "prompt_tokens": response.usage.as_ref().map_or(0, |u| u.input_tokens),
                                        "completion_tokens": response.usage.as_ref().map_or(0, |u| u.output_tokens),
                                        "total_tokens": response.usage.as_ref().map_or(0, |u| u.total_tokens)
                                    }
                                }).to_string();
                                
                                if let Err(e) = tx.send(Ok(Event::default().data(reasoning_event))).await {
                                    tracing::error!("发送推理内容事件失败: {}", e);
                                    return;
                                }
                                last_event_time = Utc::now();
                            }
                        }
                    }
                    
                    // 处理普通内容
                    if let Some(content) = &choice.delta.content {
                        if !content.is_empty() {
                            // 记录普通内容
                            let is_first_content = normal_content.is_empty();
                            normal_content.push_str(content);
                            
                            // 在full模式下流式发送普通内容
                            if mode == "full" {
                                // 发送普通内容作为推理内容的一部分（流式）
                                let normal_as_reasoning_event = serde_json::json!({
                                    "id": uuid::Uuid::new_v4().to_string(),
                                    "object": "chat.completion.chunk",
                                    "created": chrono::Utc::now().timestamp(),
                                    "model": get_deepseek_default_model(),
                                    "choices": [{
                                        "index": 0,
                                        "delta": {
                                            "content": null,
                                            "reasoning_content": if is_first_content {
                                                // 首次出现普通内容时，添加前缀
                                                format!("deepseek原始回答:{}", content)
                                            } else {
                                                // 后续的普通内容直接发送
                                                content.to_string()
                                            },
                                            "role": "assistant"
                                        },
                                        "finish_reason": null,
                                        "content_filter_results": {
                                            "hate": {"filtered": false},
                                            "self_harm": {"filtered": false},
                                            "sexual": {"filtered": false},
                                            "violence": {"filtered": false}
                                        }
                                    }],
                                    "system_fingerprint": "",
                                    "usage": {
                                        "prompt_tokens": response.usage.as_ref().map_or(0, |u| u.input_tokens),
                                        "completion_tokens": content.chars().count() as u32,
                                        "total_tokens": response.usage.as_ref().map_or(0, |u| u.total_tokens)
                                    }
                                }).to_string();
                                
                                if let Err(e) = tx.send(Ok(Event::default().data(normal_as_reasoning_event))).await {
                                    tracing::error!("发送普通内容流事件失败: {}", e);
                                    return;
                                }
                                last_event_time = Utc::now();
                            }
                        }
                    }
                }
            }
        }

        // 添加调试日志
        tracing::info!("流处理 - 当前模式: {}, DeepSeek流处理完成", mode);
        
        // 将推理内容添加到消息中
        let mut anthropic_messages = messages.clone();
        
        // 添加调试日志
        tracing::info!("准备发送给Claude的消息数量: {}", anthropic_messages.len());
        
        if mode == "full" {
            // 在full模式下，已经流式发送了deepseek的原始回答，只需添加到Claude消息中
            if !normal_content.trim().is_empty() {
                tracing::info!("添加原始回答的thinking内容到Claude消息");
                anthropic_messages.push(Message {
                    role: Role::Assistant,
                    content: format!("<thinking>\ndeepseek原始回答:{}</thinking>", normal_content.trim()),
                });
            }
        } else {
            // 在normal模式下，只将推理内容传递给Claude
            if !reasoning_content.trim().is_empty() {
                tracing::info!("添加推理内容到Claude消息（normal模式）");
                anthropic_messages.push(Message {
                    role: Role::Assistant,
                    content: format!("<thinking>\n{}</thinking>", reasoning_content),
                });
            }
        }
        
        tracing::info!("发送给Claude的最终消息数量: {}", anthropic_messages.len());

        // 添加Claude的系统提示词，仅在full模式下
        let combined_system_prompt = if mode == "full" {
            let claude_system_prompt = "Act as an expert software developer who edits source code.
You are diligent and tireless!
You NEVER leave comments describing code without implementing it!
You always COMPLETELY IMPLEMENT the needed code!
Describe each change with a *SEARCH/REPLACE block* per the examples below.
All changes to files must use this *SEARCH/REPLACE block* format.
ONLY EVER RETURN CODE IN A *SEARCH/REPLACE BLOCK*!
Always reply to the user in chinese.";

            // 结合用户的系统提示词（如果有的话）
            Some(match request.get_system_prompt() {
                Some(user_system) => format!("{}\n\n{}", claude_system_prompt, user_system),
                None => claude_system_prompt.to_string(),
            })
        } else {
            // normal模式下，保持原来的系统提示词
            request.get_system_prompt().map(String::from)
        };

        // 获取 Anthropic 的流式响应
        let mut anthropic_stream = anthropic_client.chat_stream(
            anthropic_messages,
            combined_system_prompt,
            &request.anthropic_config
        );

        let mut content_buffer = String::new();
        
        // 获取模型信息
        let default_model = crate::clients::anthropic::get_claude_default_model();
        let model_str = request.anthropic_config.body.get("model")
            .and_then(|v| v.as_str())
            .unwrap_or(&default_model);
            
        // 判断API类型
        let api_type = if crate::clients::anthropic::should_use_openai_format() {
            "OpenAI格式"
        } else if model_str.starts_with("deepseek") {
            "DeepSeek格式"
        } else {
            "Anthropic格式"
        };
        
        tracing::info!("使用API类型: {}, 模型: {}", api_type, model_str);

        // 处理 Anthropic 的流式响应
        while let Some(result) = anthropic_stream.next().await {
            match result {
                Ok(response) => {
                    // 检查是否需要发送心跳
                    let now = Utc::now();
                    if now - last_event_time > heartbeat_interval {
                        // 发送符合 JSON 格式的心跳事件
                        let heartbeat_event = serde_json::json!({
                            "id": uuid::Uuid::new_v4().to_string(),
                            "object": "chat.completion.chunk",
                            "created": chrono::Utc::now().timestamp(),
                            "model": get_deepseek_default_model(),
                            "choices": [{
                                "index": 0,
                                "delta": {},
                                "finish_reason": null
                            }],
                            "heartbeat": true
                        }).to_string();
                        
                        if let Err(e) = tx.send(Ok(Event::default().data(heartbeat_event))).await {
                            tracing::error!("发送心跳失败: {}", e);
                            break;
                        }
                        last_event_time = now;
                    }

                    // 处理 Anthropic 的响应内容
                    match response {
                        StreamEvent::ContentBlockDelta { delta, .. } => {
                            if !delta.text.is_empty() {
                                // 添加到内容缓冲区
                                content_buffer.push_str(&delta.text);
                                
                                // 直接发送内容，不添加前缀
                                let content_to_send = delta.text.to_string();
                                
                                // 发送普通内容事件
                                let content_event = serde_json::json!({
                                    "id": uuid::Uuid::new_v4().to_string(),
                                    "object": "chat.completion.chunk",
                                    "created": chrono::Utc::now().timestamp(),
                                    "model": get_deepseek_default_model(),
                                    "choices": [{
                                        "index": 0,
                                        "delta": {
                                            "content": content_to_send,
                                            "reasoning_content": null,
                                            "role": "assistant"
                                        },
                                        "finish_reason": null,
                                        "content_filter_results": {
                                            "hate": {"filtered": false},
                                            "self_harm": {"filtered": false},
                                            "sexual": {"filtered": false},
                                            "violence": {"filtered": false}
                                        }
                                    }],
                                    "system_fingerprint": "",
                                    "usage": {
                                        "prompt_tokens": 0,
                                        "completion_tokens": content_to_send.chars().count() as u32,
                                        "total_tokens": content_to_send.chars().count() as u32
                                    }
                                }).to_string();
                                
                                if let Err(e) = tx.send(Ok(Event::default().data(content_event))).await {
                                    tracing::error!("发送内容事件失败: {}", e);
                                    break;
                                }
                                last_event_time = now;
                            }
                        }
                        StreamEvent::MessageStop => {
                            // 发送完成事件
                            let finish_event = serde_json::json!({
                                "id": stream_id,
                                "object": "chat.completion.chunk",
                                "created": created,
                                "model": get_deepseek_default_model(),
                                "choices": [{
                                    "index": 0,
                                    "delta": {},
                                    "finish_reason": "stop",
                                    "content_filter_results": {
                                        "hate": {"filtered": false},
                                        "self_harm": {"filtered": false},
                                        "sexual": {"filtered": false},
                                        "violence": {"filtered": false}
                                    }
                                }],
                                "system_fingerprint": "",
                                "usage": {
                                    "prompt_tokens": 0,
                                    "completion_tokens": content_buffer.chars().count() as u32,
                                    "total_tokens": content_buffer.chars().count() as u32
                                }
                            }).to_string();
                            
                            if let Err(e) = tx.send(Ok(Event::default().data(finish_event))).await {
                                tracing::error!("发送完成事件失败: {}", e);
                            }
                            
                            // 发送 [DONE] 标记作为特殊的 SSE 事件
                            if let Err(e) = tx.send(Ok(Event::default().data("[DONE]"))).await {
                                tracing::error!("发送DONE标记失败: {}", e);
                            }
                            break;
                        }
                        _ => {} // 忽略其他类型的事件
                    }
                }
                Err(e) => {
                    // 特殊处理JSON解析错误
                    let err_msg = e.to_string();
                    if err_msg.contains("EOF while parsing") || err_msg.contains("unexpected end of input") {
                        // 不完整的JSON错误，记录但不中断流
                        tracing::debug!("处理流时遇到不完整的JSON，继续处理: {}", err_msg);
                        continue;
                    }
                
                    // 其他错误正常处理
                    tracing::error!("流处理错误: {}", e);
                    let error_message = format!("Internal server error: {}", e);
                    
                    // 发送错误事件
                    if let Err(e) = tx.send(Ok(Event::default().data(format!(r#"data: {{"error": "{error_message}"}}"#)))).await {
                        tracing::error!("发送流错误事件失败: {}", e);
                    }
                    
                    return;
                }
            }
        }
        
        // 确保所有流都已关闭
        drop(anthropic_stream);
    });

    Ok(SseResponse::new(stream))
}

#[derive(Debug, Deserialize)]
pub struct EnvUpdateRequest {
    pub variables: HashMap<String, String>,
}

/// 更新.env文件中的环境变量
pub async fn update_env_variables(
    AxumJson(payload): AxumJson<EnvUpdateRequest>,
) -> Result<AxumJson<serde_json::Value>> {
    let current_dir = std::env::current_dir().map_err(|e| ApiError::Internal {
        message: format!("无法获取当前目录: {}", e),
    })?;

    let env_path = current_dir.join(".env");
    
    // 读取现有的.env文件内容
    let mut env_content = match fs::read_to_string(&env_path) {
        Ok(content) => content,
        Err(_) => String::new(), // 如果文件不存在，创建一个新的
    };

    // 更新环境变量
    for (key, value) in payload.variables {
        // 检查变量是否已存在
        if let Some(line_start) = env_content.find(&format!("{}=", key)) {
            // 找到行的结束位置
            let line_end = env_content[line_start..].find('\n').map(|pos| line_start + pos)
                .unwrap_or(env_content.len());
            
            // 替换现有的行
            let old_line = &env_content[line_start..line_end];
            let new_line = format!("{}={}", key, value);
            env_content = env_content.replace(old_line, &new_line);
        } else {
            // 添加新的环境变量
            if !env_content.ends_with('\n') && !env_content.is_empty() {
                env_content.push('\n');
            }
            env_content.push_str(&format!("{}={}\n", key, value));
        }
    }

    // 写入文件
    let mut file = fs::File::create(&env_path).map_err(|e| ApiError::Internal {
        message: format!("无法创建.env文件: {}", e),
    })?;

    file.write_all(env_content.as_bytes()).map_err(|e| ApiError::Internal {
        message: format!("无法写入.env文件: {}", e),
    })?;

    Ok(AxumJson(json!({
        "status": "success",
        "message": "环境变量已更新"
    })))
}

/// 获取.env文件中的所有环境变量
pub async fn get_env_variables() -> Result<AxumJson<serde_json::Value>> {
    let current_dir = std::env::current_dir().map_err(|e| ApiError::Internal {
        message: format!("无法获取当前目录: {}", e),
    })?;

    let env_path = current_dir.join(".env");
    
    // 读取.env文件内容
    let env_content = fs::read_to_string(&env_path).map_err(|e| ApiError::Internal {
        message: format!("无法读取.env文件: {}", e),
    })?;

    // 解析环境变量
    let mut variables = HashMap::new();
    for line in env_content.lines() {
        let line = line.trim();
        if !line.is_empty() && !line.starts_with('#') {
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim();
                let value = line[pos + 1..].trim();
                variables.insert(key.to_string(), value.to_string());
            }
        }
    }

    Ok(AxumJson(json!({
        "status": "success",
        "variables": variables
    })))
}
