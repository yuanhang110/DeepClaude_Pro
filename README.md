<div align="center">
<h1>DeepClaude_Pro(OpenAI Compatible) 🐬🧠</h1>
<img src="frontend/public/deepclaude.png" width="300">

<div align="left">
This project is upgraded from the official Rust version of <a href="https://github.com/getAsterisk/deepclaude">deepclaude</a>. It supports the return results in OpenAI format and can be used in chatbox and cherrystudio. At the same time, it allows for relatively free replacement of third-party APIs of Claude or DeepSeek to achieve other model combinations such as deepclaude or deepgeminipro.

With the help of the API, this project can combine the reasoning ability of DeepSeek R1 with the creativity and code generation ability of Claude. As for the effectiveness, you can check the evaluation results of my other project, <a href="https://github.com/yuanhang110/DeepClaude_Benchmark">deepclaude's benchmark</a>.

In the future, I will further explore diverse model combinations and prompt engineering to optimize this project. The feature of this project is that if there are code modifications related to process or engineering optimization, the benchmark will be tested synchronously to ensure that everyone can use an API with a real effectiveness improvement. 

该项目是基于<a href="https://github.com/getAsterisk/deepclaude">deepclaude rust官方</a>版本升级而来，支持了OpenAI格式的返回结果，可以用于chatbox和cherrystudio，同时可以比较自由的替换claude 或者deepseek的第三方api来实现deepclaude或者deepgeminipro等其他模型组合。

借助API，该项目可以结合DeepSeek R1的推理能力以及Claude的创造力和代码生成能力。至于效果，可以看我另一个项目的评测结果<a href="https://github.com/yuanhang110/DeepClaude_Benchmark">deepclaude的benchmark</a>。

后续我将进一步尝试模型多样化组合和提示词工程去优化这个项目，这个项目特点是如果有流程或者工程优化相关的代码修改，会同步的测试benchmark，确保大家可以用上真实有效果提升的api。
</div>

