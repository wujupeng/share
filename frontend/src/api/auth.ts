import api from './index'

export interface LoginRequest {
  username: string
  password: string
}

export interface LoginResponse {
  token: string
  role: string
  expires_in: number
}

export interface UserProfile {
  username: string
  role: string
  status: string
}

export const authApi = {
  login(data: LoginRequest) {
    return api.post<{ data: LoginResponse }>('/auth/login', data)
  },
  logout() {
    return api.post('/auth/logout')
  },
  getProfile() {
    return api.get<{ data: UserProfile }>('/auth/profile')
  },
  changePassword(oldPassword: string, newPassword: string) {
    return api.put('/auth/password', { old_password: oldPassword, new_password: newPassword })
  },
}
