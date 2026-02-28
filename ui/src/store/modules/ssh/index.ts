// stores/terminal.ts
import { defineStore } from 'pinia';

export const useTerminalStore = defineStore('terminal', {
  state: () => ({
    instances: [] as { id: string; name: string }[],
    activeInstanceId: null as string | null,
    outputHistory: new Map<string, string[]>()
  }),
  actions: {
    addInstance(id: string, name: string) {
      this.instances.push({ id, name });
      if (!this.activeInstanceId) {
        this.activeInstanceId = id;
      }
      this.outputHistory.set(id, []);
    },
    recordOutput(instanceId: string, data: string) {
      const history = this.outputHistory.get(instanceId);
      if (history) {
        history.push(data);
        // 限制历史记录长度
        if (history.length > 1000) {
          history.shift();
        }
      }
    }
  }
});
