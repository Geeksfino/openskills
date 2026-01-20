// OpenSkills Agent Playground - Frontend JavaScript

let currentEventSource = null;

// 添加日志到日志面板
function addLog(level, message) {
  const logsDiv = document.getElementById("logs");
  const logEntry = document.createElement("div");
  
  const levelColors = {
    info: "text-blue-600",
    success: "text-green-600",
    error: "text-red-600",
    warning: "text-yellow-600",
  };
  
  const levelIcons = {
    info: "ℹ️",
    success: "✅",
    error: "❌",
    warning: "⚠️",
  };
  
  const timestamp = new Date().toLocaleTimeString();
  logEntry.className = `${levelColors[level] || "text-gray-600"} flex items-start gap-2`;
  logEntry.innerHTML = `
    <span class="flex-shrink-0">${levelIcons[level] || "•"}</span>
    <span class="text-gray-500 flex-shrink-0">[${timestamp}]</span>
    <span class="flex-1">${escapeHtml(message)}</span>
  `;
  
  logsDiv.appendChild(logEntry);
  logsDiv.scrollTop = logsDiv.scrollHeight;
  
  // 如果日志太多，移除最旧的
  while (logsDiv.children.length > 100) {
    logsDiv.removeChild(logsDiv.firstChild);
  }
}

// 清空日志
function clearLogs() {
  const logsDiv = document.getElementById("logs");
  logsDiv.innerHTML = '<div class="text-gray-400">日志已清空</div>';
}

// 页面加载时初始化
document.addEventListener("DOMContentLoaded", () => {
  checkHealth();
  loadSkills();
  enableInput();
});

// 健康检查
async function checkHealth() {
  try {
    const res = await fetch("/api/health");
    const data = await res.json();
    updateStatus("就绪", "green");
  } catch (error) {
    updateStatus("连接失败", "red");
    console.error("Health check failed:", error);
  }
}

// 更新状态指示器
function updateStatus(text, color) {
  const indicator = document.getElementById("status-indicator");
  const statusText = document.getElementById("status-text");
  indicator.className = `w-3 h-3 rounded-full bg-${color}-500`;
  statusText.textContent = text;
}

// 加载技能列表
async function loadSkills() {
  const list = document.getElementById("skills-list");
  list.innerHTML = '<div class="text-gray-500 text-sm">加载中...</div>';

  try {
    const res = await fetch("/api/skills");
    if (!res.ok) throw new Error("Failed to load skills");
    
    const skills = await res.json();
    
    if (skills.length === 0) {
      list.innerHTML = '<div class="text-gray-500 text-sm">暂无可用技能</div>';
      return;
    }

    list.innerHTML = skills
      .map(
        (skill) => `
        <div class="skill-card" onclick="useSkill('${skill.id}')" title="${skill.description}">
          <div class="font-semibold text-sm text-gray-800">${skill.id}</div>
          <div class="text-xs text-gray-600 mt-1 line-clamp-2">${skill.description}</div>
        </div>
      `
      )
      .join("");
  } catch (error) {
    list.innerHTML = `<div class="text-red-500 text-sm">加载失败: ${error.message}</div>`;
    console.error("Failed to load skills:", error);
  }
}

// 使用技能（插入到输入框）
function useSkill(skillId) {
  const input = document.getElementById("message-input");
  const currentValue = input.value.trim();
  
  if (currentValue) {
    input.value = `${currentValue}\n使用 ${skillId} 技能：`;
  } else {
    input.value = `使用 ${skillId} 技能：`;
  }
  
  input.focus();
  
  // 高亮显示被点击的技能卡片
  const cards = document.querySelectorAll(".skill-card");
  cards.forEach((card) => {
    if (card.textContent.includes(skillId)) {
      card.classList.add("active");
      setTimeout(() => card.classList.remove("active"), 1000);
    }
  });
}

// 启用输入
function enableInput() {
  const input = document.getElementById("message-input");
  const button = document.getElementById("send-button");
  input.disabled = false;
  button.disabled = false;
}

// 禁用输入
function disableInput() {
  const input = document.getElementById("message-input");
  const button = document.getElementById("send-button");
  input.disabled = true;
  button.disabled = true;
}