[![Rust](https://img.shields.io/badge/rust-v1.75%2B-orange)](https://www.rust-lang.org/)
[![API Status](https://img.shields.io/badge/API-Stable-green)](https://deepclaude.asterisk.so)

</div>

<details open>
<summary><strong>更新日志：</strong></summary> 
<div>
2025-04-12: 更新 1.6版本，
  <li>支持免费使用gemini2.5pro的专属升级模式</li>
                      <li>支持升级pro+账户来使用gemini2.5pro和deepseekv3的组合模型</li>
</div>
<div>
2025-04-05: 更新 1.5版本，
  <li>正式版本发布，支持线上使用</li>
                      <li>支持用户注册</li>
                      <li>支持注册用户免费使用额度</li>
                      <li>支持pro账户升级</li>
                      <li>支持查看更新记录</li>
</div>
<div>
2025-03-20: 更新 1.3.1版本，前端密钥支持隐藏显示，后端修复claude的openai格式返回错误问题
</div>
<div>
2025-03-16: 更新 1.3版本，完整模式大更新，参照aider架构师编辑师模式，提升完整模式效果，benchmark的效果测试已完成
</div>
<div>
2025-03-15: 更新 1.2版本，后端大版本更新，新增完整模式，前端界面支持配置完整或者普通模式，benchmark的效果测试已完成
</div>
<div>
2025-03-14: 更新 1.1版本，支持前端界面配置环境变量，前端直接支持对话
</div>
<div>
2025-03-13: 更新 1.0.2版本，支持在.env文件中配置api路径和模型id
</div>
<div>
2025-03-11: 更新 1.0.1版本，修复cherrystudio输出问题
</div>
<div>
2025-03-09: 更新 1.0 版本，支持chatbox和cherrystudio
</div>
</details>
<details open>
<summary><strong>介绍视频：</strong></summary> 
<div>
 <a href="https://www.bilibili.com/video/BV1uVdeYtE46/">1.6版本deepclaude pro新增gemini2.5pro专属优化模式，提供免费试用额度</a>
</div>
<div>
 <a href="https://www.bilibili.com/video/BV1BGRfY3En3/">1.5版本deepclaude pro支持在线使用</a>
</div>
<div>
 <a href="https://www.bilibili.com/video/BV1NwXqYQEgH/?share_source=copy_web&vd_source=af0467782c65c2210ca5b92fa8959105">1.3.1版本deepclaude pro普通模式和架构师模式生成塞尔达版本超级马里奥对比</a>
</div>
<div>
 <a href="https://www.bilibili.com/video/BV1uAXuY7EeC/?share_source=copy_web&vd_source=af0467782c65c2210ca5b92fa8959105">1.3完整模式更新，包括deepclaude pro连接cursor教程</a>
</div>
<div>
 <a href="https://www.bilibili.com/video/BV1r8QXY9En9/?share_source=copy_web&vd_source=af0467782c65c2210ca5b92fa8959105">1.2后端大版本更新介绍，增加了完整模式</a>
</div>
<div>
 <a href="https://www.bilibili.com/video/BV179QKYQEHc/?share_source=copy_web&vd_source=af0467782c65c2210ca5b92fa8959105">1.1前端大版本更新介绍</a>
</div>
</details>




</details>

## 概述

DeepClaude是一个高性能的大语言模型（LLM）推理API，它将深度求索R1的思维链（CoT）推理能力与人工智能公司Anthropic的Claude模型在创造力和代码生成方面的优势相结合。它提供了一个统一的接口，让你在完全掌控自己的API密钥和数据的同时，充分利用这两个模型的优势。

## 在线访问地址

```
https://deepclaudepro.com/
```

## 功能特性
🚀 **零延迟** - 由高性能的Rust API驱动，先由R1的思维链提供即时响应，随后在单个流中呈现Claude的回复  
🔒 **私密且安全** - 采用端到端的安全措施，进行本地API密钥管理。你的数据将保持私密  
⚙️ **高度可配置** - 可自定义API和接口的各个方面，以满足你的需求  
🌟 **开源** - 免费的开源代码库。你可以根据自己的意愿进行贡献、修改和部署  
🤖 **双人工智能能力** - 将深度求索R1的推理能力与Claude的创造力和代码生成能力相结合  
🔑 **自带密钥管理的API** - 在我们的托管基础设施中使用你自己的API密钥，实现完全掌控

## 为什么选择R1和Claude？
深度求索R1的思维链轨迹展示了深度推理能力，达到了大语言模型能够进行“元认知”的程度——能够自我纠正、思考边缘情况，并以自然语言进行准蒙特卡洛树搜索。

然而，R1在代码生成、创造力和对话技巧方面有所欠缺。claude 3.5 sonnet版本在这些领域表现出色，是完美的补充。DeepClaude结合了这两个模型，以提供：
- R1卓越的推理和问题解决能力
- Claude出色的代码生成和创造力
- 单次API调用即可实现快速的流式响应
- 使用你自己的API密钥实现完全掌控

## 快速入门
### 先决条件
- Rust 1.75或更高版本
- 深度求索API密钥
- Anthropic API密钥

### 安装步骤
1. 克隆存储库：
   ```bash
   git clone https://github.com/getasterisk/deepclaude.git
   cd deepclaude
   ```
2. 构建项目：
   ```bash
   cargo build --release
   ```

3. 运行后端环境

   ```
   UST_LOG=debug cargo run --release
   ```

4. 运行前端环境

   windows中

   ```
   cd frontend & npm run dev
   ```

   macos中

   ```
   cd frontend && npm run dev
   ```

5. 前端访问地址

   ```
   http://localhost:3000/chat
   ```

### 模式切换
测试结果在：<a href="https://github.com/yuanhang110/DeepClaude_Benchmark">deepclaude的benchmark项目中</a>, full模式是参照aider官方的架构师编辑师模式实现，需要等更长时间，有更好的效果。

**方法一：**

在前端界面直接设置，在底下编辑完成后，可以直接保存环境变量到.env文件中

<img src="picture/mode.png" width="150" style="zoom: 200%;" >

**方法二：**

在项目根目录中编辑`.env`文件：

mode变量可以编辑为full或者normal

### 配置方法

第一步执行环境文件的模版迁移，会将 `.env.example` 文件复制为 `.env` 文件

mac os中

```
cp .env.example .env
```

windows中

```
copy .env.example .env
```

第二步就是配置.env文件的内容

**方法一：**

在前端界面直接设置，在底下编辑完成后，可以直接保存环境变量到.env文件中

<img src="picture/setting.png" width="150" style="zoom: 200%;" >

**方法二：**

在项目根目录中编辑`.env`文件：

```toml
# api密钥，自己取的
API_KEY=xyh110
# deepseek的密钥
DEEPSEEK_API_KEY=
# claude模型的密钥
ANTHROPIC_API_KEY=
# 服务的端口
PORT=1337
# 选择模式，包括full和normal，full是包括r1的结果且进行了专门的优化适合于编程，normal是只包含思考内容，所以full模型下，获取calude结果时间更长
MODE=normal
# API URL配置
# DeeepSeek的密钥
# 如果使用deepseek格式的api就填DEEPSEEK_OPENAI_TYPE_API_URL
DEEPSEEK_OPENAI_TYPE_API_URL=https://ark.cn-beijing.volces.com/api/v3/chat/completions
# Claude的密钥，底下两种2选1填
# 如果使用claude格式的api就填ANTHROPIC_API_URL，比如https://xxxx/v1/messages
ANTHROPIC_API_URL=
# 如果使用openai格式的api就填CLAUDE_OPENAI_TYPE_API_URL，比如https://xxxx/v1/chat/completions
CLAUDE_OPENAI_TYPE_API_URL=https://api.gptsapi.net/v1/chat/completions
# 模型配置
CLAUDE_DEFAULT_MODEL=claude-3-7-sonnet-20250219	
#DEEPSEEK_DEFAULT_MODEL=deepseek-r1-250120
DEEPSEEK_DEFAULT_MODEL=deepseek-r1-250120
```

## API使用方法

请参阅[API文档](https://deepclaude.chat)

### 非流式输出示例

```python
curl -X POST "http://127.0.0.1:1337/v1/chat/completions" \
  -H "Authorization: Bearer xyh110" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "deepclaude",
    "messages": [
        {"role": "user", "content": "你是谁"}
    ]
}'
```

### 流式传输示例
```python
curl -X POST "http://127.0.0.1:1337/v1/chat/completions" \
  -H "Authorization: Bearer xyh110" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "deepclaude",
    "messages": [
        {"role": "user", "content": "你是谁"}
    ],
    "stream": true
}'
```

## 配置选项
API支持通过请求体进行广泛的配置：
```json
{
  "stream": false,
  "verbose": false,
  "system": "可选的系统提示",
  "messages": [...],
  "deepseek_config": {
    "headers": {},
    "body": {}
  },
  "anthropic_config": {
    "headers": {},
    "body": {}
  }
}
```

## 配置chatbox和cherrystudio

密钥都是前面.env中配置的API_KEY=xxx，那么这里就填xxx

**chatbox**

<img src="picture/chatbox.png" width="600" style="zoom: 200%;" >

**cherrystudio**

<img src="picture/cherrystudio.png" width="600" style="zoom: 200%;" >

## 自主托管

DeepClaude可以在你自己的基础设施上进行自主托管。请按照以下步骤操作：
1. 配置环境变量或`config.toml`文件
2. 构建Docker镜像或从源代码编译
3. 部署到你首选的托管平台

## 安全性
- 不存储或记录数据
- 采用自带密钥（BYOK）架构
- 定期进行安全审计和更新

# 星星记录

[![Star History Chart](https://api.star-history.com/svg?repos=yuanhang110/DeepClaude_Pro&type=Date)](https://star-history.com/#yuanhang110/DeepClaude_Pro&Date)

## 贡献代码
我们欢迎贡献！请参阅我们的[贡献指南](CONTRIBUTING.md)，了解有关以下方面的详细信息：
- 行为准则
- 开发流程
- 提交拉取请求
- 报告问题
