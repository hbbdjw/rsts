<script setup lang="ts">
import { onUnmounted, ref } from 'vue';
import { useMessage } from 'naive-ui';

const message = useMessage();
const videoRef = ref<HTMLVideoElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);
const processedImageSrc = ref<string>('');
const isStreaming = ref(false);

let ws: WebSocket | null = null;
let intervalId: number | null = null;
let stream: MediaStream | null = null;

function connectWebSocket() {
  const wsUrl = 'ws://localhost:8000/ws/sobel';
  ws = new WebSocket(wsUrl);

  ws.onopen = () => {
    // console.log('Sobel WebSocket connected');
    startSendingFrames();
  };

  ws.onmessage = event => {
    const blob = event.data;
    if (processedImageSrc.value) {
      URL.revokeObjectURL(processedImageSrc.value);
    }
    processedImageSrc.value = URL.createObjectURL(blob);
  };

  ws.onerror = () => {
    // console.error('WebSocket error:', error);
    message.error('WebSocket连接错误');
    stopCamera();
  };

  ws.onclose = () => {
    // console.log('WebSocket closed');
    if (isStreaming.value) {
      stopCamera();
    }
  };
}

function startSendingFrames() {
  if (!videoRef.value || !canvasRef.value) return;

  const ctx = canvasRef.value.getContext('2d');
  if (!ctx) return;

  intervalId = window.setInterval(() => {
    if (!videoRef.value || !ws || ws.readyState !== WebSocket.OPEN) return;

    const video = videoRef.value;
    const canvas = canvasRef.value!;

    if (video.videoWidth === 0 || video.videoHeight === 0) return;

    if (canvas.width !== video.videoWidth || canvas.height !== video.videoHeight) {
      canvas.width = video.videoWidth;
      canvas.height = video.videoHeight;
    }

    ctx.drawImage(video, 0, 0, canvas.width, canvas.height);

    canvas.toBlob(
      blob => {
        if (blob) {
          ws?.send(blob);
        }
      },
      'image/jpeg',
      0.6
    );
  }, 100);
}

function stopCamera() {
  isStreaming.value = false;

  if (intervalId) {
    clearInterval(intervalId);
    intervalId = null;
  }

  if (ws) {
    ws.close();
    ws = null;
  }

  if (stream) {
    stream.getTracks().forEach(track => track.stop());
    stream = null;
  }

  if (videoRef.value) {
    videoRef.value.srcObject = null;
  }

  if (processedImageSrc.value) {
    URL.revokeObjectURL(processedImageSrc.value);
    processedImageSrc.value = '';
  }
}

async function startCamera() {
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      video: {
        width: { ideal: 640 },
        height: { ideal: 480 },
        frameRate: { ideal: 60 } // 限制帧率以减轻后端压力
      }
    });

    if (videoRef.value) {
      videoRef.value.srcObject = stream;
      await videoRef.value.play();
    }

    isStreaming.value = true;

    connectWebSocket();
  } catch {
    // console.error('无法访问摄像头:', err);
    message.error('无法访问摄像头，请检查权限设置');
  }
}

onUnmounted(() => {
  stopCamera();
});
</script>

<template>
  <div class="h-full w-full flex flex-col gap-4 p-4">
    <NSpace>
      <NButton type="primary" :disabled="isStreaming" @click="startCamera">启动摄像头 & 实时处理</NButton>
      <NButton type="error" :disabled="!isStreaming" @click="stopCamera">停止</NButton>
    </NSpace>

    <div class="h-[500px] flex flex-row gap-4">
      <!-- 原视频 -->
      <div class="relative flex-1 overflow-hidden border rounded-lg bg-black">
        <div class="absolute left-2 top-2 rounded bg-black/50 px-2 py-1 text-white">原视频</div>
        <video ref="videoRef" class="h-full w-full object-contain" autoplay playsinline muted></video>
      </div>

      <!-- 处理后视频 -->
      <div class="relative flex-1 overflow-hidden border rounded-lg bg-black">
        <div class="absolute left-2 top-2 rounded bg-black/50 px-2 py-1 text-white">Sobel 边缘检测</div>
        <img v-if="processedImageSrc" :src="processedImageSrc" class="h-full w-full object-contain" />
        <div v-else class="h-full w-full flex items-center justify-center text-gray-500">等待处理结果...</div>
      </div>
    </div>

    <!-- 隐形Canvas用于截图 -->
    <canvas ref="canvasRef" class="hidden"></canvas>
  </div>
</template>

<style scoped>
/* 确保视频容器样式正确 */
</style>