// 添加消息到界面
function addMessage(role, content, messageId = null) {
  const messagesDiv = document.getElementById("messages");
  const messageDiv = document.createElement("div");
  messageDiv.className = `message-${role} rounded-lg p-3 max-w-[80%] ${
    role === "user" ? "ml-auto" : ""
  }`;
  
  if (messageId) {
    messageDiv.id = messageId;
  }

  if (role === "assistant") {
    messageDiv.innerHTML = `
      <div class="font-semibold mb-1 text-gray-700">AI Assistant</div>
      <div class="text-gray-800 whitespace-pre-wrap">${escapeHtml(content)}</div>
    `;
  } else {
    messageDiv.innerHTML = `
      <div class="font-semibold mb-1 text-white">You</div>
      <div class="text-white whitespace-pre-wrap">${escapeHtml(content)}</div>
    `;
  }

  messagesDiv.appendChild(messageDiv);
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
  
  return messageDiv;
}

// 更新消息内容
function updateMessage(messageId, content) {
  const messageDiv = document.getElementById(messageId);
  if (messageDiv) {
    const contentDiv = messageDiv.querySelector(".text-gray-800, .text-white");
    if (contentDiv) {
      contentDiv.innerHTML = escapeHtml(content);
    }
  }
}

// 显示打字指示器
function showTypingIndicator() {
  const messagesDiv = document.getElementById("messages");
  const indicatorDiv = document.createElement("div");
  indicatorDiv.id = "typing-indicator";
  indicatorDiv.className = "message-assistant rounded-lg p-3 max-w-[80%]";
  indicatorDiv.innerHTML = `
    <div class="font-semibold mb-1 text-gray-700">AI Assistant</div>
    <div class="typing-indicator">
      <span class="typing-dot"></span>
      <span class="typing-dot" style="animation-delay: 0.2s"></span>
      <span class="typing-dot" style="animation-delay: 0.4s"></span>
    </div>
  `;
  messagesDiv.appendChild(indicatorDiv);
  messagesDiv.scrollTop = messagesDiv.scrollHeight;
}

// 移除打字指示器
function removeTypingIndicator() {
  const indicator = document.getElementById("typing-indicator");
  if (indicator) {
    indicator.remove();
  }
}

// 发送消息
async function sendMessage() {
  const input = document.getElementById("message-input");
  const message = input.value.trim();
  
  if (!message) return;
  if (currentEventSource) {
    // 如果已有请求在进行，先关闭
    currentEventSource.close();
  }

  // 添加用户消息到界面
  addMessage("user", message);
  input.value = "";

  // 显示"正在思考"
  showTypingIndicator();
  updateStatus("思考中...", "yellow");
  disableInput();

  // 创建消息 ID 用于更新
  const messageId = `msg-${Date.now()}`;
  removeTypingIndicator();
  addMessage("assistant", "", messageId);

  try {
    // 使用 Fetch API 处理 SSE
    const response = await fetch("/api/chat", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ message }),
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    let accumulatedContent = "";

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() || "";

      for (const line of lines) {
        if (line.startsWith("data: ")) {
          try {
            const data = JSON.parse(line.slice(6));
            
            if (data.type === "start") {
              updateStatus("处理中...", "yellow");
              clearLogs();
              addLog("info", "开始处理请求...");
            } else if (data.type === "log") {
              // 处理日志事件
              addLog(data.level || "info", data.message || "");
            } else if (data.type === "response") {
              accumulatedContent = data.content;
              updateMessage(messageId, accumulatedContent);
              addLog("success", "响应生成完成");
            } else if (data.type === "error") {
              updateMessage(messageId, `错误: ${data.content}`);
              updateStatus("错误", "red");
              addLog("error", `错误: ${data.content}`);
            } else if (data.type === "done") {
              updateStatus("就绪", "green");
              addLog("success", "请求处理完成");
            }
          } catch (e) {
            console.error("Failed to parse SSE data:", e);
            addLog("error", `解析数据失败: ${e.message}`);
          }
        }
      }
    }
  } catch (error) {
    updateMessage(messageId, `错误: ${error.message}`);
    updateStatus("错误", "red");
    console.error("Chat error:", error);
  } finally {
    enableInput();
    currentEventSource = null;
  }
}

// HTML 转义
function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}
