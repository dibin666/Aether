/**
 * 带重试机制和缓存处理的动态导入工具
 */

const MAX_RETRIES = 3
const RETRY_DELAY = 1000 // 1秒
const CACHE_BUSTER_DELAY = 2000 // 2秒后尝试缓存清除

const MODULE_LOAD_ERROR_FRAGMENTS = [
  'failed to fetch',
  'loading chunk',
  'dynamically imported module',
  'networkerror',
  'importing a module script failed',
  'failed to load module script',
]

// 模块缓存
const moduleCache = new Map<string, Promise<unknown>>()

/**
 * 清除浏览器缓存的工具函数
 */
function clearBrowserCache() {
  if (typeof window !== 'undefined') {
    // 清除一些可能的缓存
    if ('caches' in window) {
      caches.keys().then(names => {
        names.forEach(name => {
          caches.delete(name)
        })
      })
    }
  }
}

/**
 * 检查错误是否是动态模块加载失败（常见于部署后旧页面尝试加载已删除 chunk）
 */
export function isModuleLoadFailure(error: unknown): boolean {
  const err = error as { message?: string; name?: string } | null
  const errorMessage = String(err?.message || '').toLowerCase()
  return (
    MODULE_LOAD_ERROR_FRAGMENTS.some(fragment => errorMessage.includes(fragment)) ||
    err?.name === 'ChunkLoadError'
  )
}

/**
 * 使用时间戳参数强制重新请求最新页面入口，绕过浏览器/代理缓存。
 */
export function reloadPageBypassingCache(): void {
  if (typeof window === 'undefined') return
  const url = new URL(window.location.href)
  url.searchParams.set('_t', Date.now().toString())
  window.location.replace(url.toString())
}

/**
 * 重试动态导入
 * @param importFn 动态导入函数
 * @param retries 剩余重试次数
 * @param cacheKey 缓存键
 * @returns Promise
 */
export async function importWithRetry<T = unknown>(
  importFn: () => Promise<T>,
  retries: number = MAX_RETRIES,
  cacheKey?: string
): Promise<T> {
  try {
    // 如果有缓存键且缓存中存在，直接返回
    if (cacheKey && moduleCache.has(cacheKey)) {
      return await moduleCache.get(cacheKey) as T
    }

    const importPromise = importFn()

    // 缓存 Promise
    if (cacheKey) {
      moduleCache.set(cacheKey, importPromise)
    }

    const result = await importPromise
    return result
  } catch (error) {
    // 如果是缓存相关错误，清除对应缓存
    if (cacheKey && moduleCache.has(cacheKey)) {
      moduleCache.delete(cacheKey)
    }

    if (retries > 0 && isModuleLoadFailure(error)) {
      // 如果是第二次重试，尝试清除浏览器缓存
      if (MAX_RETRIES - retries + 1 === 2) {
        clearBrowserCache()
        await new Promise(resolve => setTimeout(resolve, CACHE_BUSTER_DELAY))
      } else {
        await new Promise(resolve => setTimeout(resolve, RETRY_DELAY))
      }

      return importWithRetry(importFn, retries - 1, cacheKey)
    }

    // 最后的 fallback：如果是模块加载错误，刷新页面拉取最新入口
    if (isModuleLoadFailure(error)) {
      reloadPageBypassingCache()
    }
    throw error
  }
}

/**
 * 创建带重试的组件导入函数
 * @param importPath 组件路径
 * @returns 组件导入函数
 */
export function createRetryableImport(importPath: string) {
  const cacheKey = importPath
  return () => importWithRetry(() => import(/* @vite-ignore */ importPath), MAX_RETRIES, cacheKey)
}

/**
 * 预加载关键模块
 */
export function preloadCriticalModules() {
  // 在开发环境中预加载已被禁用，因为路径别名在运行时动态导入中不可用
  // 模块会在需要时按需加载，这在开发环境中是可接受的
  // 生产环境中模块已经被构建和优化，不需要预加载
}
