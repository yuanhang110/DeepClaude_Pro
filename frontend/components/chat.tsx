"use client"

import * as React from "react"
import { useState, useEffect, useRef, useMemo, useCallback } from "react"
import Image from "next/image"
import { useVirtualizer } from "@tanstack/react-virtual"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import rehypeRaw from "rehype-raw"
import rehypeSanitize from "rehype-sanitize"
import { Card } from "@/components/ui/card"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Dialog, DialogContent, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog"
import { ChevronRight, ChevronDown, Loader2, Settings2, PlusCircle, Trash2, Github } from "lucide-react"
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter"
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism"
import { CopyButton } from "@/components/ui/copy-button"
import { ChatInput } from "@/components/ui/chat-input"
import { usePostHog } from "../providers/posthog"

interface Message {
  role: "user" | "assistant"
  content: string
  thinking?: string
}

interface StoredChat {
  id: string
  title: string
  timestamp: string
  messages: Message[]
}

interface ChatProps {
  apiTokens: {
    deepseekApiToken: string
    anthropicApiToken: string
  }
}

export function Chat({ apiTokens }: ChatProps) {
  const posthog = usePostHog()
  const [messages, setMessages] = useState<Message[]>([])
  const [input, setInput] = useState("")
  const [isLoading, setIsLoading] = useState(false)
  const [useStreaming, setUseStreaming] = useState(true)
  const [openThinking, setOpenThinking] = useState<number | null>(null)
  const [autoExpandThinking, setAutoExpandThinking] = useState(false)
  const parentRef = useRef<HTMLDivElement>(null)
  const [isAutoScrollEnabled, setIsAutoScrollEnabled] = useState(true)
  const [isScrolling, setIsScrolling] = useState(false)
  const [chats, setChats] = useState<StoredChat[]>([])
  const [currentChatId, setCurrentChatId] = useState<string | null>(null)
  const [isSidebarOpen, setIsSidebarOpen] = useState(true)
  const [chatToDelete, setChatToDelete] = useState<string | null>(null)
  const [showClearConfirm, setShowClearConfirm] = useState(false)
  const [thinkingStartTime, setThinkingStartTime] = useState<number | null>(null)
  const [elapsedTime, setElapsedTime] = useState<number>(0)
  const [isThinkingComplete, setIsThinkingComplete] = useState<boolean>(false)

  // Format elapsed time into human readable string
  const formatElapsedTime = (seconds: number): string => {
    const minutes = Math.floor(seconds / 60)
    const remainingSeconds = seconds % 60

    if (minutes === 0) {
      return `${remainingSeconds} seconds`
    }
    return `${minutes} minute${minutes > 1 ? 's' : ''} ${remainingSeconds} seconds`
  }

  // Track elapsed time during thinking
  useEffect(() => {
    if (isLoading && !isThinkingComplete) {
      if (!thinkingStartTime) {
        setThinkingStartTime(Date.now())
      }

      const interval = setInterval(() => {
        if (thinkingStartTime) {
          setElapsedTime(Math.floor((Date.now() - thinkingStartTime) / 1000))
        }
      }, 1000)

      return () => clearInterval(interval)
    }
  }, [isLoading, thinkingStartTime, isThinkingComplete])

  // Load chats from localStorage on mount and create new chat
  useEffect(() => {
    const storedChats = localStorage.getItem('deepclaude-chats')
    if (storedChats) {
      const parsedChats = JSON.parse(storedChats)
      setChats(parsedChats)
    }
    // Always create a new chat on mount
    createNewChat()
  }, [])

  // Save chats to localStorage whenever they change
  useEffect(() => {
    if (chats.length > 0) {
      localStorage.setItem('deepclaude-chats', JSON.stringify(chats))
    }
  }, [chats])

  // Generate chat title from first message
  const generateChatTitle = (firstMessage: string): string => {
    return firstMessage.slice(0, 20)
  }

  // Delete a chat
  const deleteChat = (chatId: string) => {
    posthog.capture('chat_deleted', {
      chat_id: chatId,
      timestamp: new Date().toISOString()
    })
    setChats(prev => prev.filter(chat => chat.id !== chatId))
    if (currentChatId === chatId) {
      setCurrentChatId(null)
      setMessages([])
    }
    setChatToDelete(null)
  }

  // Clear all chats
  const clearAllChats = () => {
    posthog.capture('chats_cleared', {
      chats_count: chats.length,
      timestamp: new Date().toISOString()
    })
    setChats([])
    setCurrentChatId(null)
    setMessages([])
    localStorage.removeItem('deepclaude-chats')
    setShowClearConfirm(false)
  }

  // Generate UUID v4
  const generateUUID = () => {
    // Fallback UUID generator for older browsers
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
      const r = Math.random() * 16 | 0
      const v = c === 'x' ? r : (r & 0x3 | 0x8)
      return v.toString(16)
    })
  }

  // Create a new chat
  const createNewChat = () => {
    const chatId = typeof crypto.randomUUID === 'function'
      ? crypto.randomUUID()
      : generateUUID()
    const newChat: StoredChat = {
      id: chatId,
      title: '新对话',
      timestamp: new Date().toISOString(),
      messages: []
    }
    setChats(prev => [...prev, newChat])
    setCurrentChatId(chatId)
    setMessages([])

    posthog.capture('chat_created', {
      chat_id: chatId,
      timestamp: new Date().toISOString()
    })
  }

  // Update current chat
  const updateCurrentChat = useCallback(() => {
    if (currentChatId && messages.length > 0) {
      setChats(prev => {
        const updatedChats = prev.map(chat => {
          if (chat.id === currentChatId) {
            return {
              ...chat,
              messages,
              title: chat.messages.length === 0 && messages[0] ?
                generateChatTitle(messages[0].content) :
                chat.title
            }
          }
          return chat
        })
        return updatedChats
      })
    }
  }, [currentChatId, messages])

  // Update chat whenever messages change
  useEffect(() => {
    updateCurrentChat()
  }, [messages, updateCurrentChat])

  // Ref for current message
  const currentMessageRef = useRef<Message | null>(null)
  const scrollRef = useRef<number | null>(null)

  // Memoized renderers for code blocks
  const renderers = useMemo(() => {
    const CodeRenderer = React.memo(({ node, inline, className, children, ...props }: any) => {
      const match = /language-(\w+)/.exec(className || "")
      const language = match ? match[1] : "text"
      const content = String(children).replace(/\n$/, "")

      // Check if it's a code block (has language or multiple lines)
      const isCodeBlock = match || content.includes("\n")

      if (!inline && isCodeBlock) {
        return (
          <div className="relative">
            {!isLoading && <CopyButton value={content} />}
            <SyntaxHighlighter
              language={language}
              style={{
                ...oneDark,
                'pre[class*="language-"]': {
                  ...oneDark['pre[class*="language-"]'],
                  background: 'none',
                },
                'code[class*="language-"]': {
                  ...oneDark['code[class*="language-"]'],
                  background: 'none',
                }
              }}
              PreTag="div"
              customStyle={{
                margin: 0,
                borderRadius: "0.375rem"
              }}
              {...props}
            >
              {content}
            </SyntaxHighlighter>
          </div>
        )
      }

      // For inline code or single backticks
      return (
        <code className={className} {...props}>
          {children}
        </code>
      )
    })
    CodeRenderer.displayName = 'CodeRenderer'

    return {
      code: CodeRenderer
    }
  }, [isLoading])

  // Optimized virtual list with dynamic sizing and performance tweaks
  const rowVirtualizer = useVirtualizer({
    count: messages.length,
    getScrollElement: () => document.getElementById('chat-container'),
    estimateSize: useCallback(() => 100, []), // Lower initial estimate for faster first render
    overscan: 2, // Reduced overscan for better performance
    paddingStart: 20, // Add padding for smoother scrolling
    paddingEnd: 20,
    scrollPaddingStart: 20, // Additional scroll padding for smoother experience
    scrollPaddingEnd: 20
  })

  // RAF-based scroll handler
  const handleScroll = useCallback(() => {
    const container = document.getElementById('chat-container')
    if (!container || isScrolling) return

    const { scrollTop, scrollHeight, clientHeight } = container
    const isAtBottom = scrollHeight - (scrollTop + clientHeight) < 50
    setIsAutoScrollEnabled(isAtBottom)
  }, [isScrolling])

  // Immediate scroll to bottom
  const scrollToBottom = useCallback(() => {
    const container = document.getElementById('chat-container')
    if (!container) return

    if (scrollRef.current) {
      cancelAnimationFrame(scrollRef.current)
    }

    scrollRef.current = requestAnimationFrame(() => {
      if (!container) return
      container.scrollTo({
        top: container.scrollHeight,
        behavior: "auto"
      })
      scrollRef.current = null
    })
  }, [])

  // Immediate auto-scroll on message updates
  useEffect(() => {
    if (isAutoScrollEnabled && messages.length > 0) {
      scrollToBottom()
    }
  }, [messages, isAutoScrollEnabled, scrollToBottom])

  // Scroll event listener with cleanup
  useEffect(() => {
    const container = document.getElementById('chat-container')
    if (!container) return

    container.addEventListener("scroll", handleScroll, { passive: true })
    return () => {
      container.removeEventListener("scroll", handleScroll)
    }
  }, [handleScroll])

  // Memoized message renderer
  const MessageContent = useMemo(() => {
    const MemoizedMessageContent = React.memo(({ message, index }: { message: Message; index: number }) => {
      if (message.role === "user") {
        return (
          <div className="prose prose-zinc dark:prose-invert max-w-none bg-primary/10 rounded-lg px-4 py-3 message-transition" data-loaded="true">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={[rehypeRaw, rehypeSanitize]}
              components={renderers}
              className="message-content"
            >
              {message.content}
            </ReactMarkdown>
          </div>
        )
      }

      return (
        <>
          {message.thinking && (
            <Collapsible
              open={autoExpandThinking || openThinking === index}
              onOpenChange={(open) => setOpenThinking(open ? index : null)}
            >
              <div className="border border-border/40 rounded-lg message-transition" data-loaded="true">
                <CollapsibleTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="w-full flex items-center justify-between p-2 text-sm text-muted-foreground hover:text-primary"
                  >
                    <div className="flex items-center gap-2">
                      {!message.content && (
                        <Loader2 className="h-3 w-3 animate-spin" />
                      )}
                      Thinking
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-muted-foreground">
                        {isThinkingComplete
                          ? `Thought for ${formatElapsedTime(elapsedTime)}`
                          : formatElapsedTime(elapsedTime)}
                      </span>
                      {openThinking === index ? (
                        <ChevronDown className="h-4 w-4" />
                      ) : (
                        <ChevronRight className="h-4 w-4" />
                      )}
                    </div>
                  </Button>
                </CollapsibleTrigger>
                <CollapsibleContent>
                  <div className="p-4 text-sm italic text-muted-foreground whitespace-pre-wrap border-t border-border/40 message-content">
                    {message.thinking}
                  </div>
                </CollapsibleContent>
              </div>
            </Collapsible>
          )}
          <div className="prose prose-zinc dark:prose-invert max-w-none bg-muted/30 rounded-lg px-4 py-3 relative message-transition" data-loaded="true">
            {!isLoading && <CopyButton value={message.content} src="message" />}
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={[rehypeRaw, rehypeSanitize]}
              components={renderers}
              className="message-content"
            >
              {message.content}
            </ReactMarkdown>
          </div>
        </>
      )
    })
    MemoizedMessageContent.displayName = 'MemoizedMessageContent'
    return MemoizedMessageContent
  }, [openThinking, renderers, isLoading, elapsedTime, isThinkingComplete, autoExpandThinking])

  const handleSubmit = async () => {
    if (!input.trim() || isLoading) return
    if (!apiTokens.deepseekApiToken) return

    // Track message sent
    posthog.capture('message_sent', {
      chat_id: currentChatId,
      model: "deepclaude",
      message_length: input.length,
      has_code: input.includes('```'),
      timestamp: new Date().toISOString()
    })

    // Create new chat if none exists
    if (!currentChatId) {
      const newChat: StoredChat = {
        id: typeof crypto.randomUUID === 'function'
          ? crypto.randomUUID()
          : generateUUID(),
        title: generateChatTitle(input),
        timestamp: new Date().toISOString(),
        messages: []
      }
      setChats(prev => [...prev, newChat])
      setCurrentChatId(newChat.id)
    }

    // Create a new user message
    const userMessage: Message = {
      role: "user",
      content: input
    }

    setMessages(prev => [...prev, userMessage])
    setInput("")
    setIsLoading(true)
    setThinkingStartTime(null)
    setElapsedTime(0)
    setIsThinkingComplete(false)

    const controller = new AbortController()

    try {
      const response = await fetch("http://127.0.0.1:1337/v1/chat/completions", {
        method: "POST",
        signal: controller.signal,
        headers: {
          "Content-Type": "application/json",
          "Accept": "application/json",
          "Authorization": `Bearer ${apiTokens.deepseekApiToken}`
        },
        body: JSON.stringify({
          model: "deepclaude",
          messages: [...messages, { content: input, role: "user" }].map(msg => ({
            content: msg.content,
            role: msg.role
          })),
          stream: useStreaming
        })
      })

      // 初始化当前消息
      currentMessageRef.current = {
        role: "assistant",
        content: "",
        thinking: ""
      }

      if (useStreaming) {
        // 处理流式响应
        const reader = response.body?.getReader()
        if (!reader) throw new Error("No reader available")

        let isThinking = false

        const processLine = (line: string) => {
          if (!line.trim()) return
          
          // 处理流式响应结束标记
          if (line === "data: [DONE]") {
            return
          }
          
          // 确保行以"data: "开头
          if (!line.startsWith("data: ")) return

          try {
            const data = JSON.parse(line.slice(6))

            if (data.error) {
              console.error("Server error:", data.error)
              throw new Error(data.error.message)
            }

            if (data.choices && data.choices[0]) {
              const choice = data.choices[0]
              if (!currentMessageRef.current) return

              if (choice.delta.reasoning_content) {
                currentMessageRef.current.thinking += choice.delta.reasoning_content
                setIsThinkingComplete(false)
              } else if (choice.delta.content) {
                currentMessageRef.current.content += choice.delta.content
              }

              // Update message immediately with optimized state update
              setMessages(prev => {
                // Avoid unnecessary array operations if content hasn't changed
                const lastMessage = prev[prev.length - 1]
                if (lastMessage?.role === "assistant" &&
                  lastMessage.content === currentMessageRef.current!.content &&
                  lastMessage.thinking === currentMessageRef.current!.thinking) {
                  return prev
                }

                // Create new array only when content has changed
                if (lastMessage?.role === "assistant") {
                  const newMessages = [...prev]
                  newMessages[newMessages.length - 1] = { ...currentMessageRef.current! }
                  return newMessages
                }
                return [...prev, { ...currentMessageRef.current! }]
              })
            }
          } catch (error) {
            console.error("Error parsing JSON:", line, error)
            // 不抛出错误，继续处理下一行
          }
        }

        while (true) {
          const { done, value } = await reader.read()
          if (done) break

          const chunk = new TextDecoder().decode(value)
          // 处理可能包含多个完整或不完整数据块的情况
          const lines = chunk.split("\n")

          for (const line of lines) {
            try {
              processLine(line)
            } catch (error) {
              console.error("Error processing line:", error)
              // 继续处理下一行，不中断整个流程
            }
          }
        }
      } else {
        // 处理非流式响应
        try {
          const responseText = await response.text()
          
          if (!response.ok) {
            let errorMessage = "请求失败"
            try {
              const errorData = JSON.parse(responseText)
              errorMessage = errorData.error?.message || errorMessage
            } catch (e) {
              // 如果无法解析为JSON，使用原始响应文本
              errorMessage = responseText || errorMessage
            }
            throw new Error(errorMessage)
          }
          
          let data
          try {
            data = JSON.parse(responseText)
          } catch (e) {
            console.error("解析响应数据失败:", e)
            throw new Error("解析响应数据失败")
          }
          
          let content = ""
          
          if (data.choices && data.choices[0]) {
            // 处理不同的响应格式
            if (data.choices[0].message && data.choices[0].message.content) {
              // OpenAI格式
              content = data.choices[0].message.content
            } else if (data.choices[0].text) {
              // 可能的其他格式
              content = data.choices[0].text
            } else if (typeof data.choices[0] === 'string') {
              // 纯文本格式
              content = data.choices[0]
            }
          } else if (data.content) {
            // 直接包含content的格式
            content = data.content
          } else {
            throw new Error("无法解析响应数据")
          }
          
          currentMessageRef.current.content = content
          
          setMessages(prev => [...prev, { 
            role: "assistant", 
            content: content 
          }])
        } catch (error) {
          console.error("处理响应失败:", error)
          throw error
        }
      }
    } catch (error) {
      console.error("Error:", error)
      // Track error
      posthog.capture('chat_error', {
        chat_id: currentChatId,
        error: error instanceof Error ? error.message : String(error),
        timestamp: new Date().toISOString()
      })
    } finally {
      setIsLoading(false)
      // Clean up
      if (scrollRef.current) {
        cancelAnimationFrame(scrollRef.current)
      }
    }

    return () => {
      controller.abort()
      if (scrollRef.current) {
        cancelAnimationFrame(scrollRef.current)
      }
    }
  }

  const hasApiTokens = apiTokens.deepseekApiToken && apiTokens.anthropicApiToken

  return (
    <div className="flex min-h-screen">
      {/* Delete Chat Confirmation Dialog */}
      <Dialog open={!!chatToDelete} onOpenChange={() => setChatToDelete(null)}>
        <DialogContent>
          <DialogTitle>删除对话</DialogTitle>
          <DialogDescription>
            确定要删除此对话吗？此操作无法撤消。
          </DialogDescription>
          <DialogFooter className="flex gap-2 justify-end">
            <Button variant="outline" onClick={() => setChatToDelete(null)}>
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={() => chatToDelete && deleteChat(chatToDelete)}
            >
              删除
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Clear All Chats Confirmation Dialog */}
      <Dialog open={showClearConfirm} onOpenChange={setShowClearConfirm}>
        <DialogContent>
          <DialogTitle>Clear All Chats</DialogTitle>
          <DialogDescription>
            Are you sure you want to clear all chats? This action cannot be undone.
          </DialogDescription>
          <DialogFooter className="flex gap-2 justify-end">
            <Button variant="outline" onClick={() => setShowClearConfirm(false)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={clearAllChats}
            >
              Clear All
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Backdrop */}
      {isSidebarOpen && (
        <div
          className="fixed inset-0 bg-background/80 backdrop-blur-sm z-40 lg:hidden"
          onClick={() => setIsSidebarOpen(false)}
        />
      )}

      {/* Sidebar */}
      <aside
        className={`fixed top-0 left-0 h-full w-64 border-r border-border/40 bg-background transition-all duration-300 ease-in-out z-50 ${
          isSidebarOpen ? 'translate-x-0' : '-translate-x-64'
          }`}
      >
        <div className="flex flex-col h-full justify-between">
          {/* Top Section */}
          <div className="flex-shrink-0">
            <div className="p-4 border-b border-border/40">
              <div className="flex gap-2">
                <Button
                  onClick={createNewChat}
                  className="flex-1"
                >
                  <PlusCircle className="h-4 w-4 mr-2" />
                  新对话
                </Button>
                <Button
                  variant="outline"
                  size="icon"
                  onClick={() => setShowClearConfirm(true)}
                  className="shrink-0"
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </div>
            </div>
            {/* Chat History - Scrollable Section */}
            <div className="flex-1 overflow-y-auto p-2 h-[calc(100vh-220px)] sidebar-scroll">
              {chats.map(chat => (
                <div
                  key={chat.id}
                className={`group flex items-center gap-2 p-3 rounded-lg mb-2 hover:bg-muted/50 transition-colors ${
                  currentChatId === chat.id ? 'bg-muted' : ''
                    }`}
                >
                  <button
                    onClick={() => {
                      setCurrentChatId(chat.id)
                      setMessages(chat.messages)
                      // Track chat selection
                      posthog.capture('chat_selected', {
                        chat_id: chat.id,
                        timestamp: new Date().toISOString()
                      })
                    }}
                    className="flex-1 text-left"
                  >
                    <div className="font-mono text-sm truncate">{chat.title}</div>
                    <div className="text-xs text-muted-foreground">
                      {new Date(chat.timestamp).toLocaleString()}
                    </div>
                  </button>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={(e) => {
                      e.stopPropagation()
                      setChatToDelete(chat.id)
                    }}
                    className="opacity-0 group-hover:opacity-100 transition-opacity"
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              ))}
            </div>
          </div>

          {/* Bottom Section */}
          {/* Bottom Section */}
          <div className="flex-shrink-0 p-4 pb-8 border-t border-border/40 space-y-4">
            <a
              href="https://github.com/yuanhang110/DeepClaude_Pro/issues/new"
              target="_blank"
              rel="noopener noreferrer"
              onClick={() => {
                posthog.capture('github_issue_click', {
                  timestamp: new Date().toISOString()
                })
              }}
            >
              <Button
                variant="outline"
                className="w-full"
              >
                <Github className="h-4 w-4 mr-2" />
                提交bug给GitHub
              </Button>
            </a>

            <div className="flex items-center justify-center text-sm text-muted-foreground whitespace-nowrap">
              <span className="flex-shrink-0 mr-1">一个"好玩"的项目由</span>
              <span className="flex-shrink-0">DeepClaude</span>
            </div>
          </div>
        </div>
      </aside>

      {/* Toggle Sidebar Button */}
      <button
        onClick={() => {
          const newState = !isSidebarOpen
          setIsSidebarOpen(newState)
          // Track sidebar toggle
          posthog.capture('sidebar_toggled', {
            new_state: newState ? 'open' : 'closed',
            timestamp: new Date().toISOString()
          })
        }}
        className={`fixed top-4 z-50 p-2 bg-muted/30 hover:bg-muted/50 rounded-lg transition-all duration-300 ease-in-out ${
          isSidebarOpen ? 'left-[268px]' : 'left-4'
          }`}
      >
        <ChevronRight
          className={`h-4 w-4 transition-transform duration-300 ${
            isSidebarOpen ? 'rotate-180' : ''
            }`}
        />
      </button>

      {/* Main Chat Area */}
      <main
        className={`flex-1 transition-[margin] duration-300 ease-in-out ${
          isSidebarOpen ? 'ml-64' : 'ml-0'
          }`}
      >
        <div className="container max-w-4xl mx-auto px-4 flex flex-col h-screen">
          <header className="sticky top-0 py-4 px-2 bg-background/80 backdrop-blur z-40 border-b border-border/40">
            <div className="flex items-center justify-center gap-2">
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setUseStreaming(!useStreaming)}
                className={`bg-muted/30 ${useStreaming ? 'text-green-500' : 'text-gray-500'}`}
                title={useStreaming ? "当前为流式响应" : "当前为非流式响应"}
              >
                {useStreaming ? "流式" : "非流式"}
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setAutoExpandThinking(!autoExpandThinking)}
                className={`bg-muted/30 ${autoExpandThinking ? 'text-green-500' : 'text-gray-500'}`}
                title={autoExpandThinking ? "思考内容默认展开" : "思考内容默认折叠"}
              >
                思考
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={createNewChat}
                className="bg-muted/30"
              >
                <PlusCircle className="h-4 w-4" />
              </Button>
            </div>
          </header>
          <div
            ref={parentRef}
            className="flex-1 w-full overflow-y-auto px-2"
            id="chat-container"
          >
            <div
              className="relative mx-auto min-h-full py-4"
              style={{
                height: messages.length > 0 ? `${rowVirtualizer.getTotalSize()}px` : '100%',
                minHeight: '100%'
              }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const message = messages[virtualRow.index]
                const index = virtualRow.index
                return (
                  <div
                    key={virtualRow.index}
                    data-index={virtualRow.index}
                    ref={rowVirtualizer.measureElement}
                    className="absolute left-0 w-full virtual-item-transition"
                    style={{
                      transform: `translate3d(0, ${virtualRow.start}px, 0)`
                    }}
                  >
                    <div
                      className={`py-4 message-transition ${message.role === "assistant" ? "border-b border-border/40" : ""}`}
                      data-loaded="true"
                    >
                      <div className="max-w-4xl mx-auto space-y-3 px-4">
                        <div className="font-medium text-sm text-muted-foreground message-content">
                          {message.role === "user" ? "You" : "Assistant"}
                        </div>
                        <MessageContent message={message} index={index} />
                      </div>
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
          <div className="sticky bottom-0 bg-background/80 backdrop-blur border-t border-border/40 w-full">
            <div className="py-4 px-2">
              <ChatInput
                value={input}
                onChange={setInput}
                onSubmit={handleSubmit}
                placeholder={hasApiTokens ? "发一条消息......" : "请先在设置中配置API密钥"}
              />
            </div>
          </div>
        </div>
      </main>
    </div>
  )
}
