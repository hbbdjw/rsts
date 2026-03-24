import { defineStore } from 'pinia';
import { ref } from 'vue';
import { getServiceBaseURL } from '@/utils/service';

interface TableDataRequest {
  connection_id: number;
  database: string;
  schema: string;
  table: string;
  page: number;
  page_size: number;
  sort_by?: string;
  sort_order?: 'ASC' | 'DESC';
}

interface ExecuteSqlRequest {
  connection_id: number;
  database: string;
  sql: string;
}

interface ExecuteSqlResponse {
  columns: string[] | null;
  rows: any[] | null;
  affected_rows: number | null;
  execution_time_ms: number;
  message: string | null;
}

export const useSqlStudioStore = defineStore('sqlstudio', () => {
  const tableData = ref<any[]>([]);
  const columns = ref<any[]>([]);
  const loading = ref(false);
  const pagination = ref({
    page: 1,
    pageSize: 50,
    total: 0,
    totalPages: 0
  });
  const currentTable = ref<{
    connectionId: number;
    database: string;
    schema: string;
    table: string;
  } | null>(null);

  const sortState = ref<{
    sortBy: string | undefined;
    sortOrder: 'ASC' | 'DESC' | undefined;
  }>({
    sortBy: undefined,
    sortOrder: undefined
  });
  
  const executionResult = ref<ExecuteSqlResponse | null>(null);

  const isHttpProxy = import.meta.env.DEV && import.meta.env.VITE_HTTP_PROXY === 'Y';
  const { baseURL } = getServiceBaseURL(import.meta.env, isHttpProxy);

  async function executeSql(sql: string, connectionId?: number, database?: string) {
    // If connection info is not provided, try to use currentTable info or fail
    const connId = connectionId ?? currentTable.value?.connectionId;
    const dbName = database ?? currentTable.value?.database;

    if (connId === undefined || !dbName) {
        window.$message?.error('请先选择数据库连接');
        return;
    }
    
    loading.value = true;
    executionResult.value = null; // Clear previous result
    tableData.value = []; // Clear table data (optional, but good for UI consistency if we reuse the table)
    columns.value = [];

    try {
        const body: ExecuteSqlRequest = {
            connection_id: connId,
            database: dbName,
            sql
        };
        
        let res;
        if (connId) {
             res = await fetch(`${baseURL}/api/sqlstudio/connection/execute`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(body)
             });
        } else {
             // SQLite fallback (generic query)
             res = await fetch(`${baseURL}/api/sqlite/query`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    db_name: dbName,
                    sql
                })
             });
        }

        if (!res.ok) {
             const text = await res.text();
             throw new Error(text || 'Execute failed');
        }
        
        const json = await res.json();
        
        if (connId) {
            if (json.code === 0 && json.data) {
                executionResult.value = json.data;
                // Update table data if it's a query result
                if (json.data.rows && json.data.columns) {
                    tableData.value = json.data.rows;
                    columns.value = json.data.columns.map((col: string) => ({
                        title: col,
                        key: col,
                        resizable: true,
                        ellipsis: { tooltip: true },
                        width: 150
                    }));
                    // Reset pagination for custom query as we don't have total count usually unless we fetch all
                    pagination.value.total = json.data.rows.length; 
                    pagination.value.page = 1;
                    pagination.value.totalPages = 1;
                }
            } else {
                throw new Error(json.msg || '执行失败');
            }
        } else {
            // SQLite response format adaptation
             // SQLite API returns { status: "ok", data: [], changed: number }
            executionResult.value = {
                columns: json.data && json.data.length > 0 ? Object.keys(json.data[0]) : [],
                rows: json.data,
                affected_rows: json.changed,
                execution_time_ms: 0, // SQLite API doesn't return time yet
                message: json.status === 'ok' ? 'Success' : 'Failed'
            };
             if (json.data) {
                tableData.value = json.data;
                 if (json.data.length > 0) {
                     columns.value = Object.keys(json.data[0]).map(key => ({
                        title: key,
                        key: key,
                        resizable: true,
                        ellipsis: { tooltip: true },
                        width: 150
                     }));
                 }
                pagination.value.total = json.data.length;
                pagination.value.page = 1;
                pagination.value.totalPages = 1;
            }
        }

    } catch (e: any) {
        window.$message?.error(e.message || '执行失败');
    } finally {
        loading.value = false;
    }
  }

  async function fetchTableData() {
    if (!currentTable.value) return;
    loading.value = true;
    try {
      const body: TableDataRequest = {
        connection_id: currentTable.value.connectionId,
        database: currentTable.value.database,
        schema: currentTable.value.schema,
        table: currentTable.value.table,
        page: pagination.value.page,
        page_size: pagination.value.pageSize,
        sort_by: sortState.value.sortBy,
        sort_order: sortState.value.sortOrder
      };

      const fullUrl = body.connection_id ? `${baseURL}/api/sqlstudio/connection/table-data` : `${baseURL}/api/sqlite/table-data`; 
      // Note: SQLite API might have different payload structure.
      // The current SQLite API uses GET /api/sqlite/table-data?db_name=...&table_name=...&page=...
      // I should unify them or handle difference.
      // The user specifically asked for "server/database/public/tables/table" which implies the new Postgres path.
      // I will handle Postgres here. SQLite logic in MenuPanel.vue calls /api/sqlite/table-data directly?
      // MenuPanel.vue doesn't fetch table data, it just shows tree.
      // I need to support both or at least the requested Postgres one.
      
      // If connection_id is present, it's a saved connection (Postgres etc).
      // If not, it's local SQLite.
      
      let res;
      if (body.connection_id) {
          res = await fetch(fullUrl, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body)
          });
      } else {
          // Local SQLite fallback (simplified for now, might need adaptation)
          // SQLite API: GET /api/sqlite/table-data?db_name=...&table_name=...&page=...&page_size=...
          const qs = new URLSearchParams({
              db_name: body.database, // SQLite uses db name
              table_name: body.table,
              page: body.page.toString(),
              page_size: body.page_size.toString()
          }).toString();
          res = await fetch(`${baseURL}/api/sqlite/table-data?${qs}`);
      }

      if (!res.ok) {
          const text = await res.text();
          throw new Error(text || 'Fetch failed');
      }
      const json = await res.json();
      
      // SQLite API returns direct object or {code, data}?
      // SQLite API: returns PaginationResult directly (from sqlite_api.rs: HttpResponse::Ok().json(result))
      // Postgres API: returns {code: 0, data: PaginationResult}
      
      let result: PaginationResult;
      if (body.connection_id) {
          if (json.code === 0 && json.data) {
              result = json.data;
          } else {
              throw new Error(json.msg || '加载失败');
          }
      } else {
          result = json as PaginationResult;
      }

        tableData.value = result.data;
        pagination.value.total = result.total;
        pagination.value.totalPages = result.total_pages;
        
        // Generate columns
        if (result.data.length > 0) {
          const firstRow = result.data[0];
          columns.value = Object.keys(firstRow).map(key => ({
            title: key,
            key: key,
            sorter: 'custom',
            resizable: true,
            ellipsis: { tooltip: true },
            width: 150
          }));
        } else if (columns.value.length === 0) {
            // If no data and no columns, maybe try to fetch metadata?
            // For now, leave empty.
        }

    } catch (e: any) {
      window.$message?.error(e.message || '加载失败');
    } finally {
      loading.value = false;
    }
  }

  function setTable(info: { connectionId: number; database: string; schema: string; table: string }) {
    currentTable.value = info;
    pagination.value.page = 1;
    sortState.value = { sortBy: undefined, sortOrder: undefined };
    columns.value = []; // Clear columns
    tableData.value = [];
    fetchTableData();
  }
  
  function handlePageChange(page: number) {
    pagination.value.page = page;
    fetchTableData();
  }

  function handlePageSizeChange(pageSize: number) {
    pagination.value.pageSize = pageSize;
    pagination.value.page = 1;
    fetchTableData();
  }

  function handleSorterChange(sorter: { columnKey: string; order: 'ascend' | 'descend' | false } | null) {
    if (!sorter || sorter.order === false) {
        sortState.value.sortBy = undefined;
        sortState.value.sortOrder = undefined;
    } else {
        sortState.value.sortBy = sorter.columnKey;
        sortState.value.sortOrder = sorter.order === 'ascend' ? 'ASC' : 'DESC';
    }
    fetchTableData();
  }

  return {
    tableData,
    columns,
    loading,
    pagination,
    currentTable,
    fetchTableData,
    executeSql,
    setTable,
    handlePageChange,
    handlePageSizeChange,
    handleSorterChange,
    executionResult
  };
});
