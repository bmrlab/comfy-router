import axios from "axios";

const api = axios.create({
  baseURL: import.meta.env.DEV ? "http://localhost:8080" : void 0,
});

api.interceptors.request.use(
  (config) => {
    if (import.meta.env.DEV) {
      const username = "admin";
      const password = "admin";

      // Encode credentials in base64
      const token = btoa(`${username}:${password}`);

      // Set Authorization header
      config.headers["Authorization"] = `Basic ${token}`;
    }

    return config;
  },
  (error) => {
    // Handle error
    return Promise.reject(error);
  }
);

export default api;
