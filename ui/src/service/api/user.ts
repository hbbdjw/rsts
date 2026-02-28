import { request } from '../request';

/** get user theme config */
export function fetchUserThemeConfig() {
  return request<{ themeConfig: string | null }>({
    url: '/api/user/theme-config',
    method: 'get'
  });
}

/** update user theme config */
export function updateUserThemeConfig(config: string) {
  return request<null>({
    url: '/api/user/theme-config',
    method: 'post',
    data: {
      theme_config: config
    }
  });
}
